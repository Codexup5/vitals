//! The Windows side of the network: the adapters from IP Helper, and ICMP echo without raw sockets or admin rights.
#![allow(
    unsafe_code,
    reason = "IP Helper calls of the network screen live in this file"
)]

use std::net::Ipv4Addr;
use std::time::Duration;

use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, HANDLE, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
    GetAdaptersAddresses, ICMP_ECHO_REPLY, IP_ADAPTER_ADDRESSES_LH, IcmpCloseHandle,
    IcmpCreateFile, IcmpSendEcho,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR, SOCKADDR_IN, SOCKET_ADDRESS};

use super::RawAdapter;

/// `IF_TYPE_ETHERNET_CSMACD` and `IF_TYPE_IEEE80211`
const ETHERNET: u32 = 6;
const WIFI: u32 = 71;
/// `IP_SUCCESS` in `ICMP_ECHO_REPLY.Status`
const IP_SUCCESS: u32 = 0;

/// The IPv4 address a socket address holds, if it is one.
///
/// # Safety
///
/// `address.lpSockaddr` is null or points to a socket address of `address.iSockaddrLength` bytes that outlives the
/// call.
unsafe fn ipv4_of(address: &SOCKET_ADDRESS) -> Option<Ipv4Addr> {
    let socket: *const SOCKADDR = address.lpSockaddr;
    if socket.is_null() || usize::try_from(address.iSockaddrLength).ok()? < size_of::<SOCKADDR_IN>()
    {
        return None;
    }
    // SAFETY: not null, and long enough for SOCKADDR_IN, as the caller promises.
    let family = unsafe { (*socket).sa_family };
    if family != AF_INET {
        return None;
    }
    // SAFETY: an AF_INET socket address is a SOCKADDR_IN; IP Helper aligns it, the read does not rely on that.
    let inet = unsafe { socket.cast::<SOCKADDR_IN>().read_unaligned() };
    // SAFETY: S_addr is the plain u32 view of the union.
    let raw = unsafe { inet.sin_addr.S_un.S_addr };
    Some(Ipv4Addr::from(raw.to_ne_bytes()))
}

fn wide(text: windows::core::PWSTR) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: IP Helper gives terminated wide strings that live in its buffer, alive while we read.
    unsafe { text.to_string() }.unwrap_or_default()
}

/// The adapters that are up, as IP Helper lists them.
pub fn adapters() -> Vec<RawAdapter> {
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let family = u32::from(AF_INET.0);
    // u64 elements keep the buffer aligned for IP_ADAPTER_ADDRESSES_LH
    let mut buffer: Vec<u64> = vec![0; 2048];
    let mut size = u32::try_from(buffer.len() * 8).unwrap_or(0);
    let mut answer = NO_ERROR.0;
    for _ in 0..3 {
        // SAFETY: `buffer` holds `size` bytes, aligned for the structure, and outlives the call.
        answer = unsafe {
            GetAdaptersAddresses(
                family,
                flags,
                None,
                Some(buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>()),
                &raw mut size,
            )
        };
        if answer != ERROR_BUFFER_OVERFLOW.0 {
            break;
        }
        buffer = vec![0; usize::try_from(size).unwrap_or(0) / 8 + 1];
    }
    if answer != NO_ERROR.0 {
        tracing::warn!("GetAdaptersAddresses failed: {answer}");
        return Vec::new();
    }
    let mut adapters = Vec::new();
    let mut next = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    while !next.is_null() {
        // SAFETY: the list lives in `buffer`, which is alive and was filled by the call above.
        let adapter = unsafe { &*next };
        next = adapter.Next;
        if adapter.OperStatus != IfOperStatusUp {
            continue;
        }
        // SAFETY: the first unicast and gateway entries are null or live in `buffer`.
        let address = unsafe { adapter.FirstUnicastAddress.as_ref() }
            // SAFETY: as above, their socket addresses live in `buffer`.
            .and_then(|unicast| unsafe { ipv4_of(&unicast.Address) });
        // SAFETY: as above.
        let gateway = unsafe { adapter.FirstGatewayAddress.as_ref() }
            // SAFETY: as above.
            .and_then(|gateway| unsafe { ipv4_of(&gateway.Address) });
        adapters.push(RawAdapter {
            name: wide(adapter.FriendlyName),
            description: wide(adapter.Description),
            if_type: adapter.IfType,
            link_bps: adapter.TransmitLinkSpeed,
            address,
            gateway,
        });
    }
    adapters
}

pub const fn is_ethernet(if_type: u32) -> Option<bool> {
    match if_type {
        ETHERNET => Some(true),
        WIFI => Some(false),
        _ => None,
    }
}

/// An ICMP echo handle, closed when dropped.
pub struct Pinger(HANDLE);

impl Pinger {
    pub fn open() -> Option<Self> {
        // SAFETY: IcmpCreateFile takes no arguments; the handle is closed in Drop.
        unsafe { IcmpCreateFile() }.ok().map(Self)
    }

    /// One echo: the round trip in milliseconds, or none when no reply came in time.
    pub fn ping(&self, host: Ipv4Addr, timeout: Duration) -> Option<u32> {
        let payload = [0x56u8; 32];
        // The reply holds the structure, the echoed payload and room for an ICMP error
        let mut reply = vec![0u64; (size_of::<ICMP_ECHO_REPLY>() + payload.len() + 8) / 8 + 2];
        let reply_size = u32::try_from(reply.len() * 8).unwrap_or(0);
        let timeout = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: the payload and the reply buffer outlive the call, their sizes are the ones passed.
        let count = unsafe {
            IcmpSendEcho(
                self.0,
                u32::from_ne_bytes(host.octets()),
                payload.as_ptr().cast(),
                u16::try_from(payload.len()).unwrap_or(0),
                None,
                reply.as_mut_ptr().cast(),
                reply_size,
                timeout,
            )
        };
        if count == 0 {
            return None;
        }
        // SAFETY: a non-zero count means the buffer starts with a filled ICMP_ECHO_REPLY, aligned by its u64s.
        let first = unsafe { &*reply.as_ptr().cast::<ICMP_ECHO_REPLY>() };
        (first.Status == IP_SUCCESS).then_some(first.RoundTripTime)
    }
}

impl Drop for Pinger {
    fn drop(&mut self) {
        // SAFETY: the handle came from IcmpCreateFile and is closed once, here.
        if let Err(error) = unsafe { IcmpCloseHandle(self.0) } {
            tracing::warn!("could not close the ICMP handle: {error}");
        }
    }
}
