//! The devices of the local network, without logging in to the router: a ping sweep of the subnet fills Windows'
//! neighbour cache, the cache gives each device's MAC, the IEEE registry its maker, Windows' resolver its name. A
//! device first seen after the panel's first scan is new for a week.

use std::net::Ipv4Addr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use vitals_ipc::{Device, DeviceRole};

/// A neighbour from Windows' cache
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Neighbor {
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
}

/// What a scan found of one device
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub mac: [u8; 6],
    pub ip: Ipv4Addr,
    pub name: Option<String>,
}

/// A device as the database keeps it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub mac: String,
    pub ip: String,
    pub name: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
}

/// The widest subnet the sweep covers: a /22 is 1022 hosts; a wider one is read from the cache only
const WIDEST_PREFIX: u8 = 22;
/// Narrower than /30 has no room for another device
const NARROWEST_PREFIX: u8 = 30;
/// A device is new this long after it first showed up
pub const NEW_SECONDS: i64 = 7 * 24 * 60 * 60;

const REGISTRY: &str = include_str!("../../data/oui.tsv");

/// The registry as sorted (prefix, maker) pairs, read once.
fn registry() -> &'static [(u32, &'static str)] {
    static TABLE: OnceLock<Vec<(u32, &'static str)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        REGISTRY
            .lines()
            .filter_map(|line| {
                let (prefix, maker) = line.split_once('\t')?;
                Some((u32::from_str_radix(prefix, 16).ok()?, maker))
            })
            .collect()
    })
}

/// A MAC with the locally administered bit set was made up by the device, not assigned by a maker.
pub const fn is_random(mac: [u8; 6]) -> bool {
    mac[0] & 0x02 != 0
}

/// The maker of a network card by its first three bytes.
pub fn maker_of(mac: [u8; 6]) -> Option<&'static str> {
    if is_random(mac) {
        return None;
    }
    let prefix = u32::from_be_bytes([0, mac[0], mac[1], mac[2]]);
    let table = registry();
    table
        .binary_search_by_key(&prefix, |(key, _)| *key)
        .ok()
        .and_then(|index| table.get(index))
        .map(|(_, maker)| *maker)
}

pub fn mac_text(mac: [u8; 6]) -> String {
    mac.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn mac_of(text: &str) -> Option<[u8; 6]> {
    let bytes: Vec<u8> = text
        .split(':')
        .map(|part| u8::from_str_radix(part, 16).ok())
        .collect::<Option<_>>()?;
    bytes.try_into().ok()
}

fn mask(prefix_len: u8) -> u32 {
    u32::MAX
        .checked_shl(32 - u32::from(prefix_len))
        .unwrap_or(0)
}

/// The hosts of the subnet, without the network and broadcast addresses; none for a subnet too wide to sweep.
pub fn subnet_hosts(address: Ipv4Addr, prefix_len: u8) -> Vec<Ipv4Addr> {
    if !(WIDEST_PREFIX..=NARROWEST_PREFIX).contains(&prefix_len) {
        return Vec::new();
    }
    let mask = mask(prefix_len);
    let network = u32::from(address) & mask;
    let broadcast = network | !mask;
    (network + 1..broadcast).map(Ipv4Addr::from).collect()
}

/// A neighbour of this subnet with a real MAC: not a multicast or broadcast entry.
pub fn is_device(neighbor: &Neighbor, address: Ipv4Addr, prefix_len: u8) -> bool {
    let mask = mask(prefix_len);
    let in_subnet = u32::from(neighbor.ip) & mask == u32::from(address) & mask;
    let is_broadcast = u32::from(neighbor.ip) | mask == u32::MAX;
    let is_group_mac = neighbor.mac[0] & 0x01 != 0;
    in_subnet && !is_broadcast && !is_group_mac && neighbor.mac != [0; 6]
}

/// The screen's devices: what the database knows, with the maker, the role and whether it is new or online.
pub fn devices_of(
    known: &[Known],
    this: Option<&str>,
    gateway: Option<&str>,
    now: i64,
) -> Vec<Device> {
    let baseline = known.iter().map(|device| device.first_seen).min();
    let last_scan = known.iter().map(|device| device.last_seen).max();
    let mut devices: Vec<Device> = known
        .iter()
        .map(|device| {
            let mac = mac_of(&device.mac).unwrap_or_default();
            Device {
                mac: device.mac.clone(),
                ip: device.ip.clone(),
                name: device.name.clone(),
                maker: maker_of(mac).map(str::to_owned),
                is_random_mac: is_random(mac),
                role: if this == Some(device.mac.as_str()) {
                    DeviceRole::This
                } else if gateway == Some(device.ip.as_str()) {
                    DeviceRole::Gateway
                } else {
                    DeviceRole::Other
                },
                first_seen: device.first_seen,
                last_seen: device.last_seen,
                is_online: Some(device.last_seen) == last_scan,
                is_new: baseline.is_some_and(|baseline| device.first_seen > baseline)
                    && now - device.first_seen < NEW_SECONDS,
            }
        })
        .collect();
    devices.sort_by_key(|device| {
        (
            !device.is_online,
            device.ip.parse::<Ipv4Addr>().map_or(u32::MAX, u32::from),
        )
    });
    devices
}

/// One scan at a time: the hourly one of the sampler and the button share it.
#[derive(Debug, Default)]
pub struct DevicesJob {
    pub is_running: AtomicBool,
}

impl DevicesJob {
    /// Runs `scan` unless one runs; whether it ran.
    pub fn run_once(&self, scan: impl FnOnce()) -> bool {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return false;
        }
        scan();
        self.is_running.store(false, Ordering::SeqCst);
        true
    }
}

