//! The network screen: the counters, the latency chart of the day with its table, the adapters and the speed test.
//! The panel measures every five minutes by itself; "Measure now" adds a sample, the speed test runs only on its
//! button and is polled once a second while it runs.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{DevicesOverview, NetworkOverview, SpeedStatus, command};

use super::widgets::{Eyebrow, Failure, Stats, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::disks::Tone;
use crate::views::network::{
    AdapterRow, CHART_HEIGHT, CHART_WIDTH, LatencyChart, NetworkView, RecentRow, Stretch,
    devices_view, network_view, speed_line,
};

const STATUS_POLL: Duration = Duration::from_secs(1);
/// The screen reads the samples again as often as the panel takes them
const REFRESH: Duration = Duration::from_secs(5 * 60);

type Answer<T> = LocalResource<Result<T, IpcError>>;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn is_testing(status: Answer<SpeedStatus>) -> bool {
    status
        .get()
        .and_then(Result::ok)
        .is_some_and(|status| status.is_running)
}

/// Polls the speed test once a second while it runs.
fn watch_speed(status: Answer<SpeedStatus>) {
    let was_running = StoredValue::new(false);
    Effect::new(move |_| was_running.set_value(is_testing(status)));
    if let Ok(poll) = set_interval_with_handle(
        move || {
            if was_running.get_value() {
                status.refetch();
            }
        },
        STATUS_POLL,
    ) {
        on_cleanup(move || poll.clear());
    }
}

#[component]
pub fn NetworkPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let overview = LocalResource::new(|| async {
        ipc::call::<NetworkOverview, _>(command::NETWORK_OVERVIEW, &()).await
    });
    let speed = LocalResource::new(|| async {
        ipc::call::<SpeedStatus, _>(command::SPEED_TEST_STATUS, &()).await
    });
    watch_speed(speed);
    if let Ok(refresh) = set_interval_with_handle(move || overview.refetch(), REFRESH) {
        on_cleanup(move || refresh.clear());
    }
    // A measurement now answers with the whole overview, which replaces the one shown
    let measured = RwSignal::new(None::<Result<NetworkOverview, IpcError>>);
    let is_measuring = RwSignal::new(false);
    let measure = move || {
        is_measuring.set(true);
        spawn_local(async move {
            let answer = ipc::call::<NetworkOverview, _>(command::NETWORK_PROBE, &()).await;
            // The page may be gone by the time the pings end
            if measured.try_set(Some(answer)).is_none() {
                is_measuring.set(false);
            }
        });
    };
    let view_model = move || {
        let answer = measured.get().or_else(|| overview.get());
        let answer = answer
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale())));
        let speed = speed.get().and_then(Result::ok);
        network_view(answer, speed.as_ref(), now(), locale())
    };

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().network.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().network.title}</h1>
                    <div class="scan">
                        <button
                            type="button"
                            disabled=move || is_measuring.get()
                            aria-busy=move || is_measuring.get().to_string()
                            on:click=move |_| measure()
                        >
                            {move || locale().texts().network.measure}
                        </button>
                    </div>
                </div>
            </header>
            {move || {
                measured
                    .get()
                    .is_some_and(|answer| answer.is_err())
                    .then(|| view! { <p role="alert">{locale().texts().network.measure_failed}</p> })
            }}
            {move || match view_model() {
                NetworkView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                NetworkView::Error(message) => {
                    view! { <Failure message retry=locale().texts().common.retry on_retry=move || overview.refetch() /> }
                        .into_any()
                }
                NetworkView::Ready(ready) => {
                    view! {
                        <Stats stats=ready.stats />
                        <Latency chart=ready.chart recent=ready.recent host=ready.internet_host locale=locale() />
                        <SpeedTest status=speed locale=Signal::derive(locale) />
                        <Devices locale=Signal::derive(locale) />
                        <Adapters rows=ready.adapters locale=locale() />
                    }
                        .into_any()
                }
            }}
        </main>
    }
}

/// A stretch of a line, or the dot of a measurement that stands alone.
pub(super) fn stretch_view(stretch: Stretch, class: &'static str) -> AnyView {
    match stretch {
        Stretch::Line(points) => view! { <polyline class=class points=points /> }.into_any(),
        Stretch::Dot { x, y } => {
            view! { <circle class=format!("{class} dot") cx=x cy=y r="4" /> }.into_any()
        }
    }
}

