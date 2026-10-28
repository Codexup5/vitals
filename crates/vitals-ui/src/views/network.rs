//! The network screen: the counters of the last measurement, the latency chart of the day, the adapters, the speed
//! test.

use vitals_ipc::{
    Adapter, AdapterKind, Device, DeviceRole, DevicesOverview, NetSample, NetworkOverview, Probe,
    SpeedStatus,
};

use crate::format;
use crate::i18n::Locale;
use crate::views::disks::Tone;
use crate::views::errors::nice_ceiling;

pub const CHART_WIDTH: f64 = 720.0;
pub const CHART_HEIGHT: f64 = 200.0;
pub const LEFT: f64 = 48.0;
pub const BOTTOM: f64 = 24.0;
pub const TOP: f64 = 8.0;
const DAY_SECONDS: i64 = 24 * 60 * 60;
/// Two missed rounds of five minutes break a line: the panel was off or the PC slept
const BREAK_SECONDS: i64 = 15 * 60;
/// The chart keeps at least this many milliseconds of scale, so a calm line does not look like noise
const MIN_SCALE_MS: u32 = 10;
const RECENT_ROWS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
    pub tone: Option<Tone>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterRow {
    pub name: String,
    pub description: String,
    pub kind: &'static str,
    pub link: Option<String>,
    pub addresses: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub y: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeLabel {
    pub x: f64,
    pub label: String,
}

/// A hover target over one measurement: its column of the plot and what it says
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub x: f64,
    pub width: f64,
    pub title: String,
}