/// Sweeps the subnet of the way out and reads who answered: this PC first, then its neighbours with their names.
#[cfg(windows)]
pub fn scan() -> Vec<Seen> {
    use super::windows::{Pinger, host_name, neighbors};

    let Some(adapter) = super::windows::adapters()
        .into_iter()
        .find(|adapter| adapter.gateway.is_some() && adapter.address.is_some())
    else {
        return Vec::new();
    };
    let (Some(address), Some(prefix_len)) = (adapter.address, adapter.prefix_len) else {
        return Vec::new();
    };
    let hosts = subnet_hosts(address, prefix_len);
    // A reply is not needed: the echo makes Windows resolve the MAC, which lands in the neighbour cache
    std::thread::scope(|scope| {
        for chunk in hosts.chunks(hosts.len().div_ceil(32).max(1)) {
            scope.spawn(move || {
                let Some(pinger) = Pinger::open() else { return };
                for host in chunk {
                    pinger.ping(*host, std::time::Duration::from_millis(400));
                }
            });
        }
    });
    let neighbors: Vec<Neighbor> = neighbors(adapter.if_index)
        .into_iter()
        .filter(|neighbor| is_device(neighbor, address, prefix_len))
        .collect();
    let names: Vec<Option<String>> = std::thread::scope(|scope| {
        let lookups: Vec<_> = neighbors
            .iter()
            .map(|neighbor| scope.spawn(move || host_name(neighbor.ip)))
            .collect();
        lookups
            .into_iter()
            .map(|lookup| lookup.join().ok().flatten())
            .collect()
    });
    let mut seen: Vec<Seen> = adapter
        .mac
        .map(|mac| Seen {
            mac,
            ip: address,
            name: std::env::var("COMPUTERNAME").ok(),
        })
        .into_iter()
        .collect();
    seen.extend(
        neighbors
            .into_iter()
            .zip(names)
            .map(|(neighbor, name)| Seen {
                mac: neighbor.mac,
                ip: neighbor.ip,
                name,
            }),
    );
    seen
}