#[component]
fn Latency(
    chart: Option<LatencyChart>,
    recent: Vec<RecentRow>,
    host: String,
    locale: Locale,
) -> impl IntoView {
    let texts = &locale.texts().network;
    let internet = (texts.internet)(&host);
    view! {
        <section class="section" aria-labelledby="latency-title">
            <h2 id="latency-title">{texts.chart_title}</h2>
            <p class="muted">{texts.chart_hint}</p>
            {match chart {
                None => view! { <p class="muted">{texts.no_samples}</p> }.into_any(),
                Some(chart) => {
                    view! {
                        <figure class="chart card">
                            <ul class="legend">
                                <li>
                                    <svg class="swatch-line" viewBox="0 0 24 12" aria-hidden="true">
                                        <line x1="0" y1="6" x2="24" y2="6" class="line-gateway" />
                                    </svg>
                                    {texts.router}
                                </li>
                                <li>
                                    <svg class="swatch-line" viewBox="0 0 24 12" aria-hidden="true">
                                        <line x1="0" y1="6" x2="24" y2="6" class="line-internet" />
                                    </svg>
                                    {internet.clone()}
                                </li>
                            </ul>
                            <svg
                                class="plot"
                                viewBox=format!("0 0 {CHART_WIDTH} {CHART_HEIGHT}")
                                role="img"
                                aria-label=texts.chart_title
                            >
                                {chart
                                    .ticks
                                    .into_iter()
                                    .map(|tick| {
                                        view! {
                                            <line class="chart-grid" x1="48" x2=CHART_WIDTH y1=tick.y y2=tick.y />
                                            <text class="chart-tick" x="40" y=tick.y + 4.0 text-anchor="end">{tick.label}</text>
                                        }
                                    })
                                    .collect_view()}
                                {chart
                                    .times
                                    .into_iter()
                                    .map(|time| {
                                        view! {
                                            <text class="chart-tick" x=time.x y=CHART_HEIGHT - 4.0 text-anchor="middle">{time.label}</text>
                                        }
                                    })
                                    .collect_view()}
                                {chart.gateway.into_iter().map(|stretch| stretch_view(stretch, "line-gateway")).collect_view()}
                                {chart.internet.into_iter().map(|stretch| stretch_view(stretch, "line-internet")).collect_view()}
                                {chart
                                    .points
                                    .into_iter()
                                    .map(|point| {
                                        view! {
                                            <rect class="chart-hit" x=point.x y="0" width=point.width height=CHART_HEIGHT - 24.0>
                                                <title>{point.title}</title>
                                            </rect>
                                        }
                                    })
                                    .collect_view()}
                            </svg>
                        </figure>
                    }
                        .into_any()
                }
            }}
            {(!recent.is_empty())
                .then(|| {
                    view! {
                        <div class="table-card">
                            <table>
                                <caption>{texts.recent_title}</caption>
                                <thead>
                                    <tr>
                                        <th scope="col">{texts.time}</th>
                                        <th scope="col">{texts.router}</th>
                                        <th scope="col">{internet}</th>
                                        <th scope="col">{texts.loss}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {recent
                                        .into_iter()
                                        .map(|row| {
                                            view! {
                                                <tr>
                                                    <td class="mono">{row.time}</td>
                                                    <td class="mono">{row.gateway}</td>
                                                    <td class="mono">{row.internet}</td>
                                                    <td class="mono">{row.loss}</td>
                                                </tr>
                                            }
                                        })
                                        .collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }
                })}
        </section>
    }
}

#[component]
fn SpeedTest(status: Answer<SpeedStatus>, locale: Signal<Locale>) -> impl IntoView {
    let texts = move || &locale.get().texts().network;
    let failed = RwSignal::new(false);
    let start = move || {
        failed.set(false);
        spawn_local(async move {
            let answer: Result<bool, IpcError> = ipc::call(command::SPEED_TEST_START, &()).await;
            if failed.try_set(answer.is_err()).is_none() {
                status.refetch();
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="speed-title">
            <h2 id="speed-title">{move || texts().speed_title}</h2>
            <p class="muted">{move || texts().speed_hint}</p>
            <div class="card speed">
                {move || {
                    let busy = is_testing(status);
                    view! {
                        <button type="button" disabled=busy aria-busy=busy.to_string() on:click=move |_| start()>
                            {if busy { texts().speed_running } else { texts().speed_start }}
                        </button>
                    }
                }}
                {move || {
                    status
                        .get()
                        .and_then(Result::ok)
                        .filter(|status| !status.is_running)
                        .and_then(|status| speed_line(&status, locale.get()))
                        .map(|(text, tone)| view! { <p role="status" class="mode-line" data-tone=tone.code()>{text}</p> })
                }}
                {move || failed.get().then(|| view! { <p role="alert">{texts().speed_start_failed}</p> })}
            </div>
        </section>
    }
}

fn is_scanning(devices: Answer<DevicesOverview>) -> bool {
    devices
        .get()
        .and_then(Result::ok)
        .is_some_and(|overview| overview.is_scanning)
}

#[component]
fn Devices(locale: Signal<Locale>) -> impl IntoView {
    let texts = move || &locale.get().texts().network;
    let devices = LocalResource::new(|| async {
        ipc::call::<DevicesOverview, _>(command::DEVICES_OVERVIEW, &()).await
    });
    // While a scan runs, the list is read again every second until it ends
    let was_scanning = StoredValue::new(false);
    Effect::new(move |_| was_scanning.set_value(is_scanning(devices)));
    if let Ok(poll) = set_interval_with_handle(
        move || {
            if was_scanning.get_value() {
                devices.refetch();
            }
        },
        STATUS_POLL,
    ) {
        on_cleanup(move || poll.clear());
    }
    let failed = RwSignal::new(false);
    let scan = move || {
        failed.set(false);
        spawn_local(async move {
            let answer: Result<bool, IpcError> = ipc::call(command::DEVICES_SCAN, &()).await;
            if failed.try_set(answer.is_err()).is_none() {
                devices.refetch();
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="devices-title">
            <h2 id="devices-title">{move || texts().devices_title}</h2>
            <p class="muted">{move || texts().devices_hint}</p>
            {move || failed.get().then(|| view! { <p role="alert">{texts().devices_failed}</p> })}
            {move || match devices.get() {
                None => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                Some(Err(error)) => {
                    view! { <Failure message=error.text(locale.get()) retry=texts().devices_scan on_retry=move || devices.refetch() /> }
                        .into_any()
                }
                Some(Ok(overview)) => {
                    let ready = devices_view(&overview, locale.get());
                    let busy = ready.is_scanning;
                    view! {
                        <div class="card speed">
                            <button type="button" disabled=busy aria-busy=busy.to_string() on:click=move |_| scan()>
                                {if busy { texts().devices_scanning } else { texts().devices_scan }}
                            </button>
                            <p class="muted" role="status">{ready.summary} " · " {ready.last_scan}</p>
                        </div>
                        {(!ready.rows.is_empty())
                            .then(|| {
                                view! {
                                    <div class="card">
                                        <ul class="rows">
                                            {ready
                                                .rows
                                                .into_iter()
                                                .map(|row| {
                                                    view! {
                                                        <li class="row">
                                                            <div class="row-main">
                                                                <span class="row-title">{row.title}</span>
                                                                <span class="mono nowrap muted">{row.seen}</span>
                                                            </div>
                                                            <span class="mono muted wrap">{row.address}</span>
                                                            {row.detail.map(|detail| view! { <span class="muted">{detail}</span> })}
                                                            <div class="row-tags">
                                                                {row
                                                                    .tags
                                                                    .into_iter()
                                                                    .map(|(word, tone)| view! { <Tag tone>{word}</Tag> })
                                                                    .collect_view()}
                                                            </div>
                                                        </li>
                                                    }
                                                })
                                                .collect_view()}
                                        </ul>
                                    </div>
                                }
                            })}
                    }
                        .into_any()
                }
            }}
        </section>
    }
}

#[component]
fn Adapters(rows: Vec<AdapterRow>, locale: Locale) -> impl IntoView {
    let texts = &locale.texts().network;
    let is_empty = rows.is_empty();
    view! {
        <section class="section" aria-labelledby="adapters-title">
            <h2 id="adapters-title">{texts.adapters_title}</h2>
            {if is_empty {
                view! { <p class="muted">{texts.no_adapters}</p> }.into_any()
            } else {
                view! {
                    <div class="card">
                        <ul class="rows">
                            {rows
                                .into_iter()
                                .map(|row| {
                                    view! {
                                        <li class="row">
                                            <div class="row-main">
                                                <span class="row-title">{row.name}</span>
                                                {row.link.map(|link| view! { <span class="mono nowrap">{link}</span> })}
                                            </div>
                                            <span class="muted">{row.description}</span>
                                            <div class="row-tags">
                                                <Tag tone=Tone::Accent>{row.kind}</Tag>
                                                <span class="mono muted">{row.addresses}</span>
                                            </div>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    </div>
                }
                    .into_any()
            }}
        </section>
    }
}
