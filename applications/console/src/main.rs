#![no_std]
#![no_main]

use core::fmt::Write as WriteFmt;
use ferros::{cap::role, debug_println};
use imx6_hal::embedded_hal::serial::Read;
use imx6_hal::serial::Serial;
extern crate selfe_runtime;

use console::ProcParams;

#[no_mangle]
pub extern "C" fn _start(params: ProcParams<role::Local>) -> ! {
    debug_println!("console process started, run 'telnet 0.0.0.0 8888' to connect");

    let serial = Serial::new(params.uart);

    params.int_consumer.consume(serial, move |mut serial| {
        if let Ok(b) = serial.read() {
            debug_println!("b={}", b as char);
            writeln!(serial, "echo back={}", b as char).unwrap();
        }
        serial
    })
}
