#![no_std]
#![no_main]

use selfe_runtime as _;

use crate::ipc_phy_dev::IpcPhyDevice;
use ferros::cap::role;
use sabrelite_bsp::debug_logger::DebugLogger;
use smoltcp::iface::{EthernetInterface, EthernetInterfaceBuilder, NeighborCache, Routes};
use smoltcp::socket::{SocketHandle, SocketSet, UdpPacketMetadata, UdpSocket, UdpSocketBuffer};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{IpCidr, IpEndpoint};
use tcpip::ProcParams;

mod ipc_phy_dev;

/// Maximum number of ARP (Neighbor) cache entries
/// available in the storage
const MAX_ARP_ENTRIES: usize = 32;

const EPHEMERAL_PORT: u16 = 49152;

static LOGGER: DebugLogger = DebugLogger;

#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn _start(params: ProcParams<role::Local>) -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(DebugLogger::max_log_level_from_env()))
        .unwrap();

    log::debug!("[tcpip-driver] Process started");

    let ipc_phy = IpcPhyDevice {
        consumer: params.frame_consumer,
        producer: params.frame_producer,
    };

    // Build the IP stack
    let ip_addr = IpCidr::new(smoltcp::wire::Ipv4Address(params.ip_addr.into()).into(), 24);
    let mut ip_addrs = [ip_addr];
    let mut neighbor_storage = [None; MAX_ARP_ENTRIES];
    let neighbor_cache = NeighborCache::new(&mut neighbor_storage[..]);

    let ethernet_addr = smoltcp::wire::EthernetAddress(params.mac_addr.into());
    let mut routes_storage = [None; 4];
    let routes = Routes::new(&mut routes_storage[..]);

    let mut iface = EthernetInterfaceBuilder::new(ipc_phy)
        .ethernet_addr(ethernet_addr)
        .ip_addrs(&mut ip_addrs[..])
        .neighbor_cache(neighbor_cache)
        .routes(routes)
        .finalize();

    // Only capacity for a single UDP socket
    let mut sockets_storage = [None];
    let mut sockets = SocketSet::new(&mut sockets_storage[..]);

    // Split up the memory for socket rx/tx buffers
    let socket_mem = params.socket_buffer_mem;
    socket_mem.flush().unwrap();
    let (mut rx_mem, mut tx_mem) = socket_mem.split().unwrap();

    let mut rx_meta = [UdpPacketMetadata::EMPTY];
    let mut tx_meta = [UdpPacketMetadata::EMPTY];
    let udp_socket = UdpSocket::new(
        UdpSocketBuffer::new(&mut rx_meta[..], rx_mem.as_mut_slice()),
        UdpSocketBuffer::new(&mut tx_meta[..], tx_mem.as_mut_slice()),
    );

    let udp_handle = sockets.add(udp_socket);

    // The UDP handle is used to fulfill transmits only
    // so we can bind it now to an arbitrary local port
    sockets
        .get::<UdpSocket>(udp_handle)
        .bind(EPHEMERAL_PORT)
        .unwrap();

    log::debug!(
        "[tcpip-driver] TCP/IP stack is up IP={} MAC={}",
        params.ip_addr,
        params.mac_addr
    );

    let mut mock_timer = 0_i64;
    loop {
        // TODO TIMER/IRQ
        for _ in 0..10 {
            unsafe { selfe_sys::seL4_Yield() };
        }

        if let Err(e) = iface.poll(&mut sockets, Instant::from_millis(mock_timer)) {
            log::trace!("[tcpip-driver] {:?}", e);
        }

        mock_timer = mock_timer.wrapping_add(1);
    }
}
