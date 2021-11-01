#![no_std]

use ferros::cap::{role, CNodeRole};
use ferros::userland::{Caller, RetypeForSetup};
use ferros::vspace::{shared_status, MappedMemoryRegion};
use sabrelite_bsp::pac::{
    ecspi1::ECSPI1,
    gpio::GPIO3,
    typenum::{op, U1, U12},
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Request {
    Todo,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Response {
    Todo,
}

/// 4K buffer for persistent storage in flash
pub type StorageBufferSizeBits = U12;
pub type StorageBufferSizeBytes = op! { U1 << StorageBufferSizeBits };

#[repr(C)]
pub struct ProcParams<Role: CNodeRole> {
    pub spi: ECSPI1,
    pub gpio3: GPIO3,
    pub iomux_caller: Caller<iomux::Request, iomux::Response, Role>,
    pub storage_buffer: MappedMemoryRegion<StorageBufferSizeBits, shared_status::Exclusive>,
    // TODO responder for persistent key/val stuff
}

impl RetypeForSetup for ProcParams<role::Local> {
    type Output = ProcParams<role::Child>;
}