/// An unbroken stretch of a line: a polyline, or a dot when one measurement stands alone
#[derive(Debug, Clone, PartialEq)]
pub enum Stretch {
    Line(String),
    Dot { x: f64, y: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LatencyChart {
    pub gateway: Vec<Stretch>,
    pub internet: Vec<Stretch>,
    pub ticks: Vec<Tick>,
    pub times: Vec<TimeLabel>,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentRow {
    pub time: String,
    pub gateway: String,
    pub internet: String,
    pub loss: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkReady {
    pub stats: Vec<Stat>,
    pub chart: Option<LatencyChart>,
    pub recent: Vec<RecentRow>,
    pub adapters: Vec<AdapterRow>,
    pub internet_host: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkView {
    Loading,
    Error(&'static str),
    Ready(NetworkReady),
}

/// The ICMP API counts whole milliseconds: a mean under one is a reply faster than it can tell.
fn ms(value: f64, locale: Locale) -> String {
    let texts = &locale.texts().network;
    if value > 0.0 && value < 1.0 || value == 0.0 {
        return texts.under_one_ms.to_owned();
    }
    (texts.ms)(&format::decimal(value, locale))
}

/// A scale mark, or a jitter (a mean of differences, so a fraction is real): zero is zero, not "under one".
fn exact_ms(value: f64, locale: Locale) -> String {
    (locale.texts().network.ms)(&format::decimal(value, locale))
}

fn percent(value: f64) -> String {
    format!("{value:.0} %")
}

/// `1,2 Гбит/с` or `940,0 Мбит/с` from bits per second.
#[allow(
    clippy::cast_precision_loss,
    reason = "a link speed is shown with one decimal"
)]
pub fn bits(bits_per_second: u64, locale: Locale) -> String {
    let texts = &locale.texts().network;
    let value = bits_per_second as f64;
    if value >= 1e9 {
        (texts.gbps)(&format::decimal(value / 1e9, locale))
    } else {
        (texts.mbps)(&format::decimal(value / 1e6, locale))
    }
}

fn probe_value(probe: Option<&Probe>, locale: Locale) -> String {
    let texts = &locale.texts().network;
    match probe {
        None => texts.no_gateway.to_owned(),
        Some(probe) => probe
            .avg_ms
            .map_or_else(|| texts.no_reply.to_owned(), |avg| ms(avg, locale)),
    }
}

fn probe_stat(label: String, probe: Option<&Probe>, has_sample: bool, locale: Locale) -> Stat {
    let texts = &locale.texts().network;
    if !has_sample {
        return Stat {
            label,
            value: "—".to_owned(),
            hint: Some(texts.no_sample.to_owned()),
            tone: None,
        };
    }
    let tone = probe.map(|probe| {
        if probe.avg_ms.is_none() {
            Tone::Danger
        } else if probe.loss_pct > 0.0 {
            Tone::Warn
        } else {
            Tone::Ok
        }
    });
    Stat {
        label,
        value: probe_value(probe, locale),
        hint: probe.map(|probe| {
            (texts.probe_hint)(
                &percent(probe.loss_pct),
                &probe
                    .jitter_ms
                    .map_or_else(|| "—".to_owned(), |jitter| exact_ms(jitter, locale)),
            )
        }),
        tone,
    }
}

/// The download as the figure, the upload under it: both on one line do not fit a counter.
fn speed_stat(status: Option<&SpeedStatus>, locale: Locale) -> (String, Option<String>) {
    let texts = &locale.texts().network;
    let Some(last) = status.and_then(|status| status.last) else {
        return ("—".to_owned(), Some(texts.speed_never.to_owned()));
    };
    let value = |bytes: Option<u64>| {
        bytes.map_or_else(
            || texts.failed_value.to_owned(),
            |bytes| bits(bytes.saturating_mul(8), locale),
        )
    };
    (
        (texts.download)(&value(last.down)),
        Some((texts.upload)(&value(last.up))),
    )
}

fn adapter_row(adapter: &Adapter, locale: Locale) -> AdapterRow {
    let texts = &locale.texts().network;
    AdapterRow {
        name: adapter.name.clone(),
        description: adapter.description.clone(),
        kind: match adapter.kind {
            AdapterKind::Ethernet => texts.ethernet,
            AdapterKind::Wifi => texts.wifi,
            AdapterKind::Other => texts.other,
        },
        link: adapter.link_bps.map(|bps| bits(bps, locale)),
        addresses: (texts.addresses)(
            adapter.address.as_deref().unwrap_or("—"),
            adapter.gateway.as_deref().unwrap_or("—"),
        ),
    }
}

fn clock(seconds: i64) -> String {
    format::local_time(seconds).map_or_else(String::new, |time| time.format("%H:%M").to_string())
}

/// The unbroken stretches of a line through `(time, value)` points, oldest first: a missing value or a gap longer
/// than `break_seconds` breaks it, and a point that stands alone is a dot.
pub fn stretches_of(
    points: impl IntoIterator<Item = (i64, Option<f64>)>,
    break_seconds: i64,
    x_of: &impl Fn(i64) -> f64,
    y_of: &impl Fn(f64) -> f64,
) -> Vec<Stretch> {
    let mut stretches: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut last_time: Option<i64> = None;
    for (time, value) in points {
        match value {
            Some(value) => {
                let is_continued = last_time.is_some_and(|last| time - last <= break_seconds);
                if !is_continued {
                    stretches.push(Vec::new());
                }
                if let Some(stretch) = stretches.last_mut() {
                    stretch.push((x_of(time), y_of(value)));
                }
                last_time = Some(time);
            }
            None => last_time = None,
        }
    }
    stretches
        .into_iter()
        .map(|stretch| match stretch.as_slice() {
            [(x, y)] => Stretch::Dot { x: *x, y: *y },
            points => Stretch::Line(
                points
                    .iter()
                    .map(|(x, y)| format!("{x:.1},{y:.1}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        })
        .collect()
}

#[allow(
    clippy::cast_precision_loss,
    reason = "seconds within a day and milliseconds of a ping are far below f64's exact range"
)]
pub fn latency_chart(samples: &[NetSample], now: i64, locale: Locale) -> Option<LatencyChart> {
    let from = now - DAY_SECONDS;
    let samples: Vec<&NetSample> = samples
        .iter()
        .filter(|sample| sample.time >= from)
        .collect();
    if samples.is_empty() {
        return None;
    }
    let peak = samples
        .iter()
        .flat_map(|sample| {
            [
                sample.gateway.and_then(|probe| probe.avg_ms),
                sample.internet.avg_ms,
            ]
        })
        .flatten()
        .fold(0.0_f64, f64::max);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a ping peak in whole milliseconds"
    )]
    let scale = f64::from(nice_ceiling((peak.ceil() as u32).max(MIN_SCALE_MS)));
    let plot_height = CHART_HEIGHT - BOTTOM - TOP;
    let plot_width = CHART_WIDTH - LEFT;
    let x_of = |time: i64| LEFT + (time - from) as f64 / DAY_SECONDS as f64 * plot_width;
    let y_of = |value: f64| TOP + plot_height - value.min(scale) / scale * plot_height;
    let line = |value_of: &dyn Fn(&NetSample) -> Option<f64>| {
        stretches_of(
            samples.iter().map(|sample| (sample.time, value_of(sample))),
            BREAK_SECONDS,
            &x_of,
            &y_of,
        )
    };
    let column = plot_width / (DAY_SECONDS as f64 / 300.0);
    let texts = &locale.texts().network;
    Some(LatencyChart {
        gateway: line(&|sample| sample.gateway.and_then(|probe| probe.avg_ms)),
        internet: line(&|sample| sample.internet.avg_ms),
        ticks: [0.0, scale / 2.0, scale]
            .into_iter()
            .map(|value| Tick {
                y: y_of(value),
                label: exact_ms(value, locale),
            })
            .collect(),
        times: (0..=4)
            .map(|step| {
                let time = from + DAY_SECONDS / 4 * step;
                TimeLabel {
                    x: x_of(time),
                    label: clock(time),
                }
            })
            .collect(),
        points: samples
            .iter()
            .map(|sample| Point {
                x: x_of(sample.time) - column / 2.0,
                width: column,
                title: (texts.point)(
                    &clock(sample.time),
                    &probe_value(sample.gateway.as_ref(), locale),
                    &probe_value(Some(&sample.internet), locale),
                ),
            })
            .collect(),
    })
}

pub fn network_view(
    answer: Option<Result<&NetworkOverview, &'static str>>,
    speed: Option<&SpeedStatus>,
    now: i64,
    locale: Locale,
) -> NetworkView {
    let overview = match answer {
        None => return NetworkView::Loading,
        Some(Err(message)) => return NetworkView::Error(message),
        Some(Ok(overview)) => overview,
    };
    let texts = &locale.texts().network;
    let latest = overview.samples.last();
    let link = overview
        .adapters
        .first()
        .and_then(|adapter| adapter.link_bps);
    let stats = vec![
        probe_stat(
            texts.gateway.to_owned(),
            latest.and_then(|sample| sample.gateway.as_ref()),
            latest.is_some(),
            locale,
        ),
        probe_stat(
            (texts.internet)(&overview.internet_host),
            latest.map(|sample| &sample.internet),
            latest.is_some(),
            locale,
        ),
        Stat {
            label: texts.link.to_owned(),
            value: link.map_or_else(|| "—".to_owned(), |bps| bits(bps, locale)),
            hint: overview
                .adapters
                .first()
                .map(|adapter| adapter.name.clone()),
            tone: None,
        },
        {
            let (value, hint) = speed_stat(speed, locale);
            Stat {
                label: texts.speed.to_owned(),
                value,
                hint,
                tone: None,
            }
        },
    ];
    NetworkView::Ready(NetworkReady {
        stats,
        chart: latency_chart(&overview.samples, now, locale),
        recent: overview
            .samples
            .iter()
            .rev()
            .take(RECENT_ROWS)
            .map(|sample| RecentRow {
                time: clock(sample.time),
                gateway: probe_value(sample.gateway.as_ref(), locale),
                internet: probe_value(Some(&sample.internet), locale),
                loss: percent(sample.internet.loss_pct),
            })
            .collect(),
        adapters: overview
            .adapters
            .iter()
            .map(|adapter| adapter_row(adapter, locale))
            .collect(),
        internet_host: overview.internet_host.clone(),
    })
}

/// What the last speed test found, or why it found nothing.
pub fn speed_line(status: &SpeedStatus, locale: Locale) -> Option<(String, Tone)> {
    let texts = &locale.texts().network;
    let last = status.last?;
    if last.down.is_none() && last.up.is_none() {
        return Some((texts.speed_failed.to_owned(), Tone::Warn));
    }
    let value = |bytes: Option<u64>| {
        bytes.map_or_else(
            || texts.failed_value.to_owned(),
            |bytes| bits(bytes.saturating_mul(8), locale),
        )
    };
    Some((
        (texts.speed_result)(&value(last.down), &value(last.up), &clock(last.finished_at)),
        Tone::Ok,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    pub key: String,
    pub title: String,
    pub address: String,
    /// The maker, or why there is none
    pub detail: Option<String>,
    pub tags: Vec<(&'static str, Tone)>,
    pub seen: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicesReady {
    pub summary: String,
    pub last_scan: String,
    pub rows: Vec<DeviceRow>,
    pub is_scanning: bool,
}

fn day_time(seconds: i64, locale: Locale) -> String {
    format::local_time(seconds).map_or_else(String::new, |time| format::date_time(time, locale))
}

fn device_row(device: &Device, locale: Locale) -> DeviceRow {
    let texts = &locale.texts().network;
    let mut tags = Vec::new();
    match device.role {
        DeviceRole::This => tags.push((texts.this_pc, Tone::Accent)),
        DeviceRole::Gateway => tags.push((texts.router_role, Tone::Accent)),
        DeviceRole::Other => {}
    }
    if device.is_new {
        tags.push((texts.new_device, Tone::Warn));
    }
    if !device.is_online {
        tags.push((texts.offline_device, Tone::Accent));
    }
    if device.is_random_mac {
        tags.push((texts.random_mac, Tone::Accent));
    }
    DeviceRow {
        key: device.mac.clone(),
        title: device
            .name
            .clone()
            .or_else(|| device.maker.clone())
            .unwrap_or_else(|| texts.unknown_device.to_owned()),
        address: format!("{} · {}", device.ip, device.mac),
        // The maker under a name; a device without a name already has its maker as the title
        detail: if device.is_random_mac {
            Some(texts.random_hint.to_owned())
        } else {
            device.name.as_ref().and(device.maker.clone())
        },
        tags,
        seen: if device.is_online {
            (texts.first_seen)(&day_time(device.first_seen, locale))
        } else {
            (texts.last_seen)(&day_time(device.last_seen, locale))
        },
    }
}

pub fn devices_view(overview: &DevicesOverview, locale: Locale) -> DevicesReady {
    let texts = &locale.texts().network;
    let online = overview
        .devices
        .iter()
        .filter(|device| device.is_online)
        .count();
    DevicesReady {
        summary: (texts.devices_count)(online, overview.devices.len()),
        last_scan: overview.last_scan.map_or_else(
            || texts.devices_never.to_owned(),
            |time| (texts.devices_last)(&day_time(time, locale)),
        ),
        rows: overview
            .devices
            .iter()
            .map(|device| device_row(device, locale))
            .collect(),
        is_scanning: overview.is_scanning,
    }
}

#[cfg(test)]
mod tests {
    use vitals_ipc::SpeedResult;

    use super::*;

    fn probe(avg: Option<f64>, loss: f64) -> Probe {
        Probe {
            avg_ms: avg,
            loss_pct: loss,
            jitter_ms: avg.map(|_| 0.5),
        }
    }

    fn sample(time: i64, gateway: Option<f64>, internet: Option<f64>) -> NetSample {
        NetSample {
            time,
            gateway: Some(probe(gateway, 0.0)),
            internet: probe(internet, if internet.is_some() { 0.0 } else { 100.0 }),
        }
    }

    #[test]
    fn a_reply_faster_than_a_millisecond_says_so() {
        assert_eq!(ms(0.0, Locale::En), "< 1 ms");
        assert_eq!(ms(0.4, Locale::En), "< 1 ms");
        assert_eq!(ms(2.5, Locale::En), "2.5 ms");
        assert_eq!(exact_ms(0.0, Locale::En), "0.0 ms");
    }

    #[test]
    fn link_speeds_read_in_the_right_unit() {
        assert_eq!(bits(1_000_000_000, Locale::En), "1.0 Gbit/s");
        assert_eq!(bits(312_500_000, Locale::En), "312.5 Mbit/s");
    }

    #[test]
    fn a_line_breaks_on_a_missed_reply_and_on_a_long_gap() {
        let now = 100_000;
        let samples = [
            sample(now - 900, Some(1.0), Some(12.0)),
            sample(now - 600, Some(1.0), None),
            sample(now - 300, Some(1.0), Some(14.0)),
            sample(now, Some(1.0), Some(13.0)),
        ];
        let chart = latency_chart(&samples, now, Locale::En).expect("a chart");
        assert_eq!(chart.gateway.len(), 1);
        assert_eq!(chart.internet.len(), 2);
        assert_eq!(chart.points.len(), 4);
        // The scale is a round number above the peak of 14 ms
        assert_eq!(
            chart.ticks.last().map(|tick| tick.label.as_str()),
            Some("20.0 ms")
        );
        let gap = [
            sample(now - 3600, Some(1.0), Some(5.0)),
            sample(now, Some(1.0), Some(5.0)),
        ];
        let chart = latency_chart(&gap, now, Locale::En).expect("a chart");
        assert_eq!(chart.gateway.len(), 2);
        // A measurement alone is a dot, not an invisible line of one point
        assert!(matches!(chart.gateway[0], Stretch::Dot { .. }));
        // A calm line of 5 ms keeps the scale of at least ten
        assert_eq!(
            chart.ticks.last().map(|tick| tick.label.as_str()),
            Some("10.0 ms")
        );
    }

    #[test]
    fn every_mark_stays_inside_the_drawing() {
        let now = 200_000;
        let samples: Vec<NetSample> = (0..288)
            .map(|step| {
                sample(
                    now - DAY_SECONDS + step * 300,
                    Some(2.0),
                    Some(f64::from(u32::try_from(step).unwrap_or(0))),
                )
            })
            .collect();
        let chart = latency_chart(&samples, now, Locale::En).expect("a chart");
        for stretch in chart.internet.iter().chain(&chart.gateway) {
            let Stretch::Line(points) = stretch else {
                panic!("one long line");
            };
            for point in points.split(' ') {
                let (x, y) = point.split_once(',').expect("x,y");
                let (x, y): (f64, f64) = (x.parse().expect("x"), y.parse().expect("y"));
                assert!((LEFT..=CHART_WIDTH).contains(&x), "x {x}");
                assert!((TOP..=CHART_HEIGHT - BOTTOM).contains(&y), "y {y}");
            }
        }
    }

    fn overview(samples: Vec<NetSample>) -> NetworkOverview {
        NetworkOverview {
            adapters: vec![Adapter {
                name: "Ethernet".to_owned(),
                description: "Realtek".to_owned(),
                kind: AdapterKind::Ethernet,
                link_bps: Some(1_000_000_000),
                address: Some("192.168.10.6".to_owned()),
                gateway: Some("192.168.10.1".to_owned()),
            }],
            samples,
            internet_host: "1.1.1.1".to_owned(),
        }
    }

    #[test]
    fn the_counters_show_the_last_measurement_and_its_trouble() {
        let mut lossy = sample(500, Some(1.0), Some(20.0));
        lossy.internet.loss_pct = 10.0;
        let speed = SpeedStatus {
            is_running: false,
            last: Some(SpeedResult {
                down: Some(50_000_000),
                up: None,
                finished_at: 400,
            }),
        };
        let NetworkView::Ready(ready) = network_view(
            Some(Ok(&overview(vec![lossy]))),
            Some(&speed),
            500,
            Locale::En,
        ) else {
            panic!("ready");
        };
        assert_eq!(ready.stats[0].value, "1.0 ms");
        assert_eq!(ready.stats[0].tone, Some(Tone::Ok));
        assert_eq!(ready.stats[1].tone, Some(Tone::Warn));
        assert_eq!(
            ready.stats[1].hint.as_deref(),
            Some("loss 10 % · jitter 0.5 ms")
        );
        assert_eq!(ready.stats[2].value, "1.0 Gbit/s");
        assert_eq!(ready.stats[3].value, "↓ 400.0 Mbit/s");
        assert_eq!(ready.stats[3].hint.as_deref(), Some("upload failed"));
        assert_eq!(ready.adapters[0].kind, "Cable");
    }

    #[test]
    fn no_measurement_yet_says_so() {
        let NetworkView::Ready(ready) =
            network_view(Some(Ok(&overview(Vec::new()))), None, 0, Locale::En)
        else {
            panic!("ready");
        };
        assert_eq!(ready.stats[0].value, "—");
        assert_eq!(ready.stats[0].hint.as_deref(), Some("not measured yet"));
        assert_eq!(ready.chart, None);
        assert_eq!(ready.stats[3].value, "—");
        assert_eq!(ready.stats[3].hint.as_deref(), Some("not measured"));
    }

    fn device(name: Option<&str>, maker: Option<&str>, role: DeviceRole) -> Device {
        Device {
            mac: "dc:51:93:a5:61:44".to_owned(),
            ip: "192.168.10.1".to_owned(),
            name: name.map(str::to_owned),
            maker: maker.map(str::to_owned),
            is_random_mac: maker.is_none(),
            role,
            first_seen: 0,
            last_seen: 0,
            is_online: true,
            is_new: false,
        }
    }

    #[test]
    fn a_device_is_named_by_its_name_then_its_maker() {
        let router = device_row(&device(None, Some("zte"), DeviceRole::Gateway), Locale::En);
        assert_eq!(router.title, "zte");
        assert_eq!(router.tags, [("Router", Tone::Accent)]);
        assert_eq!(router.address, "192.168.10.1 · dc:51:93:a5:61:44");
        assert_eq!(router.detail, None);
        let mut phone = device(None, None, DeviceRole::Other);
        phone.is_new = true;
        phone.is_online = false;
        let row = device_row(&phone, Locale::En);
        assert_eq!(row.title, "Unknown device");
        assert_eq!(
            row.tags.iter().map(|tag| tag.0).collect::<Vec<_>>(),
            ["New", "Offline", "Random MAC"]
        );
        assert_eq!(
            row.detail.as_deref(),
            Some(Locale::En.texts().network.random_hint)
        );
        let tv = device_row(
            &device(Some("TV"), Some("Samsung"), DeviceRole::Other),
            Locale::En,
        );
        assert_eq!(
            (tv.title.as_str(), tv.detail.as_deref()),
            ("TV", Some("Samsung"))
        );
    }

    #[test]
    fn the_devices_summary_counts_the_online() {
        let mut offline = device(None, Some("Samsung"), DeviceRole::Other);
        offline.is_online = false;
        let overview = DevicesOverview {
            devices: vec![device(None, Some("zte"), DeviceRole::Gateway), offline],
            last_scan: None,
            is_scanning: false,
        };
        let ready = devices_view(&overview, Locale::En);
        assert_eq!(ready.summary, "1 online of 2 known");
        assert_eq!(ready.last_scan, "No search for devices yet.");
    }

    #[test]
    fn a_failed_speed_test_says_why() {
        let failed = SpeedStatus {
            is_running: false,
            last: Some(SpeedResult {
                down: None,
                up: None,
                finished_at: 0,
            }),
        };
        assert_eq!(
            speed_line(&failed, Locale::En).map(|line| line.1),
            Some(Tone::Warn)
        );
        assert_eq!(speed_line(&SpeedStatus::default(), Locale::En), None);
    }
}
