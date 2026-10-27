//! The network: the adapters that lead out of the PC, and the quality of the way out, measured by bursts of pings to
//! the gateway and to one internet host every five minutes. A burst gives the mean round trip, the loss and the jitter.

pub mod speed;
#[cfg(windows)]
pub mod windows;

use std::net::Ipv4Addr;
use std::time::Duration;

pub use vitals_ipc::{Adapter, AdapterKind, NetSample, Probe};

/// The internet end of the measurement: Cloudflare's resolver answers ICMP from everywhere. Only pings go there, no data
pub const INTERNET_HOST: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
/// Pings in a burst, the pause between them and the wait for one reply
pub const BURST: usize = 10;
pub const GAP: Duration = Duration::from_millis(100);
pub const TIMEOUT: Duration = Duration::from_secs(1);
/// The network screen draws the last day; older samples are dropped after a month
pub const KEEP_DAYS: i64 = 30;

/// An adapter as IP Helper gives it, before the screen's choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAdapter {
    pub name: String,
    pub description: String,
    pub if_type: u32,
    pub link_bps: u64,
    pub address: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
}

/// The adapters with a gateway: virtual switches (WSL, Hyper-V) and loopback have none.
pub fn ways_out(raw: &[RawAdapter], kind_of: impl Fn(u32) -> AdapterKind) -> Vec<Adapter> {
    raw.iter()
        .filter(|adapter| {
            adapter
                .gateway
                .is_some_and(|gateway| !gateway.is_unspecified())
        })
        .map(|adapter| Adapter {
            name: adapter.name.clone(),
            description: adapter.description.clone(),
            kind: kind_of(adapter.if_type),
            // IP Helper says u64::MAX when it does not know the speed
            link_bps: (adapter.link_bps != 0 && adapter.link_bps != u64::MAX)
                .then_some(adapter.link_bps),
            address: adapter.address.map(|address| address.to_string()),
            gateway: adapter.gateway.map(|gateway| gateway.to_string()),
        })
        .collect()
}

#[allow(
    clippy::cast_precision_loss,
    reason = "round trips are milliseconds and counts are tens, far below f64's exact range"
)]
fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// The mean round trip, the share of lost pings, and the jitter as the mean change between consecutive replies.
#[allow(clippy::cast_precision_loss, reason = "a burst has ten pings")]
pub fn probe_of(round_trips: &[Option<u32>]) -> Probe {
    let replies: Vec<f64> = round_trips
        .iter()
        .flatten()
        .map(|ms| f64::from(*ms))
        .collect();
    let lost = round_trips.len() - replies.len();
    let changes: Vec<f64> = replies
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .collect();
    Probe {
        avg_ms: mean(&replies),
        loss_pct: if round_trips.is_empty() {
            100.0
        } else {
            lost as f64 * 100.0 / round_trips.len() as f64
        },
        jitter_ms: mean(&changes),
    }
}

/// A burst of pings to one host through `ping`.
pub fn burst(
    ping: &impl Fn(Ipv4Addr) -> Option<u32>,
    host: Ipv4Addr,
    sleep: &impl Fn(Duration),
) -> Probe {
    let mut round_trips = Vec::with_capacity(BURST);
    for index in 0..BURST {
        if index > 0 {
            sleep(GAP);
        }
        round_trips.push(ping(host));
    }
    probe_of(&round_trips)
}

#[cfg(windows)]
pub fn kind_of(if_type: u32) -> AdapterKind {
    match windows::is_ethernet(if_type) {
        Some(true) => AdapterKind::Ethernet,
        Some(false) => AdapterKind::Wifi,
        None => AdapterKind::Other,
    }
}

/// The adapters that lead out of this PC.
#[cfg(windows)]
pub fn adapters() -> Vec<Adapter> {
    ways_out(&windows::adapters(), kind_of)
}

/// One sample: a burst to the gateway of the first way out, and one to the internet host.
#[cfg(windows)]
pub fn sample(now: i64) -> Option<NetSample> {
    let pinger = windows::Pinger::open()?;
    let ping = |host| pinger.ping(host, TIMEOUT);
    let gateway = adapters()
        .into_iter()
        .find_map(|adapter| adapter.gateway?.parse::<Ipv4Addr>().ok());
    Some(NetSample {
        time: now,
        gateway: gateway.map(|gateway| burst(&ping, gateway, &std::thread::sleep)),
        internet: burst(&ping, INTERNET_HOST, &std::thread::sleep),
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn a_burst_gives_mean_loss_and_jitter() {
        let probe = probe_of(&[Some(10), Some(14), None, Some(12), Some(12)]);
        assert_eq!(probe.avg_ms, Some(12.0));
        assert!((probe.loss_pct - 20.0).abs() < f64::EPSILON);
        // |14-10|, |12-14|, |12-12| over the replies that came
        assert_eq!(probe.jitter_ms, Some(2.0));
    }

    #[test]
    fn no_reply_is_full_loss_and_no_figures() {
        let probe = probe_of(&[None, None]);
        assert_eq!(probe.avg_ms, None);
        assert!((probe.loss_pct - 100.0).abs() < f64::EPSILON);
        assert_eq!(probe.jitter_ms, None);
        assert!((probe_of(&[]).loss_pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_burst_pings_ten_times_with_a_gap_between() {
        let pinged = RefCell::new(0);
        let slept = RefCell::new(0);
        let probe = burst(
            &|_| {
                *pinged.borrow_mut() += 1;
                Some(5)
            },
            INTERNET_HOST,
            &|_| *slept.borrow_mut() += 1,
        );
        assert_eq!((*pinged.borrow(), *slept.borrow()), (BURST, BURST - 1));
        assert_eq!(probe.avg_ms, Some(5.0));
    }

    fn raw(name: &str, gateway: Option<Ipv4Addr>, link_bps: u64) -> RawAdapter {
        RawAdapter {
            name: name.to_owned(),
            description: format!("{name} adapter"),
            if_type: 6,
            link_bps,
            address: Some(Ipv4Addr::new(192, 168, 10, 6)),
            gateway,
        }
    }

    #[test]
    fn only_adapters_with_a_gateway_lead_out() {
        let adapters = ways_out(
            &[
                raw(
                    "Ethernet",
                    Some(Ipv4Addr::new(192, 168, 10, 1)),
                    1_000_000_000,
                ),
                raw("vEthernet (WSL)", None, 10_000_000_000),
                raw("Odd", Some(Ipv4Addr::UNSPECIFIED), u64::MAX),
            ],
            |_| AdapterKind::Ethernet,
        );
        assert_eq!(adapters.len(), 1);
        assert_eq!(adapters[0].link_bps, Some(1_000_000_000));
        assert_eq!(adapters[0].gateway.as_deref(), Some("192.168.10.1"));
    }
}
