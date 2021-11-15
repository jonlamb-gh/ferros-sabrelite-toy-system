use crate::asm;
use crate::pac::uart1::*;
use core::convert::Infallible;
use core::fmt;
use embedded_hal::serial;
use nb::block;

/// Interrupt events
pub enum Event {
    Receive,
}

pub struct Serial<UART> {
    uart: UART,
}

impl Serial<UART1> {
    pub fn new(mut uart: UART1) -> Self {
        uart.ctl1.modify(Control1::Enable::Clear);
        uart.ctl1.modify(Control1::Enable::Set);
        uart.ctl2.modify(Control2::SoftwareReset::Clear);
        while uart.test.is_set(Test::SoftwareReset::Set) {
            asm::nop();
        }
        uart.ctl2.modify(
            Control2::RxEnable::Set
                + Control2::TxEnable::Set
                + Control2::WordSize::Set
                + Control2::IgnoreRTS::Set,
        );
        uart.ctl3.modify(
            Control3::RxdMuxSelect::Set
                + Control3::AutoBaudOff::Set
                + Control3::RingIndicator::Set
                + Control3::DataCarrierDetect::Set
                + Control3::DataSetReady::Set,
        );
        uart.ctl4.modify(Control4::CtsTriggerLevel::RxFifoChars32);
        uart.fctl.modify(
            FifoControl::RxTriggerLevel::TlRxFifoChars1
                + FifoControl::DceDteMode::DceMode
                + FifoControl::RefFreqDiv::DivideBy2
                + FifoControl::TxTriggerLevel::TlTxFifoChars2OrLess,
        );
        uart.bir
            .modify(BrmInc::IncNumerator::Field::new(0xF).unwrap());
        uart.bmr
            .modify(BrmMod::ModDenominator::Field::new(0x15B).unwrap());
        Serial { uart }
    }

    pub fn listen(&mut self, event: Event) {
        match event {
            Event::Receive => self.uart.ctl1.modify(Control1::RecvReadyInterrupt::Set),
        }
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
            if b == b'\n' {
                block!(self.write(b'\r')).ok();
            }
            block!(self.write(b)).map_err(|_| fmt::Error)?;
        }
        Ok(())
    }
}
