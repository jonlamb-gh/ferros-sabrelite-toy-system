#![no_std]
#![feature(proc_macro_hygiene)]

mod error;

use error::TopLevelError;
use ferros::alloc::micro_alloc::*;
use ferros::alloc::*;
use ferros::bootstrap::*;
use ferros::cap::*;
use ferros::userland::*;
use ferros::vspace::ElfProc;
use ferros::vspace::*;
use ferros::*;
use net_types::{EthernetAddress, IpcEthernetFrame, IpcUdpTransmitBuffer, Ipv4Address, MtuSize};
use sabrelite_bsp::{debug_logger::DebugLogger, pac};
use typenum::*;

/// 2^16 bytes in the L2 queues can buffer ~43 Ethernet frames
type L2IpcQueuePageBits = U16;
type L2IpcQueueDepth = op!(((U1 << L2IpcQueuePageBits) / MtuSize) - U1);

/// 2^14 bytes in the UDP queue can buffer ~10 Ethernet frames
type UdpIpcQueuePageBits = U14;
type UdpIpcQueueDepth = op!(((U1 << UdpIpcQueuePageBits) / MtuSize) - U1);

const MAC_ADDRESS: EthernetAddress = EthernetAddress([0x00, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]);
const IP_ADDRESS: Ipv4Address = Ipv4Address([192, 0, 2, 80]);

static LOGGER: DebugLogger = DebugLogger;

extern "C" {
    static _selfe_arc_data_start: u8;
    static _selfe_arc_data_end: usize;
}

#[allow(clippy::type_complexity)]
mod resources {
    include! {concat!(env!("OUT_DIR"), "/resources.rs")}
}

fn main() {
    let raw_bootinfo = unsafe { &*sel4_start::BOOTINFO };
    run(raw_bootinfo).expect("Failed to run root task setup");
}

