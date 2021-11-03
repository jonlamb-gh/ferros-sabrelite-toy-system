#![no_std]

use ferros::cap::{role, CNodeRole};
use ferros::userland::{Caller, Responder, RetypeForSetup};
use ferros::vspace::{shared_status, MappedMemoryRegion};
use heapless::String;
use sabrelite_bsp::pac::{
    ecspi1::ECSPI1,
    gpio::GPIO3,
    typenum::{op, U1, U12},
};
pub use tickv::{success_codes::SuccessCode, ErrorCode};

pub const MAX_KEY_SIZE: usize = 32;
pub type Key = String<MAX_KEY_SIZE>;

pub const MAX_VALUE_SIZE: usize = 256;
pub type Value = String<MAX_VALUE_SIZE>;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    AppendKey(Key, Value),
    Get(Key),
    InvalidateKey(Key),
    GarbageCollect,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    KeyAppended(SuccessCode),
    Value(Value),
    KeyInvalidated(SuccessCode),
    GarbageCollected(usize),
}

/// 4K buffer for persistent storage in flash (1 sector)
pub type StorageBufferSizeBits = U12;
pub type StorageBufferSizeBytes = op! { U1 << StorageBufferSizeBits };

/// 4K scratchpad buffer
pub type ScratchpadBufferSizeBits = U12;
pub type ScratchpadBufferSizeBytes = op! { U1 << ScratchpadBufferSizeBits };

#[repr(C)]
pub struct ProcParams<Role: CNodeRole> {
    pub spi: ECSPI1,
    pub gpio3: GPIO3,
    pub iomux_caller: Caller<iomux::Request, iomux::Response, Role>,
    pub responder: Responder<Request, Result<Response, ErrorCode>, Role>,
    pub storage_buffer: MappedMemoryRegion<StorageBufferSizeBits, shared_status::Exclusive>,
    pub scratchpad_buffer: MappedMemoryRegion<ScratchpadBufferSizeBits, shared_status::Exclusive>,
}

impl RetypeForSetup for ProcParams<role::Local> {
    type Output = ProcParams<role::Child>;
}
