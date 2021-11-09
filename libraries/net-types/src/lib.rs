#![no_std]

use core::fmt;

mod frame;
mod udp_transmit_buffer;

pub use crate::frame::*;
pub use crate::udp_transmit_buffer::*;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct Port(pub u16);

impl From<u16> for Port {
    fn from(port: u16) -> Self {
        Port(port)
    }
}

impl Into<u16> for Port {
    fn into(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct EthernetAddress(pub [u8; 6]);

impl From<[u8; 6]> for EthernetAddress {
    fn from(octets: [u8; 6]) -> Self {
        EthernetAddress(octets)
    }
}

impl Into<[u8; 6]> for EthernetAddress {
    fn into(self) -> [u8; 6] {
        self.0
    }
}

impl fmt::Display for EthernetAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.0;
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
        )
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct Ipv4Address(pub [u8; 4]);

impl From<[u8; 4]> for Ipv4Address {
    fn from(octets: [u8; 4]) -> Self {
        Ipv4Address(octets)
    }
}

impl Into<[u8; 4]> for Ipv4Address {
    fn into(self) -> [u8; 4] {
        self.0
    }
}

impl fmt::Display for Ipv4Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let bytes = self.0;
        write!(f, "{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
    }
}