fn run(raw_bootinfo: &'static selfe_sys::seL4_BootInfo) -> Result<(), TopLevelError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(DebugLogger::max_log_level_from_env()))?;
    log::debug!("[root-task] Initializing");

    let (allocator, mut dev_allocator) = micro_alloc::bootstrap_allocators(raw_bootinfo)?;
    let mut allocator = WUTBuddy::from(allocator);

    let (root_cnode, local_slots) = root_cnode(raw_bootinfo);
    let (root_vspace_slots, local_slots): (LocalCNodeSlots<U100>, _) = local_slots.alloc();
    let (ut_slots, local_slots): (LocalCNodeSlots<U100>, _) = local_slots.alloc();
    let mut ut_slots = ut_slots.weaken();

    let BootInfo {
        mut root_vspace,
        asid_control,
        user_image,
        root_tcb,
        mut irq_control,
        ..
    } = BootInfo::wrap(
        raw_bootinfo,
        allocator.alloc_strong::<U16>(&mut ut_slots)?,
        root_vspace_slots,
    );

    let tpa = root_tcb.downgrade_to_thread_priority_authority();

    let archive_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(
            &_selfe_arc_data_start,
            &_selfe_arc_data_end as *const _ as usize - &_selfe_arc_data_start as *const _ as usize,
        )
    };

    let archive = selfe_arc::read::Archive::from_slice(archive_slice);
    let iomux_elf_data = archive.file(resources::Iomux::IMAGE_NAME)?;
    log::debug!(
        "[root-task] Found iomux ELF data size={}",
        iomux_elf_data.len()
    );
    let enet_elf_data = archive.file(resources::Enet::IMAGE_NAME)?;
    log::debug!(
        "[root-task] Found enet ELF data size={}",
        enet_elf_data.len()
    );
    let tcpip_elf_data = archive.file(resources::TcpIp::IMAGE_NAME)?;
    log::debug!(
        "[root-task] Found tcpip ELF data size={}",
        tcpip_elf_data.len()
    );
    let pstorage_elf_data = archive.file(resources::PersistentStorage::IMAGE_NAME)?;
    log::debug!(
        "[root-task] Found persistent-storage ELF data size={}",
        pstorage_elf_data.len()
    );
    let console_elf_data = archive.file(resources::Console::IMAGE_NAME)?;
    log::debug!(
        "[root-task] Found console ELF data size={}",
        console_elf_data.len()
    );

    let uts = alloc::ut_buddy(allocator.alloc_strong::<U27>(&mut ut_slots)?);

    smart_alloc!(|slots: local_slots, ut: uts| {
        let (asid_pool, _asid_control) = asid_control.allocate_asid_pool(ut, slots)?;

        let ut_for_scratch: LocalCap<Untyped<U12>> = ut;
        let sacrificial_page = ut_for_scratch.retype(slots)?;
        let reserved_for_scratch = root_vspace.reserve(sacrificial_page)?;
        let mut scratch = reserved_for_scratch.as_scratch(&root_vspace).unwrap();

        //
        // drivers/iomux setup
        //

        log::debug!("[root-task] Setting up iomux driver");

        let (asid, asid_pool) = asid_pool.alloc();
        let vspace_slots: LocalCNodeSlots<U16> = slots;
        let vspace_ut: LocalCap<Untyped<U16>> = ut;
        let mut iomux_vspace = VSpace::new_from_elf::<resources::Iomux>(
            retype(ut, slots)?, // paging_root
            asid,
            vspace_slots.weaken(), // slots
            vspace_ut.weaken(),    // paging_untyped
            iomux_elf_data,
            slots, // page_slots
            ut,    // elf_writable_mem
            &user_image,
            &root_cnode,
            &mut scratch,
        )?;
        let (iomux_cnode, iomux_slots) = retype_cnode::<U12>(ut, slots)?;
        let (ipc_slots, _iomux_slots) = iomux_slots.alloc();
        let (iomux_ipc_setup, responder) = call_channel(ut, &root_cnode, slots, ipc_slots)?;
        let iomuxc_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    pac::iomuxc::IOMUXC::PADDR as _,
                    pac::iomuxc::IOMUXC::SIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");
        let iomuxc_mem = iomux_vspace.map_region(
            UnmappedMemoryRegion::new_device(iomuxc_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;
        let params = iomux::ProcParams {
            iomuxc: unsafe { pac::iomuxc::IOMUXC::from_vaddr(iomuxc_mem.vaddr() as _) },
            responder,
        };
        let stack_mem: UnmappedMemoryRegion<<resources::Iomux as ElfProc>::StackSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots).unwrap();
        let stack_mem =
            root_vspace.map_region(stack_mem, CapRights::RW, arch::vm_attributes::DEFAULT)?;
        let mut iomux_process = StandardProcess::new::<iomux::ProcParams<_>, _>(
            &mut iomux_vspace,
            iomux_cnode,
            stack_mem,
            &root_cnode,
            iomux_elf_data,
            params,
            ut, // ipc_buffer_ut
            ut, // tcb_ut
            slots,
            &tpa, // priority_authority
            None, // fault
        )?;

        //
        // drivers/tcpip setup
        //

        log::debug!("[root-task] Setting up tcpip driver");

        let (asid, asid_pool) = asid_pool.alloc();
        let vspace_slots: LocalCNodeSlots<ferros::arch::CodePageCount> = slots;
        let vspace_ut: LocalCap<Untyped<U16>> = ut;
        let mut tcpip_vspace = VSpace::new_from_elf::<resources::TcpIp>(
            retype(ut, slots)?, // paging_root
            asid,
            vspace_slots.weaken(), // slots
            vspace_ut.weaken(),    // paging_untyped
            tcpip_elf_data,
            slots, // page_slots
            ut,    // elf_writable_mem
            &user_image,
            &root_cnode,
            &mut scratch,
        )?;
        let (tcpip_cnode, tcpip_slots) = retype_cnode::<U12>(ut, slots)?;

        //
        // drivers/enet setup
        //

        log::debug!("[root-task] Setting up enet driver");

        let (asid, asid_pool) = asid_pool.alloc();
        let vspace_slots: LocalCNodeSlots<ferros::arch::CodePageCount> = slots;
        let vspace_ut: LocalCap<Untyped<U16>> = ut;
        let mut enet_vspace = VSpace::new_from_elf::<resources::Enet>(
            retype(ut, slots)?, // paging_root
            asid,
            vspace_slots.weaken(), // slots
            vspace_ut.weaken(),    // paging_untyped
            enet_elf_data,
            slots, // page_slots
            ut,    // elf_writable_mem
            &user_image,
            &root_cnode,
            &mut scratch,
        )?;
        let (enet_cnode, enet_slots) = retype_cnode::<U12>(ut, slots)?;
        let (slots_c, enet_slots) = enet_slots.alloc();
        let (enet_int_consumer, mut enet_int_consumer_token) =
            InterruptConsumer::new(ut, &mut irq_control, &root_cnode, slots, slots_c)?;
        //
        // shared setup between tcpip and enet drivers
        //

        // enet <- tcpip L2 frame consumer & enet IRQ waker
        let (enet_consumer, enet_producer_setup) = enet_int_consumer
            .add_queue::<IpcEthernetFrame, L2IpcQueueDepth, L2IpcQueuePageBits, _>(
                &mut enet_int_consumer_token,
                ut,
                &mut scratch,
                &mut enet_vspace,
                &root_cnode,
                slots,
                slots,
            )?;

        // tcpip -> enet L2 frame producer
        let (slots_p, tcpip_slots) = tcpip_slots.alloc();
        let tcpip_eth_producer = Producer::new(
            &enet_producer_setup,
            slots_p,
            &mut tcpip_vspace,
            &root_cnode,
            slots,
        )?;

        // tcpip <- enet L2 frame consumer
        let (slots_c, tcpip_slots) = tcpip_slots.alloc();
        let (
            tcpip_eth_consumer,
            _tcpip_eth_consumer_token,
            tcpip_eth_producer_setup,
            _tcpip_eth_waker_setup,
        ) = Consumer1::new::<L2IpcQueueDepth, L2IpcQueuePageBits, _>(
            ut,
            ut,
            &mut scratch,
            &mut tcpip_vspace,
            &root_cnode,
            slots,
            slots,
            slots,
            slots_c,
        )?;

        // enet -> tcpip L2 frame producer
        let (slots_p, enet_slots) = enet_slots.alloc();
        let enet_producer = Producer::new(
            &tcpip_eth_producer_setup,
            slots_p,
            &mut enet_vspace,
            &root_cnode,
            slots,
        )?;

        // tcpip <- console app UDP consumer & GPT IRQ waker
        let (slots_c, tcpip_slots) = tcpip_slots.alloc();
        let (tcpip_int_consumer, mut tcpip_int_consumer_token) =
            InterruptConsumer::new(ut, &mut irq_control, &root_cnode, slots, slots_c)?;
        let (tcpip_event_consumer, tcpip_event_producer_setup) = tcpip_int_consumer
            .add_queue::<IpcUdpTransmitBuffer, UdpIpcQueueDepth, UdpIpcQueuePageBits, _>(
            &mut tcpip_int_consumer_token,
            ut,
            &mut scratch,
            &mut tcpip_vspace,
            &root_cnode,
            slots,
            slots,
        )?;

        //
        // drivers/tcpip setup continued
        //

        let socket_buffer_mem_unmapped: UnmappedMemoryRegion<tcpip::RxTxSocketBufferSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots)?;
        let (mem_slots, _tcpip_slots) = tcpip_slots.alloc();
        let socket_buffer_mem = tcpip_vspace.map_region_and_move(
            socket_buffer_mem_unmapped,
            CapRights::RW,
            arch::vm_attributes::DEFAULT,
            &root_cnode,
            mem_slots,
        )?;
        let gpt_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    pac::gpt::GPT::PADDR as _,
                    pac::gpt::GPT::SIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");
        let gpt_mem = tcpip_vspace.map_region(
            UnmappedMemoryRegion::new_device(gpt_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;
        let params = tcpip::ProcParams {
            gpt: unsafe { pac::gpt::GPT::from_vaddr(gpt_mem.vaddr() as _) },
            frame_consumer: tcpip_eth_consumer,
            frame_producer: tcpip_eth_producer,
            event_consumer: tcpip_event_consumer,
            socket_buffer_mem,
            mac_addr: MAC_ADDRESS,
            ip_addr: IP_ADDRESS,
        };
        let stack_mem: UnmappedMemoryRegion<<resources::TcpIp as ElfProc>::StackSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots).unwrap();
        let stack_mem =
            root_vspace.map_region(stack_mem, CapRights::RW, arch::vm_attributes::DEFAULT)?;
        let mut tcpip_process = StandardProcess::new::<tcpip::ProcParams<_>, _>(
            &mut tcpip_vspace,
            tcpip_cnode,
            stack_mem,
            &root_cnode,
            tcpip_elf_data,
            params,
            ut, // ipc_buffer_ut
            ut, // tcb_ut
            slots,
            &tpa, // priority_authority
            None, // fault
        )?;

        //
        // drivers/enet setup continued
        //

        let enet_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    pac::enet::ENET::PADDR as _,
                    pac::enet::ENET::SIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");
        let enet_mem = enet_vspace.map_region(
            UnmappedMemoryRegion::new_device(enet_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;
        let dma_mem_unmapped: UnmappedMemoryRegion<enet::EthDmaMemSizeInBits, _> =
            UnmappedMemoryRegion::new(ut, slots)?;
        let (mem_slots, _enet_slots) = enet_slots.alloc();
        let dma_mem = enet_vspace.map_region_and_move(
            dma_mem_unmapped,
            CapRights::RW,
            // NOTE: driver expects uncached DMA memory for the time being
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
            &root_cnode,
            mem_slots,
        )?;
        let params = enet::ProcParams {
            enet: unsafe { pac::enet::ENET::from_vaddr(enet_mem.vaddr() as _) },
            consumer: enet_consumer,
            producer: enet_producer,
            dma_mem,
            mac_addr: MAC_ADDRESS,
        };
        let stack_mem: UnmappedMemoryRegion<<resources::Enet as ElfProc>::StackSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots).unwrap();
        let stack_mem =
            root_vspace.map_region(stack_mem, CapRights::RW, arch::vm_attributes::DEFAULT)?;
        let mut enet_process = StandardProcess::new::<enet::ProcParams<_>, _>(
            &mut enet_vspace,
            enet_cnode,
            stack_mem,
            &root_cnode,
            enet_elf_data,
            params,
            ut, // ipc_buffer_ut
            ut, // tcb_ut
            slots,
            &tpa, // priority_authority
            None, // fault
        )?;

        //
        // drivers/persistent-storage setup
        //

        log::debug!("[root-task] Setting up persistent-storage driver");

        let (asid, asid_pool) = asid_pool.alloc();
        let vspace_slots: LocalCNodeSlots<ferros::arch::CodePageCount> = slots;
        let vspace_ut: LocalCap<Untyped<U16>> = ut;
        let mut pstorage_vspace = VSpace::new_from_elf::<resources::PersistentStorage>(
            retype(ut, slots)?, // paging_root
            asid,
            vspace_slots.weaken(), // slots
            vspace_ut.weaken(),    // paging_untyped
            pstorage_elf_data,
            slots, // page_slots
            ut,    // elf_writable_mem
            &user_image,
            &root_cnode,
            &mut scratch,
        )?;
        let (pstorage_cnode, pstorage_slots) = retype_cnode::<U12>(ut, slots)?;
        let (ipc_slots, pstorage_slots) = pstorage_slots.alloc();
        let (pstorage_ipc_setup, responder) = call_channel(ut, &root_cnode, slots, ipc_slots)?;
        let (ipc_slots, pstorage_slots) = pstorage_slots.alloc();
        let iomux_caller = iomux_ipc_setup.create_caller(ipc_slots)?;
        let storage_buffer_unmapped: UnmappedMemoryRegion<
            persistent_storage::StorageBufferSizeBits,
            _,
        > = UnmappedMemoryRegion::new(ut, slots)?;
        let (mem_slots, pstorage_slots) = pstorage_slots.alloc();
        let storage_buffer = pstorage_vspace.map_region_and_move(
            storage_buffer_unmapped,
            CapRights::RW,
            arch::vm_attributes::DEFAULT,
            &root_cnode,
            mem_slots,
        )?;
        let scratchpad_buffer_unmapped: UnmappedMemoryRegion<
            persistent_storage::ScratchpadBufferSizeBits,
            _,
        > = UnmappedMemoryRegion::new(ut, slots)?;
        let (mem_slots, _pstorage_slots) = pstorage_slots.alloc();
        let scratchpad_buffer = pstorage_vspace.map_region_and_move(
            scratchpad_buffer_unmapped,
            CapRights::RW,
            arch::vm_attributes::DEFAULT,
            &root_cnode,
            mem_slots,
        )?;
        let spi1_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    pac::ecspi1::ECSPI1::PADDR as _,
                    pac::ecspi1::ECSPI1::SIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");
        let spi1_mem = pstorage_vspace.map_region(
            UnmappedMemoryRegion::new_device(spi1_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;
        let gpio3_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    pac::gpio::GPIO3::PADDR as _,
                    pac::gpio::GPIO3::SIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");
        let gpio3_mem = pstorage_vspace.map_region(
            UnmappedMemoryRegion::new_device(gpio3_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;
        let params = persistent_storage::ProcParams {
            spi: unsafe { pac::ecspi1::ECSPI1::from_vaddr(spi1_mem.vaddr() as _) },
            gpio3: unsafe { pac::gpio::GPIO3::from_vaddr(gpio3_mem.vaddr() as _) },
            iomux_caller,
            responder,
            storage_buffer,
            scratchpad_buffer,
        };
        let stack_mem: UnmappedMemoryRegion<
            <resources::PersistentStorage as ElfProc>::StackSizeBits,
            _,
        > = UnmappedMemoryRegion::new(ut, slots).unwrap();
        let stack_mem =
            root_vspace.map_region(stack_mem, CapRights::RW, arch::vm_attributes::DEFAULT)?;
        let mut pstorage_process = StandardProcess::new::<persistent_storage::ProcParams<_>, _>(
            &mut pstorage_vspace,
            pstorage_cnode,
            stack_mem,
            &root_cnode,
            pstorage_elf_data,
            params,
            ut, // ipc_buffer_ut
            ut, // tcb_ut
            slots,
            &tpa, // priority_authority
            None, // fault
        )?;

        //
        // applications/console setup
        //

        log::debug!("[root-task] Setting up console application");

        let (asid, _asid_pool) = asid_pool.alloc();
        let vspace_slots: LocalCNodeSlots<ferros::arch::CodePageCount> = slots;
        let vspace_ut: LocalCap<Untyped<U16>> = ut;
        let mut console_vspace = VSpace::new_from_elf::<resources::Console>(
            retype(ut, slots)?, // paging_root
            asid,
            vspace_slots.weaken(), // slots
            vspace_ut.weaken(),    // paging_untyped
            console_elf_data,
            slots, // page_slots
            ut,    // elf_writable_mem
            &user_image,
            &root_cnode,
            &mut scratch,
        )?;
        let (console_cnode, console_slots) = retype_cnode::<U12>(ut, slots)?;
        let (ipc_slots, console_slots) = console_slots.alloc();
        let storage_caller = pstorage_ipc_setup.create_caller(ipc_slots)?;
        let (slots_c, console_slots) = console_slots.alloc();
        let (int_consumer, _int_consumer_token) =
            InterruptConsumer::new(ut, &mut irq_control, &root_cnode, slots, slots_c)?;
        let uart1_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    pac::uart1::UART1::PADDR as _,
                    pac::uart1::UART1::SIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");
        let (slots_p, console_slots) = console_slots.alloc();
        let udp_producer = Producer::new(
            &tcpip_event_producer_setup,
            slots_p,
            &mut console_vspace,
            &root_cnode,
            slots,
        )?;
        let uart1_mem = console_vspace.map_region(
            UnmappedMemoryRegion::new_device(uart1_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;
        let console_buffer_unmapped: UnmappedMemoryRegion<console::ConsoleBufferSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots)?;
        let (mem_slots, _console_slots) = console_slots.alloc();
        let console_buffer = console_vspace.map_region_and_move(
            console_buffer_unmapped,
            CapRights::RW,
            arch::vm_attributes::DEFAULT,
            &root_cnode,
            mem_slots,
        )?;
        let params = console::ProcParams {
            uart: unsafe { pac::uart1::UART1::from_vaddr(uart1_mem.vaddr() as _) },
            int_consumer,
            storage_caller,
            udp_producer,
            console_buffer,
        };
        let stack_mem: UnmappedMemoryRegion<<resources::Console as ElfProc>::StackSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots).unwrap();
        let stack_mem =
            root_vspace.map_region(stack_mem, CapRights::RW, arch::vm_attributes::DEFAULT)?;
        let mut console_process = StandardProcess::new::<console::ProcParams<_>, _>(
            &mut console_vspace,
            console_cnode,
            stack_mem,
            &root_cnode,
            console_elf_data,
            params,
            ut, // ipc_buffer_ut
            ut, // tcb_ut
            slots,
            &tpa, // priority_authority
            None, // fault
        )?;
    });

    iomux_process.set_name("iomux");
    iomux_process.start()?;

    enet_process.set_name("enet-driver");
    enet_process.start()?;

    tcpip_process.set_name("tcpip-driver");
    tcpip_process.start()?;

    pstorage_process.set_name("persistent-storage");
    pstorage_process.start()?;

    console_process.set_name("console");
    console_process.start()?;

    // NOTE: we could stop the root-task here instead
    unsafe {
        loop {
            selfe_sys::seL4_Yield();
        }
    }
}
