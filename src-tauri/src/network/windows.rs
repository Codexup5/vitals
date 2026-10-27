//! The Windows side of the network: the adapters from IP Helper, and ICMP echo without raw sockets or admin rights.
#![allow(
    unsafe_code,
    reason = "IP Helper calls of the network screen live in this file"
)]

use std::net::Ipv4Addr;
use std::time::Duration;

use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, HANDLE, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
    GetAdaptersAddresses, GetIpNetTable2, ICMP_ECHO_REPLY, IP_ADAPTER_ADDRESSES_LH,
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, MIB_IPNET_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{
    AF_INET, GetNameInfoW, NI_MAXHOST, NI_NAMEREQD, NlnsIncomplete, NlnsUnreachable, SOCKADDR,
    SOCKADDR_IN, SOCKET_ADDRESS, WSADATA, WSAStartup, socklen_t,
};

use super::RawAdapter;
use super::devices::Neighbor;

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
        let unicast = unsafe { adapter.FirstUnicastAddress.as_ref() };
        // SAFETY: as above, its socket address lives in `buffer`.
        let address = unicast.and_then(|unicast| unsafe { ipv4_of(&unicast.Address) });
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
            prefix_len: unicast.map(|unicast| unicast.OnLinkPrefixLength),
            gateway,
            // SAFETY: Length and IfIndex are the plain view of the leading union.
            if_index: unsafe { adapter.Anonymous1.Anonymous.IfIndex },
            mac: (adapter.PhysicalAddressLength == 6)
                .then(|| adapter.PhysicalAddress[..6].try_into().ok())
                .flatten(),
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

/// The neighbours Windows knows on one interface: the IPv4 entries of its neighbour (ARP) cache with a MAC.
pub fn neighbors(if_index: u32) -> Vec<Neighbor> {
    let mut table: *mut MIB_IPNET_TABLE2 = std::ptr::null_mut();
    // SAFETY: `table` is a valid out pointer; the table is freed below with FreeMibTable.
    let answer = unsafe { GetIpNetTable2(AF_INET, &raw mut table) };
    if answer != NO_ERROR || table.is_null() {
        tracing::warn!("GetIpNetTable2 failed: {answer:?}");
        return Vec::new();
    }
    // SAFETY: the call filled `table` with NumEntries rows that follow its header.
    let rows = unsafe {
        std::slice::from_raw_parts(
            (*table).Table.as_ptr(),
            usize::try_from((*table).NumEntries).unwrap_or(0),
        )
    };
    let neighbors = rows
        .iter()
        .filter(|row| {
            row.InterfaceIndex == if_index
                && row.State != NlnsUnreachable
                && row.State != NlnsIncomplete
                && row.PhysicalAddressLength == 6
        })
        .filter_map(|row| {
            // SAFETY: an AF_INET row holds the Ipv4 view of the union.
            let inet = unsafe { row.Address.Ipv4 };
            // SAFETY: S_addr is the plain u32 view of the address union.
            let raw = unsafe { inet.sin_addr.S_un.S_addr };
            let mac: [u8; 6] = row.PhysicalAddress[..6].try_into().ok()?;
            Some(Neighbor {
                ip: Ipv4Addr::from(raw.to_ne_bytes()),
                mac,
            })
        })
        .collect();
    // SAFETY: the table came from GetIpNetTable2 and is freed once, here.
    unsafe { FreeMibTable(table.cast()) };
    neighbors
}

/// Winsock must be started once in the process before a name lookup.
fn start_winsock() -> bool {
    static STARTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *STARTED.get_or_init(|| {
        let mut data = WSADATA::default();
        // SAFETY: `data` is a live WSADATA the call fills; version 2.2.
        unsafe { WSAStartup(0x0202, &raw mut data) == 0 }
    })
}

/// The name a host answers to: DNS of the router, then `LLMNR` and `NetBIOS`, as Windows resolves it.
pub fn host_name(ip: Ipv4Addr) -> Option<String> {
    if !start_winsock() {
        return None;
    }
    let mut address = SOCKADDR_IN {
        sin_family: AF_INET,
        ..SOCKADDR_IN::default()
    };
    address.sin_addr.S_un.S_addr = u32::from_ne_bytes(ip.octets());
    let mut name = vec![0u16; usize::try_from(NI_MAXHOST).unwrap_or(1025)];
    // SAFETY: `address` is a valid SOCKADDR_IN of the length passed, `name` a live buffer the call fills.
    let answer = unsafe {
        GetNameInfoW(
            (&raw const address).cast::<SOCKADDR>(),
            socklen_t(i32::try_from(size_of::<SOCKADDR_IN>()).unwrap_or(16)),
            Some(&mut name),
            None,
            i32::try_from(NI_NAMEREQD).unwrap_or(4),
        )
    };
    if answer != 0 {
        return None;
    }
    let length = name
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(name.len());
    let name = String::from_utf16_lossy(&name[..length]);
    (!name.is_empty() && name != ip.to_string()).then_some(name)
}
