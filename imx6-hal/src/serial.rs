// TODO
// - need to deal PINS/IOMUX, clocks, etc
// - Error type, read status bits
// - sealed Instance trait for peripherials, UART: Instance

use crate::pac::{typenum::U1, uart1::*};
use core::convert::Infallible;
use core::fmt;
use embedded_hal::serial;
use nb::block;

pub struct Serial<UART> {
    uart: UART,
}

impl Serial<UART1> {
    pub fn new(mut uart: UART1) -> Self {
        // TODO - do real init
        // https://github.com/torvalds/linux/blob/master/drivers/tty/serial/imx.c
        uart.ctl2
            .modify(Control2::SoftwareReset::Field::checked::<U1>());
        uart.ctl1
            .modify(Control1::RecvReadyInterrupt::Field::checked::<U1>());
        Serial { uart }
    }
}

impl serial::Read<u8> for Serial<UART1> {
    type Error = Infallible;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        if self.uart.stat2.is_set(Status2::RxDataReady::Set) {
            Ok(self.uart.rx.get_field(Rx::Data::Read).unwrap().val() as u8)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl serial::Write<u8> for Serial<UART1> {
    type Error = Infallible;

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        if self.uart.stat2.is_set(Status2::TxFifoEmpty::Set) {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn write(&mut self, byte: u8) -> nb::Result<(), Self::Error> {
        if self.uart.stat2.is_set(Status2::TxFifoEmpty::Set) {
            self.uart
                .tx
                .modify(Tx::Data::Field::new(byte as _).unwrap());
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl fmt::Write for Serial<UART1> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        use serial::Write;
        for b in s.bytes() {
            // Convert '\n' to '\r\n'
            if b as char == '\n' {
                block!(self.write('\r' as _)).ok();
            }
            block!(self.write(b)).map_err(|_| fmt::Error)?;
        }
        Ok(())
    }
}

/*
pub trait Instance: crate::Sealed + rcc::Enable + rcc::Reset + rcc::GetBusFreq {
    #[doc(hidden)]
    fn ptr() -> *const uart_base::RegisterBlock;
    #[doc(hidden)]
    fn set_stopbits(&self, bits: config::StopBits);
}
*/
