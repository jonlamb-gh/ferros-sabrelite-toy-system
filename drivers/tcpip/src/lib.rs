#![no_std]

use ferros::cap::{role, CNodeRole};
use ferros::userland::{Consumer1, Producer, RetypeForSetup};
use net_types::IpcEthernetFrame;

#[repr(C)]
pub struct ProcParams<Role: CNodeRole> {
    /// Consumer of Ethernet frames from a L2 driver
    pub frame_consumer: Consumer1<Role, IpcEthernetFrame>,

    /// Producer of Ethernet frames destined to a L2 driver
    pub frame_producer: Producer<Role, IpcEthernetFrame>,
}

impl RetypeForSetup for ProcParams<role::Local> {
    type Output = ProcParams<role::Child>;
}
