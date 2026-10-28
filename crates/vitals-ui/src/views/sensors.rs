//! The sensors screen: the counters of the latest reading, a chart of the load or the temperature over the live ten
//! minutes or the history, and the readings of the processor, the memory and each card.

use vitals_ipc::{GpuReading, SensorPoint, SensorReading, SensorsLive};

use crate::format;
use crate::i18n::Locale;
use crate::views::disks::Tone;
use crate::views::network::{
    BOTTOM, CHART_HEIGHT, CHART_WIDTH, LEFT, Point, Stat, Stretch, TOP, Tick, TimeLabel,
    stretches_of,
};

/// Both metrics draw on 0–100: percent of load, degrees Celsius a card never passes
const SCALE: f64 = 100.0;
const LIVE_STEP_SECONDS: i64 = 2;
const HISTORY_STEP_SECONDS: i64 = 5 * 60;
/// NVIDIA cards slow down near 90 °C; from 80 the case is worth a look
const GPU_WARM_C: f64 = 80.0;
const GPU_HOT_C: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Live,
    Day,
    Week,
}

impl Span {
    pub const ALL: [Self; 3] = [Self::Live, Self::Day, Self::Week];

    /// The hours of history the span asks the core for; the live span draws the live points
    pub const fn hours(self) -> Option<u32> {
        match self {
            Self::Live => None,
            Self::Day => Some(24),
            Self::Week => Some(7 * 24),
        }
    }

    const fn seconds(self) -> i64 {
        match self {
            Self::Live => 10 * 60,
            Self::Day => 24 * 60 * 60,
            Self::Week => 7 * 24 * 60 * 60,
        }
    }

    const fn step(self) -> i64 {
        match self {
            Self::Live => LIVE_STEP_SECONDS,
            Self::Day | Self::Week => HISTORY_STEP_SECONDS,
        }
    }

