#![no_std]
#![no_main]

use selfe_runtime as _;

use ferros::cap::role;
use sabrelite_bsp::debug_logger::DebugLogger;
use tcpip::ProcParams;

static LOGGER: DebugLogger = DebugLogger;

// TODO - this is just a stub for now, eventually will host smoltcp stack

#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn _start(params: ProcParams<role::Local>) -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Trace))
        // TODO
        //.map(|()| log::set_max_level(DebugLogger::max_log_level_from_env()))
        .unwrap();

    log::debug!("[tcpip-driver] Process started");

    loop {
        unsafe { selfe_sys::seL4_Yield() };
    }
}
