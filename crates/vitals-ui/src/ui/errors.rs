//! The errors screen: the counters and the crash chart of a range, the event feed, the crash dumps. The range, the
//! feed filter and the feed page live in the address, so they survive a reload.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_query_map};
use vitals_ipc::{
    Dump, EventPage, FeedKind, HealthEventsArgs, HealthOverview, HealthOverviewArgs,
    RevealMinidumpArgs, command,
};

use super::widgets::{Eyebrow, Failure};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::errors::{
    CHART_HEIGHT, CHART_KINDS, CHART_WIDTH, CrashChart, DumpView, FeedView, OverviewView, Range,
    TableRow, dump_view, feed_view, overview_view,
};

/// What the address holds: `?range=90d&kinds=crash,bsod&page=2`
#[derive(Debug, Clone, PartialEq, Eq)]
struct Search {
    range: Range,
    kinds: Vec<FeedKind>,
    /// 1-based
    page: u32,
}

impl Search {
    fn to_query(&self) -> String {
        let kinds: Vec<&str> = self.kinds.iter().map(|kind| kind.as_str()).collect();
        format!(
            "/errors?range={}&kinds={}&page={}",
            self.range.code(),
            kinds.join(","),
            self.page
        )
    }
}

/// Moves to another address of the errors screen; built once per screen, where the router context is
type Navigate = StoredValue<Box<dyn Fn(&str)>, LocalStorage>;

fn change_search(navigate: Navigate, search: Memo<Search>, change: impl FnOnce(&mut Search)) {
    let mut next = search.get_untracked();
    change(&mut next);
    navigate.with_value(|navigate| navigate(&next.to_query()));
}

fn use_search() -> Memo<Search> {
    let query = use_query_map();
    Memo::new(move |_| {
        query.with(|query| Search {
            range: query
                .get("range")
                .as_deref()
                .and_then(Range::parse)
                .unwrap_or_default(),
            kinds: query
                .get("kinds")
                .map(|kinds| kinds.split(',').filter_map(FeedKind::parse).collect())
                .unwrap_or_default(),
            page: query
                .get("page")
                .and_then(|page| page.parse().ok())
                .filter(|page| *page > 0)
                .unwrap_or(1),
        })
    })
}