    pub const fn label(self, locale: Locale) -> &'static str {
        let texts = &locale.texts().sensors;
        match self {
            Self::Live => texts.span_live,
            Self::Day => texts.span_day,
            Self::Week => texts.span_week,
        }
    }

    fn time_label(self, seconds: i64, locale: Locale) -> String {
        format::local_time(seconds).map_or_else(String::new, |time| match self {
            Self::Live => time.format("%H:%M:%S").to_string(),
            Self::Day => time.format("%H:%M").to_string(),
            Self::Week => format::calendar_day(&time.format("%Y-%m-%d").to_string(), locale),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Load,
    Temperature,
}

impl Metric {
    pub const ALL: [Self; 2] = [Self::Load, Self::Temperature];

    pub const fn label(self, locale: Locale) -> &'static str {
        let texts = &locale.texts().sensors;
        match self {
            Self::Load => texts.metric_load,
            Self::Temperature => texts.metric_temperature,
        }
    }

    fn value(self, value: f64) -> String {
        match self {
            Self::Load => percent(value),
            Self::Temperature => celsius(value),
        }
    }
}

/// A line of the chart: its style (solid for the processor, dashed for the card), its legend word, its stretches
#[derive(Debug, Clone, PartialEq)]
pub struct ChartLine {
    pub class: &'static str,
    pub label: &'static str,
    pub stretches: Vec<Stretch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SensorChart {
    pub lines: Vec<ChartLine>,
    pub ticks: Vec<Tick>,
    pub times: Vec<TimeLabel>,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartState {
    Loading,
    Error(&'static str),
    Empty(&'static str),
    Ready(SensorChart),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingRow {
    pub label: &'static str,
    pub value: String,
    /// The value is a word for what is missing, drawn muted
    pub is_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingTable {
    pub title: String,
    pub rows: Vec<ReadingRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SensorsReady {
    pub stats: Vec<Stat>,
    pub chart: ChartState,
    pub tables: Vec<ReadingTable>,
    pub has_gpu: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SensorsView {
    Loading,
    Error(&'static str),
    Ready(SensorsReady),
}

fn percent(value: f64) -> String {
    format!("{value:.0} %")
}

fn celsius(value: f64) -> String {
    format!("{value:.0} °C")
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "watts and megahertz are positive and far below u64::MAX, shown whole"
)]
fn whole(value: f64) -> u64 {
    value.max(0.0).round() as u64
}

fn ghz(mhz: f64, locale: Locale) -> String {
    (locale.texts().sensors.ghz)(&format::decimal(mhz / 1000.0, locale))
}

fn mhz(value: f64, locale: Locale) -> String {
    (locale.texts().sensors.mhz)(&format::count(whole(value), locale))
}

fn watts(value: f64, locale: Locale) -> String {
    (locale.texts().sensors.watts)(&format::count(whole(value), locale))
}

/// `nvidia-smi` counts video memory in mebibytes; the panel shows sizes as Explorer does.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "video memory in mebibytes is positive and small"
)]
fn mebibytes(value: f64, locale: Locale) -> String {
    format::bytes((value.max(0.0) * 1024.0 * 1024.0) as u64, locale)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "a share of memory in percent: bytes beyond 2^53 lose nothing visible"
)]
fn share(used: u64, total: u64) -> Option<f64> {
    (total > 0).then(|| used as f64 * 100.0 / total as f64)
}

fn missing(label: &'static str, word: &'static str) -> ReadingRow {
    ReadingRow {
        label,
        value: word.to_owned(),
        is_missing: true,
    }
}

fn row(label: &'static str, value: Option<String>, locale: Locale) -> ReadingRow {
    value.map_or_else(
        || missing(label, locale.texts().sensors.not_reported),
        |value| ReadingRow {
            label,
            value,
            is_missing: false,
        },
    )
}

fn pair(used: Option<String>, total: Option<String>, locale: Locale) -> Option<String> {
    let texts = &locale.texts().sensors;
    match (used, total) {
        (Some(used), Some(total)) => Some((texts.of)(&used, &total)),
        (used, _) => used,
    }
}

fn gpu_tone(temp_c: f64) -> Tone {
    if temp_c >= GPU_HOT_C {
        Tone::Danger
    } else if temp_c >= GPU_WARM_C {
        Tone::Warn
    } else {
        Tone::Ok
    }
}

fn memory_text(reading: &SensorReading, locale: Locale) -> Option<String> {
    pair(
        reading
            .ram_used_bytes
            .map(|bytes| format::bytes(bytes, locale)),
        reading
            .ram_total_bytes
            .map(|bytes| format::bytes(bytes, locale)),
        locale,
    )
}

fn power_text(gpu: &GpuReading, locale: Locale) -> Option<String> {
    pair(
        gpu.power_w.map(|value| watts(value, locale)),
        gpu.power_limit_w.map(|value| watts(value, locale)),
        locale,
    )
}

fn vram_text(gpu: &GpuReading, locale: Locale) -> Option<String> {
    pair(
        gpu.vram_used_mb.map(|value| mebibytes(value, locale)),
        gpu.vram_total_mb.map(|value| mebibytes(value, locale)),
        locale,
    )
}

fn stat(label: &'static str, value: Option<String>, hint: Option<String>) -> Stat {
    Stat {
        label: label.to_owned(),
        value: value.unwrap_or_else(|| "—".to_owned()),
        hint,
        tone: None,
    }
}

fn stats_of(reading: Option<&SensorReading>, locale: Locale) -> Vec<Stat> {
    let texts = &locale.texts().sensors;
    let cpu = reading.map(|reading| &reading.cpu);
    let gpu = reading.and_then(|reading| reading.gpus.first());
    let no_gpu = || Some(texts.no_gpu.to_owned());
    vec![
        stat(
            texts.cpu,
            cpu.and_then(|cpu| cpu.load_pct).map(percent),
            cpu.and_then(|cpu| cpu.mhz)
                .map(|value| (texts.clock_hint)(&ghz(value, locale))),
        ),
        stat(
            texts.memory,
            reading
                .and_then(|reading| share(reading.ram_used_bytes?, reading.ram_total_bytes?))
                .map(percent),
            reading.and_then(|reading| memory_text(reading, locale)),
        ),
        stat(
            texts.gpu,
            gpu.and_then(|gpu| gpu.load_pct).map(percent),
            gpu.map_or_else(no_gpu, |gpu| {
                vram_text(gpu, locale).map(|vram| (texts.vram_hint)(&vram))
            }),
        ),
        Stat {
            tone: gpu.and_then(|gpu| gpu.temp_c).map(gpu_tone),
            ..stat(
                texts.gpu_temperature,
                gpu.and_then(|gpu| gpu.temp_c).map(celsius),
                gpu.map_or_else(no_gpu, |gpu| {
                    power_text(gpu, locale).map(|power| (texts.power_hint)(&power))
                }),
            )
        },
    ]
}

fn system_table(reading: &SensorReading, locale: Locale) -> ReadingTable {
    let texts = &locale.texts().sensors;
    let cpu = &reading.cpu;
    let clock = cpu.mhz.map(|value| {
        let now = ghz(value, locale);
        cpu.base_mhz.map_or_else(
            || now.clone(),
            |base| (texts.with_base)(&now, &ghz(base, locale)),
        )
    });
    ReadingTable {
        title: texts.system_title.to_owned(),
        rows: vec![
            row(texts.processor, cpu.name.clone(), locale),
            row(texts.load, cpu.load_pct.map(percent), locale),
            row(texts.clock, clock, locale),
            missing(texts.cpu_temperature, texts.needs_lhm),
            row(texts.memory, memory_text(reading, locale), locale),
        ],
    }
}

fn gpu_table(gpu: &GpuReading, locale: Locale) -> ReadingTable {
    let texts = &locale.texts().sensors;
    ReadingTable {
        title: gpu.name.clone(),
        rows: vec![
            row(texts.temperature, gpu.temp_c.map(celsius), locale),
            row(texts.load, gpu.load_pct.map(percent), locale),
            row(texts.power, power_text(gpu, locale), locale),
            row(texts.fan, gpu.fan_pct.map(percent), locale),
            row(
                texts.core_clock,
                gpu.core_mhz.map(|value| mhz(value, locale)),
                locale,
            ),
            row(
                texts.memory_clock,
                gpu.memory_mhz.map(|value| mhz(value, locale)),
                locale,
            ),
            row(texts.vram, vram_text(gpu, locale), locale),
        ],
    }
}

type ValueOf = fn(&SensorPoint) -> Option<f64>;

/// The lines a metric draws; a line without a value in the span (no card) is left out.
fn series_of(
    metric: Metric,
    points: &[&SensorPoint],
    locale: Locale,
) -> Vec<(&'static str, &'static str, ValueOf)> {
    let texts = &locale.texts().sensors;
    let all: Vec<(&'static str, &'static str, ValueOf)> = match metric {
        Metric::Load => vec![
            ("line-cpu", texts.cpu, |point| point.cpu_load_pct),
            ("line-gpu", texts.gpu, |point| point.gpu_load_pct),
        ],
        Metric::Temperature => vec![("line-gpu", texts.gpu, |point| point.gpu_temp_c)],
    };
    all.into_iter()
        .filter(|(_, _, value_of)| points.iter().any(|point| value_of(point).is_some()))
        .collect()
}

#[allow(
    clippy::cast_precision_loss,
    reason = "seconds within a week are far below f64's exact range"
)]
pub fn sensor_chart(
    points: &[SensorPoint],
    span: Span,
    metric: Metric,
    now: i64,
    locale: Locale,
) -> Option<SensorChart> {
    let span_seconds = span.seconds();
    let from = now - span_seconds;
    let points: Vec<&SensorPoint> = points.iter().filter(|point| point.time >= from).collect();
    let series = series_of(metric, &points, locale);
    if series.is_empty() {
        return None;
    }
    let plot_height = CHART_HEIGHT - BOTTOM - TOP;
    let plot_width = CHART_WIDTH - LEFT;
    let x_of = |time: i64| LEFT + (time - from) as f64 / span_seconds as f64 * plot_width;
    let y_of = |value: f64| TOP + plot_height - value.clamp(0.0, SCALE) / SCALE * plot_height;
    // A gap of three steps breaks a line: the screen was closed, the panel off or the PC asleep
    let break_seconds = span.step() * 3;
    let column = plot_width / (span_seconds as f64 / span.step() as f64);
    let texts = &locale.texts().sensors;
    Some(SensorChart {
        lines: series
            .iter()
            .map(|(class, label, value_of)| ChartLine {
                class,
                label,
                stretches: stretches_of(
                    points.iter().map(|point| (point.time, value_of(point))),
                    break_seconds,
                    &x_of,
                    &y_of,
                ),
            })
            .collect(),
        ticks: [0.0, SCALE / 2.0, SCALE]
            .into_iter()
            .map(|value| Tick {
                y: y_of(value),
                label: metric.value(value),
            })
            .collect(),
        times: (0..=4)
            .map(|step| {
                let time = from + span_seconds / 4 * step;
                TimeLabel {
                    x: x_of(time),
                    label: span.time_label(time, locale),
                }
            })
            .collect(),
        points: points
            .iter()
            .map(|point| {
                let values: Vec<String> = series
                    .iter()
                    .map(|(_, label, value_of)| {
                        let value = value_of(point)
                            .map_or_else(|| "—".to_owned(), |value| metric.value(value));
                        format!("{label} {value}")
                    })
                    .collect();
                Point {
                    x: x_of(point.time) - column / 2.0,
                    width: column,
                    title: (texts.point)(&span.time_label(point.time, locale), &values.join(" · ")),
                }
            })
            .collect(),
    })
}

fn chart_state(
    live: &SensorsLive,
    history: Option<Result<&[SensorPoint], &'static str>>,
    span: Span,
    metric: Metric,
    now: i64,
    locale: Locale,
) -> ChartState {
    let texts = &locale.texts().sensors;
    let (points, empty) = if span == Span::Live {
        (live.points.as_slice(), texts.no_points_live)
    } else {
        match history {
            None => return ChartState::Loading,
            Some(Err(message)) => return ChartState::Error(message),
            Some(Ok(points)) => (points, texts.no_points_history),
        }
    };
    sensor_chart(points, span, metric, now, locale)
        .map_or(ChartState::Empty(empty), ChartState::Ready)
}

pub fn sensors_view(
    live: Option<Result<&SensorsLive, &'static str>>,
    history: Option<Result<&[SensorPoint], &'static str>>,
    span: Span,
    metric: Metric,
    now: i64,
    locale: Locale,
) -> SensorsView {
    let live = match live {
        None => return SensorsView::Loading,
        Some(Err(message)) => return SensorsView::Error(message),
        Some(Ok(live)) => live,
    };
    let reading = live.reading.as_ref();
    let mut tables: Vec<ReadingTable> = reading
        .map(|reading| system_table(reading, locale))
        .into_iter()
        .collect();
    tables.extend(
        reading
            .iter()
            .flat_map(|reading| &reading.gpus)
            .map(|gpu| gpu_table(gpu, locale)),
    );
    SensorsView::Ready(SensorsReady {
        stats: stats_of(reading, locale),
        chart: chart_state(live, history, span, metric, now, locale),
        tables,
        has_gpu: reading.is_some_and(|reading| !reading.gpus.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use vitals_ipc::CpuReading;

    use super::*;

    fn card() -> GpuReading {
        GpuReading {
            name: "NVIDIA GeForce RTX 4070 Ti SUPER".to_owned(),
            temp_c: Some(26.72),
            load_pct: Some(20.0),
            power_w: Some(23.54),
            power_limit_w: Some(285.0),
            fan_pct: Some(0.0),
            core_mhz: Some(2805.0),
            memory_mhz: Some(405.0),
            vram_used_mb: Some(1516.0),
            vram_total_mb: Some(16376.0),
        }
    }

    fn reading(gpus: Vec<GpuReading>) -> SensorReading {
        SensorReading {
            time: 1_000,
            cpu: CpuReading {
                name: Some("AMD Ryzen 9 9900X 12-Core Processor".to_owned()),
                load_pct: Some(12.4),
                mhz: Some(4950.0),
                base_mhz: Some(4400.0),
            },
            ram_used_bytes: Some(16 * 1024 * 1024 * 1024),
            ram_total_bytes: Some(64 * 1024 * 1024 * 1024),
            gpus,
        }
    }

    fn point(time: i64, cpu: Option<f64>, gpu: Option<f64>) -> SensorPoint {
        SensorPoint {
            time,
            cpu_load_pct: cpu,
            cpu_mhz: None,
            ram_used_pct: None,
            gpu_temp_c: gpu.map(|load| load + 20.0),
            gpu_load_pct: gpu,
            gpu_power_w: None,
        }
    }

    fn live(gpus: Vec<GpuReading>, points: Vec<SensorPoint>) -> SensorsLive {
        SensorsLive {
            reading: Some(reading(gpus)),
            points,
        }
    }

    fn ready(view: SensorsView) -> SensorsReady {
        match view {
            SensorsView::Ready(ready) => ready,
            other => panic!("not ready: {other:?}"),
        }
    }

    #[test]
    fn the_counters_say_the_load_the_memory_and_the_card() {
        let answer = live(vec![card()], Vec::new());
        let view = ready(sensors_view(
            Some(Ok(&answer)),
            None,
            Span::Live,
            Metric::Load,
            1_000,
            Locale::En,
        ));
        let values: Vec<&str> = view.stats.iter().map(|stat| stat.value.as_str()).collect();
        assert_eq!(values, ["12 %", "25 %", "20 %", "27 °C"]);
        assert_eq!(view.stats[0].hint.as_deref(), Some("at 5.0 GHz"));
        assert_eq!(view.stats[1].hint.as_deref(), Some("16.0 GB of 64.0 GB"));
        assert_eq!(view.stats[3].hint.as_deref(), Some("power 24 W of 285 W"));
        assert_eq!(view.stats[3].tone, Some(Tone::Ok));
        assert!(view.has_gpu);
    }

    #[test]
    fn a_hot_card_is_marked() {
        assert_eq!(gpu_tone(79.9), Tone::Ok);
        assert_eq!(gpu_tone(80.0), Tone::Warn);
        assert_eq!(gpu_tone(91.0), Tone::Danger);
    }

    #[test]
    fn without_a_card_the_counters_say_so_and_only_the_processor_draws() {
        let answer = live(
            Vec::new(),
            vec![point(400, Some(10.0), None), point(402, Some(30.0), None)],
        );
        let view = ready(sensors_view(
            Some(Ok(&answer)),
            None,
            Span::Live,
            Metric::Load,
            1_000,
            Locale::En,
        ));
        let no_gpu = Locale::En.texts().sensors.no_gpu;
        assert_eq!(view.stats[2].value, "—");
        assert_eq!(view.stats[2].hint.as_deref(), Some(no_gpu));
        assert_eq!(view.stats[3].tone, None);
        assert!(!view.has_gpu);
        assert_eq!(view.tables.len(), 1);
        let ChartState::Ready(chart) = view.chart else {
            panic!("no chart");
        };
        assert_eq!(chart.lines.len(), 1);
        assert_eq!(chart.lines[0].class, "line-cpu");
        // Without a card there is nothing to draw for the temperature
        let view = ready(sensors_view(
            Some(Ok(&answer)),
            None,
            Span::Live,
            Metric::Temperature,
            1_000,
            Locale::En,
        ));
        assert_eq!(
            view.chart,
            ChartState::Empty(Locale::En.texts().sensors.no_points_live)
        );
    }

    #[test]
    fn the_tables_read_the_processor_and_each_card() {
        let answer = live(vec![card()], Vec::new());
        let view = ready(sensors_view(
            Some(Ok(&answer)),
            None,
            Span::Live,
            Metric::Load,
            1_000,
            Locale::Ru,
        ));
        let texts = &Locale::Ru.texts().sensors;
        assert_eq!(view.tables.len(), 2);
        let system = &view.tables[0];
        assert_eq!(system.rows[0].value, "AMD Ryzen 9 9900X 12-Core Processor");
        assert_eq!(
            system.rows[2].value,
            (texts.with_base)(&ghz(4950.0, Locale::Ru), &ghz(4400.0, Locale::Ru))
        );
        assert!(system.rows[3].is_missing);
        let gpu = &view.tables[1];
        assert_eq!(gpu.title, "NVIDIA GeForce RTX 4070 Ti SUPER");
        assert_eq!(gpu.rows[4].value, (texts.mhz)("2\u{a0}805"));
        let bare = GpuReading {
            fan_pct: None,
            ..card()
        };
        assert!(gpu_table(&bare, Locale::Ru).rows[3].is_missing);
    }

    #[test]
    fn the_live_chart_breaks_where_the_screen_was_closed() {
        let points = vec![
            point(400, Some(10.0), Some(5.0)),
            point(402, Some(20.0), Some(6.0)),
            // Closed for a minute
            point(470, Some(30.0), Some(7.0)),
            point(472, Some(40.0), None),
        ];
        let chart = sensor_chart(&points, Span::Live, Metric::Load, 1_000, Locale::En).unwrap();
        let cpu = &chart.lines[0].stretches;
        assert_eq!(cpu.len(), 2);
        assert!(matches!(cpu[0], Stretch::Line(_)));
        // The card's last value is missing: its second stretch is one point
        assert!(matches!(chart.lines[1].stretches[1], Stretch::Dot { .. }));
        assert_eq!(chart.points.len(), 4);
        assert!(chart.points[3].title.ends_with("CPU 40 % · GPU —"));
        assert_eq!(chart.ticks[2].label, "100 %");
    }

    #[test]
    fn the_history_waits_fails_or_draws_on_its_own() {
        let answer = live(vec![card()], Vec::new());
        let at = |history| {
            ready(sensors_view(
                Some(Ok(&answer)),
                history,
                Span::Day,
                Metric::Temperature,
                100_000,
                Locale::En,
            ))
            .chart
        };
        assert_eq!(at(None), ChartState::Loading);
        assert_eq!(at(Some(Err("storage"))), ChartState::Error("storage"));
        assert_eq!(
            at(Some(Ok(&[]))),
            ChartState::Empty(Locale::En.texts().sensors.no_points_history)
        );
        let history = [point(99_000, Some(5.0), Some(20.0))];
        let ChartState::Ready(chart) = at(Some(Ok(&history))) else {
            panic!("no chart");
        };
        assert_eq!(chart.ticks[1].label, "50 °C");
    }

    #[test]
    fn the_screen_waits_for_its_first_answer() {
        assert_eq!(
            sensors_view(None, None, Span::Live, Metric::Load, 0, Locale::En),
            SensorsView::Loading
        );
        assert_eq!(
            sensors_view(
                Some(Err("timeout")),
                None,
                Span::Live,
                Metric::Load,
                0,
                Locale::En
            ),
            SensorsView::Error("timeout")
        );
    }
}
