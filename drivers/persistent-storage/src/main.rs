#![no_std]
#![no_main]

use selfe_runtime as _;

use crate::flash_controller::SpiNorFlashController;
use core::convert::TryInto;
use core::hash::{Hash, Hasher};
use ferros::cap::role;
use persistent_storage::{ProcParams, Request, Response, StorageBufferSizeBytes};
use sabrelite_bsp::imx6_hal::{gpio::GpioExt, spi::Spi};
use sabrelite_bsp::{
    debug_logger::DebugLogger,
    flash::{Flash, ERASE_SIZE_BYTES},
    pac::typenum::Unsigned,
};
use siphasher::sip::SipHasher;
use static_assertions::const_assert_eq;
use tickv::{TicKV, MAIN_KEY};

mod flash_controller;

static LOGGER: DebugLogger = DebugLogger;

const_assert_eq!(StorageBufferSizeBytes::USIZE, ERASE_SIZE_BYTES);

#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn _start(params: ProcParams<role::Local>) -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(DebugLogger::max_log_level_from_env()))
        .unwrap();

    log::debug!("[persistent-storage] process started",);

    log::debug!(
        "[persistent-storage] storage vaddr=0x{:X} size={}",
        params.storage_buffer.vaddr(),
        params.storage_buffer.size_bytes()
    );

    log::debug!(
        "[persistent-storage] scratchpad vaddr=0x{:X} size={}",
        params.scratchpad_buffer.vaddr(),
        params.scratchpad_buffer.size_bytes()
    );

    // Scratchpad mem to deal with flash sub-page size writes (read-modify-write)
    let mut scratchpad_buffer = params.scratchpad_buffer;
    let scratchpad_buffer_slice = scratchpad_buffer.as_mut_slice();

    // TickV expects an array ref
    let mut storage_buffer = params.storage_buffer;
    let storage_buffer_slice = storage_buffer.as_mut_slice();
    let storage_buffer_array: &mut [u8; ERASE_SIZE_BYTES] =
        storage_buffer_slice.try_into().unwrap();

    // Configure ECSPI1 IO
    let resp = params
        .iomux_caller
        .blocking_call(&iomux::Request::ConfigureEcSpi1)
        .unwrap();
    log::debug!("[persistent-storage] Configured ECSPI1 IO resp={:?}", resp);

    let gpio = params.gpio3.split();
    let spi_nor_cs_pin = gpio.bank3.p3_19.into_push_pull_output();
    let spi = Spi::new(params.spi);

    let spi_nor_flash = Flash::init(spi, spi_nor_cs_pin).unwrap();
    let flash = SpiNorFlashController::new(spi_nor_flash, scratchpad_buffer_slice).unwrap();

    let tickv = TicKV::<SpiNorFlashController, ERASE_SIZE_BYTES>::new(
        flash,
        storage_buffer_array,
        StorageBufferSizeBytes::USIZE,
    );

    let mut hasher = SipHasher::new();
    MAIN_KEY.hash(&mut hasher);
    tickv.initalise(hasher.finish()).unwrap();

    // Add a key
    let value: [u8; 32] = [0x23; 32];
    tickv.append_key(get_hashed_key(b"ONE"), &value).unwrap();

    // Get the same key back
    let mut buf: [u8; 32] = [0; 32];
    tickv.get_key(get_hashed_key(b"ONE"), &mut buf).unwrap();
    log::debug!("---> {:X?}", buf);

    // TODO
    unsafe {
        loop {
            selfe_sys::seL4_Yield();
        }
    }
}

fn get_hashed_key(unhashed_key: &[u8]) -> u64 {
    let mut hash_function = SipHasher::new();
    unhashed_key.hash(&mut hash_function);
    hash_function.finish()
}
