#![no_std]

use ferros::cap::{role, CNodeRole};
use ferros::userland::{Caller, InterruptConsumer, RetypeForSetup};
use sabrelite_bsp::pac::uart1::{self, UART1};

/// Expected badge value on IRQ notifications
pub type IrqBadgeBits = uart1::Irq;

#[repr(C)]
pub struct ProcParams<Role: CNodeRole> {
    pub uart: UART1,
    pub storage_caller: Caller<
        persistent_storage::Request,
        Result<persistent_storage::Response, persistent_storage::ErrorCode>,
        Role,
    >,
    pub int_consumer: InterruptConsumer<uart1::Irq, Role>,
}

impl RetypeForSetup for ProcParams<role::Local> {
    type Output = ProcParams<role::Child>;
}