#[component]
pub fn ErrorsPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let search = use_search();
    let navigate: Navigate = {
        let navigate = use_navigate();
        StoredValue::new_local(Box::new(move |url: &str| {
            navigate(url, NavigateOptions::default());
        }))
    };

    let events = LocalResource::new(move || {
        let Search { kinds, page, .. } = search.get();
        async move {
            let args = HealthEventsArgs {
                kinds: kinds.iter().map(|kind| kind.as_str().to_owned()).collect(),
                from: None,
                to: None,
                page: page - 1,
            };
            ipc::call::<EventPage, _>(command::HEALTH_EVENTS, &args).await
        }
    });

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().errors.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().errors.title}</h1>
                </div>
                <div class="toggles" role="group" aria-label=move || locale().texts().errors.ranges_label>
                    {Range::ALL
                        .into_iter()
                        .map(|range| {
                            view! {
                                <button
                                    type="button"
                                    aria-pressed=move || (search.get().range == range).to_string()
                                    on:click=move |_| change_search(navigate, search, |search| search.range = range)
                                >
                                    {move || range.label(locale())}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </header>

            <Overview search locale=Signal::derive(locale) />

            <EventFeed search navigate events locale=Signal::derive(locale) />

            <Minidumps locale=Signal::derive(locale) />
        </main>
    }
}

#[component]
fn Overview(search: Memo<Search>, locale: Signal<Locale>) -> impl IntoView {
    let overview = LocalResource::new(move || {
        let days = search.get().range.days();
        async move {
            ipc::call::<HealthOverview, _>(command::HEALTH_OVERVIEW, &HealthOverviewArgs { days })
                .await
        }
    });
    view! {
        {move || {
            let range = search.get().range;
            match overview_view(overview.get().as_ref(), range, locale.get()) {
                OverviewView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                OverviewView::Error(message) => {
                    view! {
                        <Failure
                            message
                            retry=locale.get().texts().common.retry
                            on_retry=move || overview.refetch()
                        />
                    }
                        .into_any()
                }
                OverviewView::Empty => {
                    let texts = &locale.get().texts().errors;
                    view! {
                        <div class="notice">
                            <h2>{texts.empty_title}</h2>
                            <p>{texts.empty_hint}</p>
                        </div>
                    }
                        .into_any()
                }
                OverviewView::Ready { kpis, chart_title, chart, table } => {
                    view! {
                        <ul class="stats">
                            {kpis
                                .into_iter()
                                .map(|kpi| {
                                    view! {
                                        <li class="stat" data-alarming=kpi.is_alarming.then_some("true")>
                                            <span class="stat-value">{kpi.value}</span>
                                            <span>{kpi.label}</span>
                                            <span class="muted">{kpi.hint}</span>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                        <ChartFigure title=chart_title chart table locale=locale.get() />
                    }
                        .into_any()
                }
            }
        }}

    }
}

#[component]
fn Minidumps(locale: Signal<Locale>) -> impl IntoView {
    let dumps =
        LocalResource::new(|| async { ipc::call::<Vec<Dump>, _>(command::MINIDUMPS, &()).await });
    let reveal_failed = RwSignal::new(false);
    let reveal = move |name: String| {
        spawn_local(async move {
            let answer: Result<(), IpcError> =
                ipc::call(command::REVEAL_MINIDUMP, &RevealMinidumpArgs { name }).await;
            reveal_failed.try_set(answer.is_err());
        });
    };
    view! {
        <section class="section" aria-labelledby="minidumps-title">
            <h2 id="minidumps-title">{move || locale.get().texts().errors.dumps_title}</h2>
            <p class="muted mono">{move || locale.get().texts().errors.dumps_folder}</p>
            {move || {
                reveal_failed
                    .get()
                    .then(|| view! { <p role="alert" class="muted">{locale.get().texts().errors.dumps_reveal_failed}</p> })
            }}
            {move || {
                let texts = &locale.get().texts().errors;
                match dump_view(dumps.get().as_ref(), locale.get()) {
                    DumpView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                    DumpView::Error(message) => {
                        view! {
                            <Failure message retry=locale.get().texts().common.retry on_retry=move || dumps.refetch() />
                        }
                            .into_any()
                    }
                    DumpView::Empty(message) => view! { <p class="muted">{message}</p> }.into_any(),
                    DumpView::Ready(rows) => {
                        view! {
                            <div class="table-card">
                                <table>
                                    <thead>
                                        <tr>
                                            <th scope="col">{texts.dumps_name}</th>
                                            <th scope="col">{texts.dumps_time}</th>
                                            <th scope="col">{texts.dumps_code}</th>
                                            <th scope="col">{texts.dumps_size}</th>
                                            <th scope="col">
                                                <span class="visually-hidden">{texts.dumps_reveal}</span>
                                            </th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {rows
                                            .into_iter()
                                            .map(|row| {
                                                let name = row.name.clone();
                                                view! {
                                                    <tr>
                                                        <td class="mono">{row.name}</td>
                                                        <td class="mono">{row.time}</td>
                                                        <td class="mono">{row.code}</td>
                                                        <td class="mono">{row.size}</td>
                                                        <td>
                                                            <button type="button" on:click=move |_| reveal(name.clone())>
                                                                {texts.dumps_reveal}
                                                            </button>
                                                        </td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        }
                            .into_any()
                    }
                }
            }}
        </section>
    }
}

#[component]
fn EventFeed(
    search: Memo<Search>,
    navigate: Navigate,
    events: LocalResource<Result<EventPage, IpcError>>,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = move || &locale.get().texts().errors;
    view! {
        <section class="section" aria-labelledby="event-feed-title">
            <h2 id="event-feed-title">{move || texts().feed_title}</h2>
            <div class="toggles" role="group" aria-label=move || texts().feed_filter>
                <button
                    type="button"
                    aria-pressed=move || search.get().kinds.is_empty().to_string()
                    on:click=move |_| {
                        change_search(
                            navigate,
                            search,
                            |search| {
                                search.kinds.clear();
                                search.page = 1;
                            },
                        );
                    }
                >
                    {move || texts().feed_all}
                </button>
                {FeedKind::ALL
                    .into_iter()
                    .map(|kind| {
                        view! {
                            <button
                                type="button"
                                aria-pressed=move || search.get().kinds.contains(&kind).to_string()
                                on:click=move |_| {
                                    change_search(
                                        navigate,
                                        search,
                                        |search| {
                                            if let Some(index) = search.kinds.iter().position(|active| *active == kind) {
                                                search.kinds.remove(index);
                                            } else {
                                                search.kinds.push(kind);
                                            }
                                            search.page = 1;
                                        },
                                    );
                                }
                            >
                                {move || locale.get().texts().kinds.of(kind)}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
            {move || {
                let current = search.get();
                let locale = locale.get();
                let texts = &locale.texts().errors;
                let common = &locale.texts().common;
                match feed_view(events.get().as_ref(), !current.kinds.is_empty(), current.page, locale) {
                    FeedView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                    FeedView::Error(message) => {
                        view! { <Failure message retry=common.retry on_retry=move || events.refetch() /> }.into_any()
                    }
                    FeedView::Empty(message) => view! { <p class="muted">{message}</p> }.into_any(),
                    FeedView::Ready { rows, found, page, page_count, position } => {
                        view! {
                            <p class="muted">{found}</p>
                            <div class="table-card">
                                <table>
                                    <thead>
                                        <tr>
                                            <th scope="col">{texts.feed_time}</th>
                                            <th scope="col">{texts.feed_kind}</th>
                                            <th scope="col">{texts.feed_source}</th>
                                            <th scope="col">{texts.feed_detail}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {rows
                                            .into_iter()
                                            .map(|row| {
                                                view! {
                                                    <tr>
                                                        <td class="mono">{row.time}</td>
                                                        <td>
                                                            <span
                                                                class="kind"
                                                                data-kind=row.chart_kind.map(FeedKind::as_str)
                                                            >
                                                                {row.kind_label}
                                                            </span>
                                                        </td>
                                                        <td class="mono">{row.source}</td>
                                                        <td class="mono wrap">{row.detail}</td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </tbody>
                                </table>
                            </div>
                            <nav class="pages" aria-label=common.pages>
                                <button
                                    type="button"
                                    disabled={page <= 1}
                                    on:click=move |_| change_search(navigate, search, |search| search.page = page - 1)
                                >
                                    {common.previous}
                                </button>
                                <span class="mono">{position}</span>
                                <button
                                    type="button"
                                    disabled={page >= page_count}
                                    on:click=move |_| change_search(navigate, search, |search| search.page = page + 1)
                                >
                                    {common.next}
                                </button>
                            </nav>
                        }
                            .into_any()
                    }
                }
            }}
        </section>
    }
}

/// Colour and texture together: a kind never reads by colour alone (the brief, §1 p.4).
fn fill_of(kind: FeedKind) -> &'static str {
    match kind {
        FeedKind::Bsod => "url(#vitals-texture-bsod)",
        FeedKind::Whea => "url(#vitals-texture-whea)",
        FeedKind::GpuReset => "url(#vitals-texture-gpu-reset)",
        FeedKind::AppCrash => "url(#vitals-texture-app-crash)",
        _ => "url(#vitals-texture-crash)",
    }
}

#[component]
fn ChartFigure(
    title: &'static str,
    chart: CrashChart,
    table: Vec<TableRow>,
    locale: Locale,
) -> impl IntoView {
    let texts = &locale.texts().errors;
    let kinds = &locale.texts().kinds;
    let has_any_crash = chart.has_any_crash;
    let plot_left = chart.plot_left;
    let plot_right = chart.plot_right;
    let baseline_y = chart.baseline_y;

    view! {
        <figure class="card chart">
            <figcaption class="chart-caption">
                <h2>{title}</h2>
                {(!has_any_crash).then(|| view! { <p class="muted">{texts.chart_calm}</p> })}
            </figcaption>
            <p class="muted">{texts.chart_axis}</p>
            <ul class="legend" aria-label=texts.chart_legend>
                {CHART_KINDS
                    .iter()
                    .map(|kind| {
                        view! {
                            <li>
                                <svg class="swatch" viewBox="0 0 12 12" aria-hidden="true">
                                    <rect width="12" height="12" rx="2" fill=fill_of(*kind)></rect>
                                </svg>
                                {kinds.of(*kind)}
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
            <svg class="plot" viewBox=format!("0 0 {CHART_WIDTH} {CHART_HEIGHT}") role="img" aria-label=title>
                <ChartTextures />
                {chart
                    .ticks
                    .into_iter()
                    .map(|tick| {
                        view! {
                            <g>
                                <line x1=plot_left x2=plot_right y1=tick.y y2=tick.y class="chart-grid"></line>
                                <text
                                    x=plot_left - 6.0
                                    y=tick.y
                                    class="chart-tick"
                                    text-anchor="end"
                                    dominant-baseline="middle"
                                >
                                    {tick.value}
                                </text>
                            </g>
                        }
                    })
                    .collect_view()}
                {chart
                    .bars
                    .into_iter()
                    .map(|bar| {
                        let x = bar.x;
                        let width = bar.width;
                        view! {
                            <g>
                                <title>{format!("{}: {}", bar.label, bar.total)}</title>
                                <rect x=x y=0 width=width height=baseline_y class="chart-hit"></rect>
                                {bar
                                    .segments
                                    .into_iter()
                                    .map(|segment| {
                                        view! {
                                            <rect
                                                x=x
                                                y=segment.y
                                                width=width
                                                height=segment.height
                                                fill=fill_of(segment.kind)
                                            ></rect>
                                        }
                                    })
                                    .collect_view()}
                                {bar
                                    .has_axis_label
                                    .then(|| {
                                        view! {
                                            <text
                                                x=bar.label_x
                                                y=baseline_y + 18.0
                                                class="chart-tick"
                                                text-anchor="middle"
                                            >
                                                {bar.label.clone()}
                                            </text>
                                        }
                                    })}
                            </g>
                        }
                    })
                    .collect_view()}
                <line x1=plot_left x2=plot_right y1=baseline_y y2=baseline_y class="chart-axis"></line>
            </svg>
            <details class="chart-table">
                <summary>{texts.chart_table}</summary>
                <div class="table-card">
                    <table>
                        <thead>
                            <tr>
                                <th scope="col">{texts.chart_period}</th>
                                {CHART_KINDS
                                    .iter()
                                    .map(|kind| view! { <th scope="col">{kinds.of(*kind)}</th> })
                                    .collect_view()}
                                <th scope="col">{texts.chart_total}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {table
                                .into_iter()
                                .map(|row| {
                                    view! {
                                        <tr>
                                            <th scope="row">{row.label}</th>
                                            {row.values.into_iter().map(|value| view! { <td>{value}</td> }).collect_view()}
                                            <td>{row.total}</td>
                                        </tr>
                                    }
                                })
                                .collect_view()}
                        </tbody>
                    </table>
                </div>
            </details>
        </figure>
    }
}

#[component]
fn ChartTextures() -> impl IntoView {
    view! {
        <defs>
            <pattern id="vitals-texture-crash" width="6" height="6" patternUnits="userSpaceOnUse">
                <rect width="6" height="6" class="fill-crash"></rect>
            </pattern>
            <pattern
                id="vitals-texture-bsod"
                width="6"
                height="6"
                patternUnits="userSpaceOnUse"
                patternTransform="rotate(45)"
            >
                <rect width="6" height="6" class="fill-bsod"></rect>
                <rect width="2" height="6" class="fill-stripe"></rect>
            </pattern>
            <pattern
                id="vitals-texture-whea"
                width="6"
                height="6"
                patternUnits="userSpaceOnUse"
                patternTransform="rotate(-45)"
            >
                <rect width="6" height="6" class="fill-whea"></rect>
                <rect width="2" height="6" class="fill-stripe"></rect>
            </pattern>
            <pattern id="vitals-texture-gpu-reset" width="6" height="6" patternUnits="userSpaceOnUse">
                <rect width="6" height="6" class="fill-gpu-reset"></rect>
                <circle cx="3" cy="3" r="1.2" class="fill-stripe"></circle>
            </pattern>
            <pattern id="vitals-texture-app-crash" width="6" height="6" patternUnits="userSpaceOnUse">
                <rect width="6" height="6" class="fill-app-crash"></rect>
                <rect width="6" height="2" class="fill-stripe"></rect>
            </pattern>
        </defs>
    }
}
