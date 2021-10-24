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
use selfe_arc;
use typenum::*;

use console;

extern "C" {
    static _selfe_arc_data_start: u8;
    static _selfe_arc_data_end: usize;
}

mod resources {
    include! {concat!(env!("OUT_DIR"), "/resources.rs")}
}

fn main() {
    let raw_bootinfo = unsafe { &*sel4_start::BOOTINFO };
    run(raw_bootinfo).expect("Failed to run root task setup");
}

fn run(raw_bootinfo: &'static selfe_sys::seL4_BootInfo) -> Result<(), TopLevelError> {
    let (allocator, mut dev_allocator) = micro_alloc::bootstrap_allocators(&raw_bootinfo)?;
    let mut allocator = WUTBuddy::from(allocator);

    let (root_cnode, local_slots) = root_cnode(&raw_bootinfo);
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
        &raw_bootinfo,
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
    let console_elf_data = archive
        .file(resources::Console::IMAGE_NAME)
        .expect("Can't find console image in arc");

    debug_println!("Binary found, size is {}", console_elf_data.len());
    debug_println!("*********************************\n");

    let uts = alloc::ut_buddy(allocator.alloc_strong::<U20>(&mut ut_slots)?);

    smart_alloc!(|slots: local_slots, ut: uts| {
        let (asid_pool, _asid_control) = asid_control.allocate_asid_pool(ut, slots)?;
        let (console_asid, _asid_pool) = asid_pool.alloc();

        let vspace_slots: LocalCNodeSlots<U16> = slots;
        let vspace_ut: LocalCap<Untyped<U16>> = ut;

        let ut_for_scratch: LocalCap<Untyped<U12>> = ut;
        let sacrificial_page = ut_for_scratch.retype(slots)?;
        let reserved_for_scratch = root_vspace.reserve(sacrificial_page)?;
        let mut scratch = reserved_for_scratch.as_scratch(&mut root_vspace).unwrap();

        let mut console_vspace = VSpace::new_from_elf::<resources::Console>(
            retype(ut, slots)?, // paging_root
            console_asid,
            vspace_slots.weaken(), // slots
            vspace_ut.weaken(),    // paging_untyped
            &console_elf_data,
            slots, // page_slots
            ut,    // elf_writable_mem
            &user_image,
            &root_cnode,
            &mut scratch,
        )?;

        let (console_cnode, console_slots) = retype_cnode::<U12>(ut, slots)?;

        let (slots_c, _console_slots) = console_slots.alloc();
        let (int_consumer, _int_consumer_token) =
            InterruptConsumer::new(ut, &mut irq_control, &root_cnode, slots, slots_c)?;

        let uart1_ut = dev_allocator
            .get_untyped_by_address_range_slot_infallible(
                PageAlignedAddressRange::new_by_size(
                    imx6_hal::pac::uart1::UART1::PADDR as _,
                    arch::PageBytes::USIZE,
                )?,
                slots,
            )?
            .as_strong::<arch::PageBits>()
            .expect("Device untyped was not the right size!");

        let uart1_mem = console_vspace.map_region(
            UnmappedMemoryRegion::new_device(uart1_ut, slots)?,
            CapRights::RW,
            arch::vm_attributes::DEFAULT & !arch::vm_attributes::PAGE_CACHEABLE,
        )?;

        let params = console::ProcParams {
            uart: unsafe { imx6_hal::pac::uart1::UART1::from_vaddr(uart1_mem.vaddr() as _) },
            int_consumer,
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

    console_process.start()?;

    unsafe {
        loop {
            selfe_sys::seL4_Yield();
        }
    }
}
