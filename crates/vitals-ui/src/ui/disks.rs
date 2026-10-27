//! The disks screen: the counters, the volumes, the drives, the treemap of the home and the cleanup preview. The
//! scan runs in the core; while it runs the screen asks how far it got once a second.

use std::collections::BTreeSet;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{Candidate, Category, DisksOverview, RevealPathArgs, ScanStatus, command};

use super::cleanup::{CleanupBar, Stage};
use super::widgets::{Eyebrow, Failure, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::disks::{
    DisksView, DriveRow, Kpi, LegendEntry, RecommendationGroup, TREEMAP_HEIGHT, TREEMAP_WIDTH,
    TileFill, Tone, TreemapMode, TreemapTile, VolumeRow, disks_view, treemap,
};

const SYSTEM_DRIVE: &str = "C";
const SCAN_POLL: Duration = Duration::from_secs(1);

type Answer<T> = LocalResource<Result<T, IpcError>>;

/// The cleanup candidates of the last scan, or none before the first scan
fn candidates_of(overview: Answer<DisksOverview>) -> Vec<Candidate> {
    overview
        .get()
        .and_then(Result::ok)
        .and_then(|overview| overview.scan)
        .map(|scan| scan.candidates)
        .unwrap_or_default()
}

fn is_running(status: Answer<ScanStatus>) -> bool {
    status
        .get()
        .and_then(Result::ok)
        .is_some_and(|status| status.is_running)
}

/// The status of the scan, polled once a second while it runs; when it turns from running to idle, the overview is
/// read again for the new result.
fn watch_scan(overview: Answer<DisksOverview>) -> Answer<ScanStatus> {
    let status = LocalResource::new(|| async {
        ipc::call::<ScanStatus, _>(command::SCAN_STATUS, &()).await
    });
    let was_running = StoredValue::new(false);
    Effect::new(move |_| {
        let running = is_running(status);
        if was_running.get_value() && !running {
            overview.refetch();
        }
        was_running.set_value(running);
    });
    if let Ok(poll) = set_interval_with_handle(
        move || {
            if was_running.get_value() {
                status.refetch();
            }
        },
        SCAN_POLL,
    ) {
        on_cleanup(move || poll.clear());
    }
    status
}

/// Opens Explorer on a path of the last scan; a failure is shown by the recommendations
fn reveal_path(path: String, failed: RwSignal<bool>) {
    spawn_local(async move {
        let answer: Result<(), IpcError> =
            ipc::call(command::REVEAL_PATH, &RevealPathArgs { path }).await;
        // The page may be gone by the time Explorer answers
        failed.try_set(answer.is_err());
    });
}

#[component]
pub fn DisksPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let overview = LocalResource::new(|| async {
        ipc::call::<DisksOverview, _>(command::DISKS_OVERVIEW, &()).await
    });
    let status = watch_scan(overview);
    let is_running = move || is_running(status);
    let is_starting = RwSignal::new(false);
    let scan_failed = RwSignal::new(false);
    let reveal_failed = RwSignal::new(false);
    let mode = RwSignal::new(TreemapMode::Category);
    let selected = RwSignal::new(BTreeSet::<String>::new());
    let stage = RwSignal::new(Stage::Idle);
    let candidates = Signal::derive(move || candidates_of(overview));

    let start_scan = move || {
        is_starting.set(true);
        spawn_local(async move {
            let answer: Result<bool, IpcError> = ipc::call(command::SCAN_START, &()).await;
            // The page may be gone by the time the core answers: then there is nothing to update
            if scan_failed.try_set(answer.is_err()).is_none() {
                is_starting.set(false);
                status.refetch();
            }
        });
    };
    let reveal = move |path: String| reveal_path(path, reveal_failed);
    let view_model = move || {
        let answer = overview.get();
        let status = status.get().and_then(Result::ok);
        disks_view(answer.as_ref(), status.as_ref(), SYSTEM_DRIVE, locale())
    };

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().disks.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().disks.title}</h1>
                    <div class="scan">
                        <button
                            type="button"
                            disabled=move || is_running() || is_starting.get()
                            aria-busy=move || (is_running() || is_starting.get()).to_string()
                            on:click=move |_| start_scan()
                        >
                            {move || locale().texts().disks.scan_start}
                        </button>
                        {move || match view_model() {
                            DisksView::Ready { scan_label, .. } => {
                                Some(view! { <p class="muted" role="status">{scan_label}</p> })
                            }
                            _ => None,
                        }}
                    </div>
                </div>
            </header>
            {move || scan_failed.get().then(|| view! { <p role="alert">{locale().texts().disks.scan_failed}</p> })}

            {move || match view_model() {
                DisksView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                DisksView::Error(message) => {
                    view! { <Failure message retry=locale().texts().common.retry on_retry=move || overview.refetch() /> }
                        .into_any()
                }
                DisksView::Ready { kpis, volumes, drives, has_scan, recommendations, .. } => {
                    view! {
                        <Stats kpis />

                        <Hardware volumes drives locale=locale() />

                        {move || {
                            let answer = overview.get();
                            let scan = answer.as_ref().and_then(|answer| answer.as_ref().ok()).and_then(|answer| answer.scan.as_ref())?;
                            let map = treemap(&scan.tiles, &scan.candidates, mode.get(), locale());
                            let description = match view_model() {
                                DisksView::Ready { scan_label, .. } => scan_label,
                                _ => String::new(),
                            };
                            Some(view! { <TreemapSection map_tiles=map.tiles legend=map.legend description mode locale=locale() /> })
                        }}

                        {has_scan
                            .then(|| {
                                view! {
                                    <Recommendations
                                        groups=recommendations
                                        reveal_failed
                                        reveal
                                        selected
                                        stage
                                        candidates
                                        on_cleaned=move || overview.refetch()
                                        locale=Signal::derive(locale)
                                    />
                                }
                            })}
                    }
                        .into_any()
                }
            }}
        </main>
    }
}

