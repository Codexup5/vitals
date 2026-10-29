//! The sensors screen: the counters and the readings of now, polled every two seconds while the screen is open, and
//! a chart of the load or the temperature over the live ten minutes, a day or a week. The switches stand in the
//! header, outside what the polling redraws, so they keep the focus.

use std::time::Duration;

use leptos::prelude::*;
use vitals_ipc::{SensorPoint, SensorsHistoryArgs, SensorsLive, command};

use super::network::stretch_view;
use super::widgets::{Eyebrow, Failure, Stats};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::network::{BOTTOM, CHART_HEIGHT, CHART_WIDTH, LEFT};
use crate::views::sensors::{
    ChartState, Metric, ReadingTable, SensorChart, SensorsView, Span, sensors_view,
};

const LIVE_POLL: Duration = Duration::from_secs(2);
/// The history gains a sample every five minutes
const HISTORY_REFRESH: Duration = Duration::from_secs(5 * 60);

type Answer<T> = LocalResource<Result<T, IpcError>>;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The page polls while it is open; the core samples only for a shown window (`commands::sensors`).
fn poll(live: Answer<SensorsLive>, history: Answer<Vec<SensorPoint>>, span: RwSignal<Span>) {
    if let Ok(poll) = set_interval_with_handle(move || live.refetch(), LIVE_POLL) {
        on_cleanup(move || poll.clear());
    }
    if let Ok(refresh) = set_interval_with_handle(
        move || {
            if span.get_untracked() != Span::Live {
                history.refetch();
            }
        },
        HISTORY_REFRESH,
    ) {
        on_cleanup(move || refresh.clear());
    }
}

#[component]
pub fn SensorsPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let span = RwSignal::new(Span::Live);
    let metric = RwSignal::new(Metric::Load);
    let live = LocalResource::new(|| async {
        ipc::call::<SensorsLive, _>(command::SENSORS_LIVE, &()).await
    });
    let history = LocalResource::new(move || {
        let hours = span.get().hours();
        async move {
            match hours {
                None => Ok(Vec::new()),
                Some(hours) => {
                    ipc::call::<Vec<SensorPoint>, _>(
                        command::SENSORS_HISTORY,
                        &SensorsHistoryArgs { hours },
                    )
                    .await
                }
            }
        }
    });
    poll(live, history, span);
    let view_model = move || {
        let live = live.get();
        let live = live
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale())));
        let history = history.get();
        let history = history.as_ref().map(|answer| {
            answer
                .as_ref()
                .map(Vec::as_slice)
                .map_err(|error| error.text(locale()))
        });
        sensors_view(live, history, span.get(), metric.get(), now(), locale())
    };

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().sensors.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().sensors.title}</h1>
                </div>
                <Switches span metric />
            </header>
            {move || match view_model() {
                SensorsView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                SensorsView::Error(message) => {
                    view! { <Failure message retry=locale().texts().common.retry on_retry=move || live.refetch() /> }
                        .into_any()
                }
                SensorsView::Ready(ready) => {
                    view! {
                        <Stats stats=ready.stats />
                        <Chart
                            chart=ready.chart
                            span=span.get()
                            metric=metric.get()
                            locale=locale()
                            on_retry=move || history.refetch()
                        />
                        <Readings tables=ready.tables has_gpu=ready.has_gpu has_lhm=ready.has_lhm locale=locale() />
                    }
                        .into_any()
                }
            }}
        </main>
    }
}

