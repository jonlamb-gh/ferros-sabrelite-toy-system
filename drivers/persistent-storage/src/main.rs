#![no_std]
#![no_main]

use selfe_runtime as _;

use ferros::cap::role;
use persistent_storage::{ProcParams, Request, Response};
use sabrelite_bsp::imx6_hal::{gpio::GpioExt, spi::Spi};
use sabrelite_bsp::{
    debug_logger::DebugLogger,
    flash::{Flash, PAGE_SIZE_BYTES},
};

static LOGGER: DebugLogger = DebugLogger;

#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn _start(params: ProcParams<role::Local>) -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        .unwrap();

    log::trace!(
        "[persistent-storage] process started, storage vaddr=0x{:X} size={}",
        params.storage_buffer.vaddr(),
        params.storage_buffer.size_bytes()
    );

    // Configure ECSPI1 IO
    let resp = params
        .iomux_caller
        .blocking_call(&iomux::Request::ConfigureEcSpi1)
        .unwrap();
    log::trace!("[persistent-storage] Configured ECSPI1 IO resp={:?}", resp);

    let gpio = params.gpio3.split();
    let spi_nor_cs_pin = gpio.bank3.p3_19.into_push_pull_output();
    let spi = Spi::new(params.spi);

    let mut flash = Flash::init(spi, spi_nor_cs_pin).unwrap();

    // TODO
    unsafe {
        loop {
            selfe_sys::seL4_Yield();
        }
    }
}