/// A scan, kept in the database; a failure is logged, the next scan tries again.
pub fn scan_and_store(db_path: &std::path::Path) {
    #[cfg(windows)]
    let seen = scan();
    #[cfg(not(windows))]
    let seen: Vec<Seen> = Vec::new();
    let stored = crate::storage::Storage::open(db_path)
        .and_then(|mut storage| storage.put_seen(&seen, chrono::Utc::now().timestamp()));
    match stored {
        Ok(()) => tracing::info!("network scan: {} devices", seen.len()),
        Err(error) => tracing::warn!("the network scan was not saved: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maker_comes_from_the_registry() {
        assert_eq!(maker_of([0xdc, 0x51, 0x93, 0xa5, 0x61, 0x44]), Some("zte"));
        assert_eq!(
            maker_of([0x04, 0xcb, 0x01, 0xfd, 0x57, 0x68]),
            Some("Samsung Electronics")
        );
        assert_eq!(
            maker_of([0x9c, 0x6b, 0x00, 0xa2, 0x05, 0x21]),
            Some("ASRock")
        );
        // A phone's made-up MAC has no maker
        let random = [0x42, 0xfe, 0x6e, 0x11, 0x87, 0xf2];
        assert!(is_random(random));
        assert_eq!(maker_of(random), None);
    }

    #[test]
    fn a_mac_reads_back() {
        let mac = [0xdc, 0x51, 0x93, 0xa5, 0x61, 0x44];
        assert_eq!(mac_text(mac), "dc:51:93:a5:61:44");
        assert_eq!(mac_of("dc:51:93:a5:61:44"), Some(mac));
        assert_eq!(mac_of("dc:51:93"), None);
    }

    #[test]
    fn the_sweep_covers_the_subnet_and_not_a_wide_one() {
        let hosts = subnet_hosts(Ipv4Addr::new(192, 168, 10, 6), 24);
        assert_eq!(hosts.len(), 254);
        assert_eq!(hosts.first(), Some(&Ipv4Addr::new(192, 168, 10, 1)));
        assert_eq!(hosts.last(), Some(&Ipv4Addr::new(192, 168, 10, 254)));
        assert_eq!(
            subnet_hosts(Ipv4Addr::new(10, 0, 0, 5), 8),
            Vec::<Ipv4Addr>::new()
        );
        assert_eq!(subnet_hosts(Ipv4Addr::new(10, 0, 0, 5), 30).len(), 2);
    }

    #[test]
    fn multicast_and_broadcast_entries_are_not_devices() {
        let address = Ipv4Addr::new(192, 168, 10, 6);
        let device = |ip: [u8; 4], mac: [u8; 6]| {
            is_device(
                &Neighbor {
                    ip: Ipv4Addr::from(ip),
                    mac,
                },
                address,
                24,
            )
        };
        assert!(device(
            [192, 168, 10, 1],
            [0xdc, 0x51, 0x93, 0xa5, 0x61, 0x44]
        ));
        assert!(!device([192, 168, 10, 255], [0xff; 6]));
        assert!(!device(
            [224, 0, 0, 251],
            [0x01, 0x00, 0x5e, 0x00, 0x00, 0xfb]
        ));
        assert!(!device(
            [192, 168, 11, 3],
            [0x04, 0xcb, 0x01, 0xfd, 0x57, 0x68]
        ));
    }

    fn known(mac: &str, ip: &str, first_seen: i64, last_seen: i64) -> Known {
        Known {
            mac: mac.to_owned(),
            ip: ip.to_owned(),
            name: None,
            first_seen,
            last_seen,
        }
    }

    #[test]
    fn a_device_after_the_first_scan_is_new_for_a_week() {
        let week = NEW_SECONDS;
        let devices = devices_of(
            &[
                known("9c:6b:00:a2:05:21", "192.168.10.6", 100, 900),
                known("dc:51:93:a5:61:44", "192.168.10.1", 100, 900),
                known("42:fe:6e:11:87:f2", "192.168.10.7", 500, 900),
                known("04:cb:01:fd:57:68", "192.168.10.3", 100, 400),
            ],
            Some("9c:6b:00:a2:05:21"),
            Some("192.168.10.1"),
            1000,
        );
        let summary: Vec<(&str, DeviceRole, bool, bool)> = devices
            .iter()
            .map(|device| {
                (
                    device.ip.as_str(),
                    device.role,
                    device.is_online,
                    device.is_new,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("192.168.10.1", DeviceRole::Gateway, true, false),
                ("192.168.10.6", DeviceRole::This, true, false),
                ("192.168.10.7", DeviceRole::Other, true, true),
                ("192.168.10.3", DeviceRole::Other, false, false),
            ]
        );
        assert!(devices[2].is_random_mac);
        let later = devices_of(
            &[known("a", "1.1.1.1", 0, 0), known("b", "1.1.1.2", 1, 1)],
            None,
            None,
            1 + week,
        );
        assert!(!later[1].is_new);
    }

    #[test]
    fn one_scan_at_a_time() {
        let job = DevicesJob::default();
        let mut inner = false;
        assert!(job.run_once(|| inner = !job.run_once(|| {})));
        assert!(inner, "a scan inside a scan does not run");
    }
}