#[component]
fn Stats(kpis: Vec<Kpi>) -> impl IntoView {
    view! {
        <ul class="stats">
            {kpis
                .into_iter()
                .map(|kpi| {
                    view! {
                        <li class="stat">
                            <span class="stat-value">{kpi.value}</span>
                            <span>{kpi.label}</span>
                            {kpi.hint.map(|hint| view! { <span class="muted">{hint}</span> })}
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}

#[component]
fn Hardware(volumes: Vec<VolumeRow>, drives: Vec<DriveRow>, locale: Locale) -> impl IntoView {
    let texts = &locale.texts().disks;
    view! {
        <section class="section" aria-labelledby="volumes-title">
            <h2 id="volumes-title">{texts.volumes_title}</h2>
            <ul class="card volumes">
                {volumes
                    .into_iter()
                    .map(|volume| {
                        view! {
                            <li class="volume">
                                <span class="volume-name">{volume.name.clone()}</span>
                                <span
                                    class="meter"
                                    role="meter"
                                    aria-valuemin="0"
                                    aria-valuemax="100"
                                    aria-valuenow=volume.used_percent
                                    aria-label=volume.name
                                    style=("--used", format!("{}%", volume.used_percent))
                                ></span>
                                <span class="mono">{volume.used}</span>
                                <span class="muted">{volume.free}</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>

        <section class="section" aria-labelledby="drives-title">
            <h2 id="drives-title">{texts.drives_title}</h2>
            <div class="table-card">
                <table>
                    <thead>
                        <tr>
                            <th scope="col">{texts.drive_name}</th>
                            <th scope="col">{texts.drive_health}</th>
                            <th scope="col">{texts.drive_temperature}</th>
                            <th scope="col">{texts.drive_wear}</th>
                            <th scope="col">{texts.drive_hours}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {drives
                            .into_iter()
                            .map(|drive| {
                                view! {
                                    <tr>
                                        <td>{drive.name}</td>
                                        <td>
                                            <Tag tone=drive.health_tone>{drive.health}</Tag>
                                        </td>
                                        <td class="mono">{drive.temperature}</td>
                                        <td class="mono">{drive.wear}</td>
                                        <td class="mono">{drive.hours}</td>
                                    </tr>
                                }
                            })
                            .collect_view()}
                    </tbody>
                </table>
            </div>
        </section>

    }
}

#[component]
fn TreemapSection(
    map_tiles: Vec<TreemapTile>,
    legend: Vec<LegendEntry>,
    description: String,
    mode: RwSignal<TreemapMode>,
    locale: Locale,
) -> impl IntoView {
    let texts = &locale.texts().disks;
    view! {
        <section class="section" aria-labelledby="treemap-title">
            <header class="section-header">
                <h2 id="treemap-title">{texts.treemap_title}</h2>
                <div class="toggles" role="group" aria-label=texts.treemap_modes>
                    <button
                        type="button"
                        aria-pressed=move || (mode.get() == TreemapMode::Category).to_string()
                        on:click=move |_| mode.set(TreemapMode::Category)
                    >
                        {texts.by_category}
                    </button>
                    <button
                        type="button"
                        aria-pressed=move || (mode.get() == TreemapMode::Cleanup).to_string()
                        on:click=move |_| mode.set(TreemapMode::Cleanup)
                    >
                        {texts.by_cleanup}
                    </button>
                </div>
            </header>
            <figure class="treemap">
                <ul class="legend" aria-label=texts.legend>
                    {legend
                        .into_iter()
                        .map(|entry| {
                            view! {
                                <li>
                                    <svg class="swatch" viewBox="0 0 12 12" aria-hidden="true">
                                        <rect width="12" height="12" rx="2" fill=pattern_of(entry.fill)></rect>
                                    </svg>
                                    {entry.label}
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
                <svg
                    class="plot"
                    viewBox=format!("0 0 {TREEMAP_WIDTH} {TREEMAP_HEIGHT}")
                    role="img"
                    aria-label=texts.treemap_title
                >
                    <desc>{description}</desc>
                    <TileTextures />
                    {map_tiles
                        .into_iter()
                        .map(|tile| {
                            let rect = tile.rect;
                            view! {
                                <g>
                                    <title>{tile.title}</title>
                                    <rect
                                        x=rect.x + 1.0
                                        y=rect.y + 1.0
                                        width=(rect.width - 2.0).max(0.0)
                                        height=(rect.height - 2.0).max(0.0)
                                        rx="3"
                                        fill=pattern_of(tile.fill)
                                        class="tile"
                                    ></rect>
                                    {tile
                                        .label
                                        .map(|label| {
                                            view! {
                                                <rect
                                                    x=rect.x + 6.0
                                                    y=rect.y + 6.0
                                                    width=(rect.width - 12.0).min(220.0)
                                                    height="34"
                                                    rx="3"
                                                    class="tile-plate"
                                                ></rect>
                                                <text x=rect.x + 12.0 y=rect.y + 20.0 class="tile-label">
                                                    {label}
                                                </text>
                                                <text x=rect.x + 12.0 y=rect.y + 35.0 class="tile-size">
                                                    {tile.size.clone()}
                                                </text>
                                            }
                                        })}
                                </g>
                            }
                        })
                        .collect_view()}
                </svg>
            </figure>
        </section>
    }
}

#[derive(Debug, Clone, Copy)]
enum Texture {
    Solid,
    Diagonal,
    Antidiagonal,
    Dots,
    Horizontal,
    Vertical,
    Cross,
}

/// Colour and texture together: a category never reads by colour alone (the brief, §1 p.4).
const LOOKS: [(&str, &str, Texture); 11] = [
    ("documentsMedia", "fill-documents", Texture::Solid),
    ("devTools", "fill-dev-tools", Texture::Diagonal),
    ("codeProjects", "fill-code", Texture::Dots),
    ("sdks", "fill-sdks", Texture::Antidiagonal),
    ("aiModels", "fill-ai", Texture::Cross),
    ("packageCaches", "fill-caches", Texture::Horizontal),
    ("system", "fill-system", Texture::Vertical),
    ("other", "fill-other", Texture::Solid),
    ("safe", "fill-code", Texture::Dots),
    ("review", "fill-caches", Texture::Horizontal),
    ("none", "fill-other", Texture::Solid),
];

fn id_of(fill: TileFill) -> &'static str {
    match fill {
        TileFill::Category(category) => match category {
            Category::DocumentsMedia => "documentsMedia",
            Category::DevTools => "devTools",
            Category::CodeProjects => "codeProjects",
            Category::Sdks => "sdks",
            Category::AiModels => "aiModels",
            Category::PackageCaches => "packageCaches",
            Category::System => "system",
            Category::Other => "other",
        },
        TileFill::Safe => "safe",
        TileFill::Review => "review",
        TileFill::Nothing => "none",
    }
}

fn pattern_of(fill: TileFill) -> String {
    format!("url(#vitals-tile-{})", id_of(fill))
}

#[component]
fn TileTextures() -> impl IntoView {
    view! {
        <defs>
            {LOOKS
                .iter()
                .map(|(id, class, texture)| {
                    let transform = match texture {
                        Texture::Diagonal => Some("rotate(45)"),
                        Texture::Antidiagonal => Some("rotate(-45)"),
                        _ => None,
                    };
                    let mark = match texture {
                        Texture::Solid => ().into_any(),
                        Texture::Dots => view! { <circle cx="4" cy="4" r="1.3" class="tile-mark"></circle> }.into_any(),
                        Texture::Horizontal => view! { <rect width="8" height="2" class="tile-mark"></rect> }.into_any(),
                        Texture::Vertical => view! { <rect width="2" height="8" class="tile-mark"></rect> }.into_any(),
                        Texture::Diagonal | Texture::Antidiagonal => {
                            view! { <rect width="2.5" height="8" class="tile-mark"></rect> }.into_any()
                        }
                        Texture::Cross => {
                            view! {
                                <rect width="2" height="8" class="tile-mark"></rect>
                                <rect width="8" height="2" class="tile-mark"></rect>
                            }
                                .into_any()
                        }
                    };
                    view! {
                        <pattern
                            id=format!("vitals-tile-{id}")
                            width="8"
                            height="8"
                            patternUnits="userSpaceOnUse"
                            patternTransform=transform
                        >
                            <rect width="8" height="8" class=*class></rect>
                            {mark}
                        </pattern>
                    }
                })
                .collect_view()}
        </defs>
    }
}

#[component]
fn Recommendations(
    groups: Vec<RecommendationGroup>,
    reveal_failed: RwSignal<bool>,
    reveal: impl Fn(String) + Copy + 'static,
    selected: RwSignal<BTreeSet<String>>,
    stage: RwSignal<Stage>,
    candidates: Signal<Vec<Candidate>>,
    on_cleaned: impl Fn() + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get_untracked().texts().disks;
    let toggle = move |id: &str| {
        selected.update(|selected| {
            if !selected.remove(id) {
                selected.insert(id.to_owned());
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="recommendations-title">
            <h2 id="recommendations-title">{texts.recommendations_title}</h2>
            <CleanupBar candidates selected stage on_cleaned locale />
            {move || reveal_failed.get().then(|| view! { <p role="alert" class="muted">{texts.reveal_failed}</p> })}
            <div class="groups">
                {groups
                    .into_iter()
                    .map(|group| {
                        let tone = match group.group {
                            vitals_ipc::Group::Safe => Tone::Ok,
                            vitals_ipc::Group::Review => Tone::Warn,
                        };
                        let is_empty = group.rows.is_empty();
                        view! {
                            <section
                                class="card group"
                                data-group=if tone == Tone::Ok { "safe" } else { "review" }
                                aria-label=group.title
                            >
                                <header class="section-header">
                                    <h3>{group.title}</h3>
                                    <Tag tone>{group.total}</Tag>
                                </header>
                                <p class="muted">{group.hint}</p>
                                {is_empty.then(|| view! { <p class="muted">{texts.nothing}</p> })}
                                <ul class="rows">
                                    {group
                                        .rows
                                        .into_iter()
                                        .map(|row| {
                                            let id = row.id.clone();
                                            let checked_id = row.id.clone();
                                            view! {
                                                <li class="row">
                                                    <div class="row-main">
                                                        {if row.is_selectable {
                                                            view! {
                                                                <label class="row-check">
                                                                    <input
                                                                        type="checkbox"
                                                                        prop:checked=move || selected.with(|selected| selected.contains(&checked_id))
                                                                        on:change=move |_| toggle(&id)
                                                                    />
                                                                    <span class="row-title">{row.title}</span>
                                                                </label>
                                                            }
                                                                .into_any()
                                                        } else {
                                                            view! {
                                                                <span class="row-title">{row.title} " " <Tag tone=Tone::Accent>{texts.manual}</Tag></span>
                                                            }
                                                                .into_any()
                                                        }}
                                                        <span class="mono nowrap">{row.size}</span>
                                                    </div>
                                                    <span class="muted">{row.hint}</span>
                                                    {row.path.clone().map(|path| view! { <span class="mono muted wrap">{path}</span> })}
                                                    {(!row.notes.is_empty())
                                                        .then(|| view! { <span class="muted">{row.notes.join(" · ")}</span> })}
                                                    {row
                                                        .path
                                                        .map(|path| {
                                                            view! {
                                                                <button
                                                                    type="button"
                                                                    class="row-action"
                                                                    on:click=move |_| reveal(path.clone())
                                                                >
                                                                    {texts.reveal}
                                                                </button>
                                                            }
                                                        })}
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            </section>
                        }
                    })
                    .collect_view()}
            </div>
        </section>
    }
}