/// What the chart shows and over what span: local choices of this screen.
#[component]
fn Switches(span: RwSignal<Span>, metric: RwSignal<Metric>) -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    view! {
        <div class="switches">
            <div class="toggles" role="group" aria-label=move || locale().texts().sensors.metrics_label>
                {Metric::ALL
                    .into_iter()
                    .map(|choice| {
                        view! {
                            <button
                                type="button"
                                aria-pressed=move || (metric.get() == choice).to_string()
                                on:click=move |_| metric.set(choice)
                            >
                                {move || choice.label(locale())}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
            <div class="toggles" role="group" aria-label=move || locale().texts().sensors.spans_label>
                {Span::ALL
                    .into_iter()
                    .map(|choice| {
                        view! {
                            <button
                                type="button"
                                aria-pressed=move || (span.get() == choice).to_string()
                                on:click=move |_| span.set(choice)
                            >
                                {move || choice.label(locale())}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[component]
fn Chart(
    chart: ChartState,
    span: Span,
    metric: Metric,
    locale: Locale,
    on_retry: impl Fn() + 'static,
) -> impl IntoView {
    let texts = &locale.texts().sensors;
    let title = (texts.chart_title)(metric.label(locale), span.label(locale));
    let hint = if span == Span::Live {
        texts.chart_hint_live
    } else {
        texts.chart_hint_history
    };
    view! {
        <section class="section" aria-labelledby="sensors-chart-title">
            <h2 id="sensors-chart-title">{title.clone()}</h2>
            <p class="muted">{hint}</p>
            {match chart {
                ChartState::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                ChartState::Error(message) => {
                    view! { <Failure message retry=locale.texts().common.retry on_retry /> }.into_any()
                }
                ChartState::Empty(text) => view! { <p class="muted">{text}</p> }.into_any(),
                ChartState::Ready(chart) => view! { <Plot chart title /> }.into_any(),
            }}
        </section>
    }
}

#[component]
fn Plot(chart: SensorChart, title: String) -> impl IntoView {
    view! {
        <figure class="chart card">
            <ul class="legend">
                {chart
                    .lines
                    .iter()
                    .map(|line| {
                        view! {
                            <li>
                                <svg class="swatch-line" viewBox="0 0 24 12" aria-hidden="true">
                                    <line x1="0" y1="6" x2="24" y2="6" class=line.class />
                                </svg>
                                {line.label}
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
            <svg class="plot" viewBox=format!("0 0 {CHART_WIDTH} {CHART_HEIGHT}") role="img" aria-label=title>
                {chart
                    .ticks
                    .into_iter()
                    .map(|tick| {
                        view! {
                            <line class="chart-grid" x1=LEFT x2=CHART_WIDTH y1=tick.y y2=tick.y />
                            <text class="chart-tick" x=LEFT - 8.0 y=tick.y + 4.0 text-anchor="end">{tick.label}</text>
                        }
                    })
                    .collect_view()}
                {
                    // The last mark ends at the right edge of the plot: a clock with seconds would be cut there
                    let last = chart.times.len().saturating_sub(1);
                    chart
                        .times
                        .into_iter()
                        .enumerate()
                        .map(|(index, time)| {
                            let anchor = if index == last { "end" } else { "middle" };
                            view! {
                                <text class="chart-tick" x=time.x y=CHART_HEIGHT - 4.0 text-anchor=anchor>{time.label}</text>
                            }
                        })
                        .collect_view()
                }
                {chart
                    .lines
                    .into_iter()
                    .map(|line| {
                        let class = line.class;
                        line.stretches.into_iter().map(move |stretch| stretch_view(stretch, class)).collect_view()
                    })
                    .collect_view()}
                {chart
                    .points
                    .into_iter()
                    .map(|point| {
                        view! {
                            <rect class="chart-hit" x=point.x y="0" width=point.width height=CHART_HEIGHT - BOTTOM>
                                <title>{point.title}</title>
                            </rect>
                        }
                    })
                    .collect_view()}
            </svg>
        </figure>
    }
}

#[component]
fn Readings(
    tables: Vec<ReadingTable>,
    has_gpu: bool,
    has_lhm: bool,
    locale: Locale,
) -> impl IntoView {
    let texts = &locale.texts().sensors;
    view! {
        <section class="section" aria-labelledby="readings-title">
            <h2 id="readings-title">{texts.readings_title}</h2>
            <div class="readings">
                {tables
                    .into_iter()
                    .map(|table| {
                        view! {
                            <div class="table-card">
                                <table>
                                    <caption>{table.title}</caption>
                                    <tbody>
                                        {table
                                            .rows
                                            .into_iter()
                                            .map(|row| {
                                                view! {
                                                    <tr>
                                                        <th scope="row">{row.label}</th>
                                                        <td class=if row.is_missing { "muted" } else { "mono" }>{row.value}</td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
            {if has_lhm {
                view! { <p class="muted">{texts.lhm_source}</p> }.into_any()
            } else {
                view! { <LhmSetup locale /> }.into_any()
            }}
            {(!has_gpu).then(|| view! { <p class="muted">{texts.no_gpu_note}</p> })}
        </section>
    }
}

/// How to turn on what needs a driver: the panel installs none, the user runs `LibreHardwareMonitor` and its server.
#[component]
fn LhmSetup(locale: Locale) -> impl IntoView {
    let texts = &locale.texts().sensors;
    view! {
        <div class="notice">
            <h3>{texts.lhm_setup_title}</h3>
            <p>{texts.lhm_setup_intro}</p>
            <ol class="lhm-steps">
                {texts.lhm_setup_steps.into_iter().map(|step| view! { <li>{step}</li> }).collect_view()}
            </ol>
            <p class="muted">{texts.lhm_setup_outro}</p>
        </div>
    }
}
