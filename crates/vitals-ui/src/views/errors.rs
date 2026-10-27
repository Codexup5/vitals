//! What the errors screen shows, computed from the answers of the core: the counters, the crash chart, the feed and
//! the dumps. No markup here; `ui::errors` draws these.

use vitals_ipc::{DayRow, Dump, EVENT_PAGE_SIZE, EventPage, FeedKind, HealthOverview};

use crate::format;
use crate::i18n::Locale;
use crate::ipc::IpcError;

/// The kinds the crash chart stacks: the ones that mean the PC or a program fell over. Order fixes their colours.
pub const CHART_KINDS: [FeedKind; 5] = [
    FeedKind::Crash,
    FeedKind::Bsod,
    FeedKind::Whea,
    FeedKind::GpuReset,
    FeedKind::AppCrash,
];

pub const CHART_WIDTH: f64 = 720.0;
pub const CHART_HEIGHT: f64 = 240.0;
const PLOT_LEFT: f64 = 40.0;
const PLOT_RIGHT: f64 = 8.0;
const PLOT_TOP: f64 = 12.0;
const PLOT_BOTTOM: f64 = 28.0;
const BAR_SHARE: f64 = 0.7;
const SEGMENT_GAP: f64 = 2.0;
const MAX_X_LABELS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Range {
    #[default]
    Days30,
    Days90,
    Months,
}

impl Range {
    pub const ALL: [Self; 3] = [Self::Days30, Self::Days90, Self::Months];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Days30 => "30d",
            Self::Days90 => "90d",
            Self::Months => "months",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|range| range.code() == code)
    }

    /// The days the overview asks for; the monthly series comes with any range
    pub const fn days(self) -> u32 {
        match self {
            Self::Days30 | Self::Months => 30,
            Self::Days90 => 90,
        }
    }

    pub const fn label(self, locale: Locale) -> &'static str {
        let texts = &locale.texts().errors;
        match self {
            Self::Days30 => texts.range_30d,
            Self::Days90 => texts.range_90d,
            Self::Months => texts.range_months,
        }
    }
}

pub fn value_of(row: &DayRow, kind: FeedKind) -> u32 {
    let counts = &row.counts;
    match kind {
        FeedKind::Crash => counts.crashes,
        FeedKind::Bsod => counts.bsod,
        FeedKind::Whea => counts.whea,
        FeedKind::GpuReset => counts.gpu_reset,
        FeedKind::AppCrash => counts.app_crashes,
        FeedKind::NvContainer => counts.nv_container,
        FeedKind::Iommu => counts.iommu,
        FeedKind::Error => counts.errors,
    }
}

