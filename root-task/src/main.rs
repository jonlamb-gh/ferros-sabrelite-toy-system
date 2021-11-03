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
use sabrelite_bsp::{debug_logger::DebugLogger, pac};
use typenum::*;

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

    let uts = alloc::ut_buddy(allocator.alloc_strong::<U20>(&mut ut_slots)?);

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
        let (proc_cnode, proc_slots) = retype_cnode::<U12>(ut, slots)?;
        let (ipc_slots, _proc_slots) = proc_slots.alloc();
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
            proc_cnode,
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
        // drivers/persistent-storage setup
        //

        log::debug!("[root-task] Setting up persistent-storage driver");

        let (asid, asid_pool) = asid_pool.alloc();
        let vspace_slots: LocalCNodeSlots<U16> = slots;
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
        let (proc_cnode, proc_slots) = retype_cnode::<U12>(ut, slots)?;
        let (ipc_slots, proc_slots) = proc_slots.alloc();
        let (pstorage_ipc_setup, responder) = call_channel(ut, &root_cnode, slots, ipc_slots)?;
        let (ipc_slots, proc_slots) = proc_slots.alloc();
        let iomux_caller = iomux_ipc_setup.create_caller(ipc_slots)?;
        let storage_buffer_unmapped: UnmappedMemoryRegion<
            persistent_storage::StorageBufferSizeBits,
            _,
        > = UnmappedMemoryRegion::new(ut, slots)?;
        let (mem_slots, proc_slots) = proc_slots.alloc();
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
        let (mem_slots, _proc_slots) = proc_slots.alloc();
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
            proc_cnode,
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
        let vspace_slots: LocalCNodeSlots<U16> = slots;
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
        let (proc_cnode, proc_slots) = retype_cnode::<U12>(ut, slots)?;
        let (ipc_slots, proc_slots) = proc_slots.alloc();
        let storage_caller = pstorage_ipc_setup.create_caller(ipc_slots)?;
        let (slots_c, _proc_slots) = proc_slots.alloc();
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
        let uart1_mem = console_vspace.map_region(
            UnmappedMemoryRegion::new_device(uart1_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT,
        )?;
        let params = console::ProcParams {
            uart: unsafe { pac::uart1::UART1::from_vaddr(uart1_mem.vaddr() as _) },
            storage_caller,
            int_consumer,
        };
        let stack_mem: UnmappedMemoryRegion<<resources::Console as ElfProc>::StackSizeBits, _> =
            UnmappedMemoryRegion::new(ut, slots).unwrap();
        let stack_mem =
            root_vspace.map_region(stack_mem, CapRights::RW, arch::vm_attributes::DEFAULT)?;
        let mut console_process = StandardProcess::new::<console::ProcParams<_>, _>(
            &mut console_vspace,
            proc_cnode,
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

    iomux_process.start()?;
    pstorage_process.start()?;
    console_process.start()?;

    // NOTE: we could stop the root-task here instead
    unsafe {
        loop {
            selfe_sys::seL4_Yield();
        }
    }
}