/// The smallest of 1, 2, 5, 10, 20, 50… not below the value: the top tick is a round number.
pub fn nice_ceiling(value: u32) -> u32 {
    if value <= 1 {
        return 1;
    }
    let mut magnitude = 1;
    while magnitude * 10 <= value {
        magnitude *= 10;
    }
    [1, 2, 5, 10]
        .into_iter()
        .map(|multiple| multiple * magnitude)
        .find(|step| *step >= value)
        .unwrap_or(10 * magnitude)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartSegment {
    pub kind: FeedKind,
    pub y: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartBar {
    pub key: String,
    pub label: String,
    pub x: f64,
    pub width: f64,
    pub total: u32,
    pub segments: Vec<ChartSegment>,
    /// Shown under the axis: at most ten labels so they never collide
    pub has_axis_label: bool,
    pub label_x: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartTick {
    pub value: u32,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrashChart {
    pub bars: Vec<ChartBar>,
    pub ticks: Vec<ChartTick>,
    pub baseline_y: f64,
    pub plot_left: f64,
    pub plot_right: f64,
    pub has_any_crash: bool,
}

fn total_of(row: &DayRow) -> u32 {
    CHART_KINDS.iter().map(|kind| value_of(row, *kind)).sum()
}

#[allow(
    clippy::cast_precision_loss,
    reason = "bar indices and counts are small"
)]
pub fn crash_chart(rows: &[DayRow], label_of: impl Fn(&str) -> String) -> CrashChart {
    let totals: Vec<u32> = rows.iter().map(total_of).collect();
    let top = nice_ceiling(totals.iter().copied().max().unwrap_or(0));
    let baseline_y = CHART_HEIGHT - PLOT_BOTTOM;
    let plot_height = baseline_y - PLOT_TOP;
    let plot_right = CHART_WIDTH - PLOT_RIGHT;
    let slot = (plot_right - PLOT_LEFT) / rows.len().max(1) as f64;
    let bar_width = slot * BAR_SHARE;
    let label_every = rows.len().div_ceil(MAX_X_LABELS).max(1);
    let scale = |value: u32| f64::from(value) / f64::from(top) * plot_height;

    let bars = rows
        .iter()
        .zip(&totals)
        .enumerate()
        .map(|(index, (row, total))| {
            let x = PLOT_LEFT + index as f64 * slot + (slot - bar_width) / 2.0;
            let mut stack_top = baseline_y;
            let segments = CHART_KINDS
                .iter()
                .filter(|kind| value_of(row, **kind) > 0)
                .map(|kind| {
                    let height = (scale(value_of(row, *kind)) - SEGMENT_GAP).max(1.0);
                    let y = stack_top - height;
                    stack_top = y - SEGMENT_GAP;
                    ChartSegment {
                        kind: *kind,
                        y,
                        height,
                    }
                })
                .collect();
            ChartBar {
                key: row.day.clone(),
                label: label_of(&row.day),
                x,
                width: bar_width,
                total: *total,
                segments,
                has_axis_label: index % label_every == 0,
                label_x: x + bar_width / 2.0,
            }
        })
        .collect();

    // A half tick only when it is a whole number of events
    let half = top.is_multiple_of(2).then_some(top / 2);
    let ticks = [Some(0), half, Some(top)]
        .into_iter()
        .flatten()
        .map(|value| ChartTick {
            value,
            y: baseline_y - scale(value),
        })
        .collect();

    CrashChart {
        bars,
        ticks,
        baseline_y,
        plot_left: PLOT_LEFT,
        plot_right,
        has_any_crash: totals.iter().any(|total| *total > 0),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kpi {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub hint: &'static str,
    pub is_alarming: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub key: String,
    pub label: String,
    pub values: Vec<u32>,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverviewView {
    Loading,
    Error(&'static str),
    Empty,
    Ready {
        kpis: Vec<Kpi>,
        chart_title: &'static str,
        chart: CrashChart,
        table: Vec<TableRow>,
    },
}

pub fn overview_view(
    answer: Option<&Result<HealthOverview, IpcError>>,
    range: Range,
    locale: Locale,
) -> OverviewView {
    let overview = match answer {
        None => return OverviewView::Loading,
        Some(Err(error)) => return OverviewView::Error(error.text(locale)),
        Some(Ok(overview)) => overview,
    };
    if overview.first_day.is_none() {
        return OverviewView::Empty;
    }
    let texts = &locale.texts().errors;
    let is_monthly = range == Range::Months;
    let rows = if is_monthly {
        &overview.months
    } else {
        &overview.days
    };
    let label_of = |period: &str| {
        if is_monthly {
            format::calendar_month(period, locale)
        } else {
            format::calendar_day(period, locale)
        }
    };
    let sum = |kind: FeedKind| -> u32 { rows.iter().map(|row| value_of(row, kind)).sum() };
    let hint = if is_monthly {
        texts.since_first_day
    } else {
        texts.in_range
    };
    let counted = |key, label, kind, is_alarming: fn(u32) -> bool| {
        let value = sum(kind);
        Kpi {
            key,
            label,
            value: value.to_string(),
            hint,
            is_alarming: is_alarming(value),
        }
    };
    let days_without_crash = overview.summary.days_without_crash;

    OverviewView::Ready {
        kpis: vec![
            Kpi {
                key: "daysWithoutCrash",
                label: texts.kpi_days_without_crash,
                value: days_without_crash.map_or_else(
                    || locale.texts().common.missing.to_owned(),
                    |days| days.to_string(),
                ),
                hint: texts.since_first_day,
                is_alarming: days_without_crash == Some(0),
            },
            counted("crashes", texts.kpi_crashes, FeedKind::Crash, |value| {
                value > 0
            }),
            counted("bsod", texts.kpi_bsod, FeedKind::Bsod, |value| value > 0),
            counted("whea", texts.kpi_whea, FeedKind::Whea, |value| value > 0),
            counted(
                "appCrashes",
                texts.kpi_app_crashes,
                FeedKind::AppCrash,
                |_| false,
            ),
        ],
        chart_title: if is_monthly {
            texts.chart_title_months
        } else {
            texts.chart_title
        },
        chart: crash_chart(rows, label_of),
        table: rows
            .iter()
            .map(|row| {
                let values: Vec<u32> = CHART_KINDS
                    .iter()
                    .map(|kind| value_of(row, *kind))
                    .collect();
                TableRow {
                    key: row.day.clone(),
                    label: label_of(&row.day),
                    total: values.iter().sum(),
                    values,
                }
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRow {
    pub id: String,
    pub time: String,
    pub kind_label: &'static str,
    /// The chart colour of the kind, when the chart shows it
    pub chart_kind: Option<FeedKind>,
    pub source: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedView {
    Loading,
    Error(&'static str),
    Empty(&'static str),
    Ready {
        rows: Vec<FeedRow>,
        found: String,
        page: u32,
        page_count: u32,
        position: String,
    },
}

pub fn page_count(total: u32) -> u32 {
    total.div_ceil(EVENT_PAGE_SIZE).max(1)
}

/// `page` is 1-based, as in the address.
pub fn feed_view(
    answer: Option<&Result<EventPage, IpcError>>,
    is_filtered: bool,
    page: u32,
    locale: Locale,
) -> FeedView {
    let events = match answer {
        None => return FeedView::Loading,
        Some(Err(error)) => return FeedView::Error(error.text(locale)),
        Some(Ok(events)) => events,
    };
    let texts = &locale.texts().errors;
    if events.total == 0 {
        return FeedView::Empty(if is_filtered {
            texts.feed_empty
        } else {
            texts.feed_empty_all
        });
    }
    let page_count = page_count(events.total);
    let page = page.clamp(1, page_count);
    let missing = locale.texts().common.missing;
    FeedView::Ready {
        rows: events
            .items
            .iter()
            .map(|event| {
                let kind = FeedKind::parse(&event.kind).unwrap_or(FeedKind::Error);
                FeedRow {
                    // The log and its time identify an entry; the record id stays in the core
                    id: format!(
                        "{}-{}-{}-{}-{}",
                        event.log,
                        event.time,
                        event.provider,
                        event.event_id,
                        event.detail.as_deref().unwrap_or_default()
                    ),
                    time: format::local_time(event.time).map_or_else(
                        || missing.to_owned(),
                        |time| format::date_time(time, locale),
                    ),
                    kind_label: locale.texts().kinds.of(kind),
                    chart_kind: CHART_KINDS.contains(&kind).then_some(kind),
                    source: format!("{} · {}", event.provider, event.event_id),
                    detail: event.detail.clone().unwrap_or_else(|| missing.to_owned()),
                }
            })
            .collect(),
        found: (texts.feed_found)(events.total),
        page,
        page_count,
        position: (locale.texts().common.position)(page, page_count),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpRow {
    pub name: String,
    pub time: String,
    pub code: String,
    pub size: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DumpView {
    Loading,
    Error(&'static str),
    Empty(&'static str),
    Ready(Vec<DumpRow>),
}

pub fn dump_view(answer: Option<&Result<Vec<Dump>, IpcError>>, locale: Locale) -> DumpView {
    let dumps = match answer {
        None => return DumpView::Loading,
        Some(Err(error)) => return DumpView::Error(error.text(locale)),
        Some(Ok(dumps)) => dumps,
    };
    let texts = &locale.texts().errors;
    if dumps.is_empty() {
        return DumpView::Empty(texts.dumps_empty);
    }
    let missing = locale.texts().common.missing;
    DumpView::Ready(
        dumps
            .iter()
            .map(|dump| DumpRow {
                name: dump.name.clone(),
                time: format::local_time(dump.time).map_or_else(
                    || missing.to_owned(),
                    |time| format::date_time(time, locale),
                ),
                code: dump
                    .code
                    .clone()
                    .unwrap_or_else(|| texts.dumps_unknown_code.to_owned()),
                size: format::bytes(dump.size, locale),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use vitals_ipc::{DayCounts, EventRow, HealthSummary};

    use super::*;

    fn day(day: &str, crashes: u32, bsod: u32, app_crashes: u32, errors: u32) -> DayRow {
        DayRow {
            day: day.to_owned(),
            counts: DayCounts {
                crashes,
                bsod,
                app_crashes,
                errors,
                ..DayCounts::default()
            },
        }
    }

    #[test]
    fn the_top_tick_is_a_round_number() {
        let pairs = [
            (0, 1),
            (1, 1),
            (3, 5),
            (7, 10),
            (10, 10),
            (11, 20),
            (24, 50),
            (51, 100),
        ];
        for (value, ceiling) in pairs {
            assert_eq!(nice_ceiling(value), ceiling, "{value}");
        }
    }

    #[test]
    fn stacks_the_crash_kinds_bottom_up_with_gaps_and_ignores_noise_kinds() {
        let chart = crash_chart(&[day("2026-10-27", 2, 1, 0, 40)], str::to_owned);
        let bar = &chart.bars[0];
        assert_eq!(bar.total, 3);
        assert_eq!(bar.segments.len(), 2);
        assert_eq!(bar.segments[0].kind, FeedKind::Crash);
        let crash = &bar.segments[0];
        let bsod = &bar.segments[1];
        assert!(crash.y + crash.height <= chart.baseline_y);
        assert!(bsod.y + bsod.height <= crash.y - SEGMENT_GAP + f64::EPSILON);
    }

    #[test]
    fn keeps_every_mark_inside_the_drawing() {
        let rows: Vec<DayRow> = (1..=30)
            .map(|index| {
                day(
                    &format!("2026-10-{index:02}"),
                    index % 4,
                    index % 3,
                    index % 5,
                    0,
                )
            })
            .collect();
        let chart = crash_chart(&rows, str::to_owned);
        for bar in &chart.bars {
            assert!(bar.x >= chart.plot_left && bar.x + bar.width <= chart.plot_right);
            for segment in &bar.segments {
                assert!(
                    segment.y >= PLOT_TOP - f64::EPSILON
                        && segment.y + segment.height <= chart.baseline_y
                );
            }
        }
        assert!(chart.bars.iter().filter(|bar| bar.has_axis_label).count() <= MAX_X_LABELS);
    }

    #[test]
    fn says_when_the_period_had_no_crash_at_all() {
        let chart = crash_chart(&[day("2026-10-27", 0, 0, 0, 12)], str::to_owned);
        assert!(!chart.has_any_crash);
    }

    fn overview(days: Vec<DayRow>) -> HealthOverview {
        HealthOverview {
            summary: HealthSummary {
                days_without_crash: Some(0),
                system_drive_free_gb: Some(267.6),
            },
            first_day: Some("2026-09-28".to_owned()),
            days,
            months: vec![day("2026-10", 15, 2, 24, 0)],
        }
    }

    #[test]
    fn the_counters_sum_the_period_and_mark_the_alarming() {
        let answer = Ok(overview(vec![
            day("2026-10-26", 1, 0, 3, 0),
            day("2026-10-27", 2, 1, 0, 0),
        ]));
        let OverviewView::Ready { kpis, .. } =
            overview_view(Some(&answer), Range::Days30, Locale::En)
        else {
            panic!("not ready");
        };
        let value = |key| kpis.iter().find(|kpi| kpi.key == key).unwrap();
        assert_eq!(value("crashes").value, "3");
        assert!(value("crashes").is_alarming);
        assert_eq!(value("appCrashes").value, "3");
        assert!(!value("appCrashes").is_alarming);
        assert!(value("daysWithoutCrash").is_alarming);
    }

    #[test]
    fn the_monthly_range_reads_the_months() {
        let answer = Ok(overview(vec![]));
        let OverviewView::Ready { table, .. } =
            overview_view(Some(&answer), Range::Months, Locale::Ru)
        else {
            panic!("not ready");
        };
        assert_eq!(
            table[0].label,
            format!("{} 2026", Locale::Ru.texts().months[9])
        );
        assert_eq!(table[0].total, 15 + 2 + 24);
    }

    #[test]
    fn no_first_day_is_empty() {
        let mut empty = overview(vec![]);
        empty.first_day = None;
        assert_eq!(
            overview_view(Some(&Ok(empty)), Range::Days30, Locale::En),
            OverviewView::Empty
        );
    }

    fn event(kind: &str) -> EventRow {
        EventRow {
            time: 1_793_097_600,
            log: "System".to_owned(),
            provider: "Kernel-Power".to_owned(),
            event_id: 41,
            level: 1,
            kind: kind.to_owned(),
            detail: None,
        }
    }

    #[test]
    fn pages_the_feed_by_fifty_and_keeps_a_plain_error_as_its_own_kind() {
        let answer = Ok(EventPage {
            items: vec![event("crash"), event("error")],
            total: 120,
        });
        let FeedView::Ready {
            rows,
            page,
            page_count,
            ..
        } = feed_view(Some(&answer), false, 9, Locale::En)
        else {
            panic!("not ready");
        };
        assert_eq!(page_count, 3);
        assert_eq!(page, 3);
        assert_eq!(rows[0].chart_kind, Some(FeedKind::Crash));
        assert_eq!(rows[1].chart_kind, None);
        assert_eq!(rows[1].kind_label, Locale::En.texts().kinds.error);
        assert_eq!(rows[0].detail, "—");
    }

    #[test]
    fn an_empty_feed_says_whether_a_filter_is_on() {
        let answer = Ok(EventPage {
            items: vec![],
            total: 0,
        });
        let texts = &Locale::En.texts().errors;
        assert_eq!(
            feed_view(Some(&answer), true, 1, Locale::En),
            FeedView::Empty(texts.feed_empty)
        );
        assert_eq!(
            feed_view(Some(&answer), false, 1, Locale::En),
            FeedView::Empty(texts.feed_empty_all)
        );
    }

    #[test]
    fn a_dump_without_a_code_says_so() {
        let answer = Ok(vec![Dump {
            name: "102726-12345-01.dmp".to_owned(),
            size: 2 * 1024 * 1024,
            time: 1_793_097_600,
            code: None,
        }]);
        let DumpView::Ready(rows) = dump_view(Some(&answer), Locale::En) else {
            panic!("not ready");
        };
        assert_eq!(rows[0].code, Locale::En.texts().errors.dumps_unknown_code);
        assert_eq!(rows[0].size, "2.0 MB");
    }

    #[test]
    fn a_range_reads_back_from_the_address() {
        for range in Range::ALL {
            assert_eq!(Range::parse(range.code()), Some(range));
        }
        assert_eq!(Range::parse("7d"), None);
    }
}
