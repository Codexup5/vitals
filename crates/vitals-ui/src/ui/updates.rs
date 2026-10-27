//! The updates screen: the winget packages with checkboxes and "Update selected", the result of the last run, and
//! the tools the panel opens. The check and the run go on in the core; while the job works the screen asks how far
//! it got once a second.

use std::collections::BTreeSet;
use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{
    OpenUpdateToolArgs, UpdateTool, UpdateTools, UpdatesPhase, UpdatesRunArgs, UpdatesStatus,
    command,
};

use super::widgets::{Eyebrow, Failure, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::disks::Tone;
use crate::views::updates::{
    PackageRow, ResultLine, UpdatesView, firmware_line, selectable_ids, updates_view,
};

const STATUS_POLL: Duration = Duration::from_secs(1);

type Answer<T> = LocalResource<Result<T, IpcError>>;

fn status_of(status: Answer<UpdatesStatus>) -> Option<UpdatesStatus> {
    status.get().and_then(Result::ok)
}

/// Polls the status once a second while the job works; when it turns idle, checked ids no longer listed drop out.
fn watch_status(status: Answer<UpdatesStatus>, selected: RwSignal<BTreeSet<String>>) {
    let was_busy = StoredValue::new(false);
    Effect::new(move |_| {
        let Some(current) = status_of(status) else {
            return;
        };
        let is_busy = current.phase != UpdatesPhase::Idle;
        if was_busy.get_value() && !is_busy {
            let listed = selectable_ids(&current.packages);
            selected.update(|selected| selected.retain(|id| listed.contains(id)));
        }
        was_busy.set_value(is_busy);
    });
    if let Ok(poll) = set_interval_with_handle(
        move || {
            if was_busy.get_value() {
                status.refetch();
            }
        },
        STATUS_POLL,
    ) {
        on_cleanup(move || poll.clear());
    }
}

/// Asks the core to start a job; a refusal is shown, an accepted job is polled.
fn start(
    command: &'static str,
    ids: Option<Vec<String>>,
    status: Answer<UpdatesStatus>,
    failed: RwSignal<Option<&'static str>>,
    message: &'static str,
) {
    failed.set(None);
    spawn_local(async move {
        let answer: Result<bool, IpcError> = match ids {
            Some(ids) => ipc::call(command, &UpdatesRunArgs { ids }).await,
            None => ipc::call(command, &()).await,
        };
        // The page may be gone by the time the core answers: then there is nothing to update
        if failed.try_set(answer.is_err().then_some(message)).is_none() {
            status.refetch();
        }
    });
}

fn open_tool(tool: UpdateTool, failed: RwSignal<bool>) {
    spawn_local(async move {
        let answer: Result<(), IpcError> =
            ipc::call(command::OPEN_UPDATE_TOOL, &OpenUpdateToolArgs { tool }).await;
        failed.try_set(answer.is_err());
    });
}

#[component]
pub fn UpdatesPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let status = LocalResource::new(|| async {
        ipc::call::<UpdatesStatus, _>(command::UPDATES_STATUS, &()).await
    });
    let selected = RwSignal::new(BTreeSet::<String>::new());
    let failed = RwSignal::new(None::<&'static str>);
    watch_status(status, selected);

    let check = move || {
        let message = locale().texts().updates.check_failed;
        start(command::UPDATES_CHECK, None, status, failed, message);
    };
    // The first visit lists the packages by itself: a check only reads, and it runs only while the screen is open
    let is_first_check_asked = StoredValue::new(false);
    Effect::new(move |_| {
        let Some(current) = status_of(status) else {
            return;
        };
        if !is_first_check_asked.get_value()
            && current.checked_at.is_none()
            && current.phase == UpdatesPhase::Idle
            && !current.no_winget
        {
            is_first_check_asked.set_value(true);
            check();
        }
    });
    let run = move || {
        let ids: Vec<String> = selected.get_untracked().into_iter().collect();
        let message = locale().texts().updates.run_failed;
        start(command::UPDATES_RUN, Some(ids), status, failed, message);
    };

    let view_model = move || {
        let answer = status.get();
        let answer = answer
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale())));
        updates_view(answer, locale())
    };
    let is_busy = move || matches!(view_model(), UpdatesView::Ready { is_busy: true, .. });

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().updates.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().updates.title}</h1>
                    <div class="scan">
                        <button
                            type="button"
                            disabled=is_busy
                            aria-busy=move || is_busy().to_string()
                            on:click=move |_| check()
                        >
                            {move || locale().texts().updates.check}
                        </button>
                        {move || match view_model() {
                            UpdatesView::Ready { status_label, .. } => {
                                Some(view! { <p class="muted" role="status">{status_label}</p> })
                            }
                            _ => None,
                        }}
                    </div>
                </div>
            </header>
            {move || failed.get().map(|message| view! { <p role="alert">{message}</p> })}

            {move || match view_model() {
                UpdatesView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                UpdatesView::Error(message) => {
                    view! { <Failure message retry=locale().texts().common.retry on_retry=move || status.refetch() /> }
                        .into_any()
                }
                UpdatesView::NoWinget => {
                    view! { <p role="alert" class="notice">{locale().texts().updates.no_winget}</p> }.into_any()
                }
                UpdatesView::Ready { is_busy, check_failed, rows, results, .. } => {
                    view! {
                        {check_failed.then(|| view! { <p role="alert">{locale().texts().updates.check_failed}</p> })}
                        <Programs rows is_busy selected status run locale=Signal::derive(locale) />
                        <Results results locale=locale() />
                    }
                        .into_any()
                }
            }}

            <Tools locale=Signal::derive(locale) />
        </main>
    }
}

#[component]
fn Programs(
    rows: Vec<PackageRow>,
    is_busy: bool,
    selected: RwSignal<BTreeSet<String>>,
    status: Answer<UpdatesStatus>,
    run: impl Fn() + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get_untracked().texts().updates;
    let toggle = move |id: &str| {
        selected.update(|selected| {
            if !selected.remove(id) {
                selected.insert(id.to_owned());
            }
        });
    };
    let select_all = move || {
        if let Some(current) = status_of(status) {
            selected.set(selectable_ids(&current.packages));
        }
    };
    let is_empty = rows.is_empty();
    view! {
        <section class="section" aria-labelledby="programs-title">
            <h2 id="programs-title">{texts.programs_title}</h2>
            <p class="muted">{texts.programs_hint}</p>
            {if is_empty {
                view! { <p class="muted">{texts.nothing}</p> }.into_any()
            } else {
                view! {
                    <div class="card updates-bar">
                        <div class="toggles">
                            <button type="button" disabled=is_busy on:click=move |_| select_all()>
                                {texts.select_all}
                            </button>
                            <button
                                type="button"
                                disabled=move || is_busy || selected.with(BTreeSet::is_empty)
                                on:click=move |_| selected.set(BTreeSet::new())
                            >
                                {texts.clear_selection}
                            </button>
                        </div>
                        <div class="toggles">
                            <button
                                type="button"
                                disabled=move || is_busy || selected.with(BTreeSet::is_empty)
                                aria-busy=is_busy.to_string()
                                on:click=move |_| run()
                            >
                                {texts.run}
                            </button>
                            <p class="mono" role="status">
                                {move || {
                                    let count = selected.with(BTreeSet::len);
                                    (count > 0).then(|| (texts.selected)(count))
                                }}
                            </p>
                        </div>
                    </div>
                    <div class="card">
                    <ul class="rows">
                        {rows
                            .into_iter()
                            .map(|row| {
                                let id = row.id.clone();
                                let checked_id = row.id.clone();
                                view! {
                                    <li class="row">
                                        <div class="row-main">
                                            <label class="row-check">
                                                <input
                                                    type="checkbox"
                                                    disabled=is_busy || !row.can_upgrade
                                                    aria-label=(texts.choose)(&row.name)
                                                    prop:checked=move || selected.with(|selected| selected.contains(&checked_id))
                                                    on:change=move |_| toggle(&id)
                                                />
                                                <span class="row-title">{row.name.clone()}</span>
                                            </label>
                                            <span class="mono nowrap">{row.versions}</span>
                                        </div>
                                        <span class="mono muted wrap">{row.id}</span>
                                        <div class="row-tags">
                                            {row.is_current.then(|| view! { <Tag tone=Tone::Accent>{(texts.upgrading)(&row.name)}</Tag> })}
                                            {row.is_open.then(|| view! { <Tag tone=Tone::Warn>{texts.open}</Tag> })}
                                            {(!row.can_upgrade).then(|| view! { <Tag tone=Tone::Accent>{texts.cut}</Tag> })}
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

#[component]
fn Results(results: Vec<ResultLine>, locale: Locale) -> impl IntoView {
    (!results.is_empty()).then(|| {
        view! {
            <section class="section" aria-labelledby="results-title">
                <h2 id="results-title">{locale.texts().updates.results_title}</h2>
                <div class="card">
                <ul class="rows results" role="status">
                    {results
                        .into_iter()
                        .map(|line| view! { <li class="row" data-tone=line.tone.code()>{line.text}</li> })
                        .collect_view()}
                </ul>
                </div>
            </section>
        }
    })
}

#[component]
fn Tools(locale: Signal<Locale>) -> impl IntoView {
    let tools = LocalResource::new(|| async {
        ipc::call::<UpdateTools, _>(command::UPDATES_TOOLS, &()).await
    });
    let open_failed = RwSignal::new(false);
    let texts = move || &locale.get().texts().updates;
    let tool_row = move |tool: UpdateTool, title: &'static str, hint: &'static str| {
        view! {
            <li class="row">
                <div class="row-main">
                    <span class="row-title">{title}</span>
                    <button type="button" class="row-action" on:click=move |_| open_tool(tool, open_failed)>
                        {move || texts().open_tool}
                    </button>
                </div>
                <span class="muted">{hint}</span>
            </li>
        }
    };
    view! {
        <section class="section" aria-labelledby="tools-title">
            <h2 id="tools-title">{move || texts().tools_title}</h2>
            <p class="muted">{move || texts().no_reboot}</p>
            {move || open_failed.get().then(|| view! { <p role="alert">{texts().open_failed}</p> })}
            {move || {
                let texts = texts();
                let answer = tools.get();
                let details = answer.as_ref().map(|answer| answer.as_ref().ok());
                let has_gpu_tool = details.flatten().is_some_and(|tools| tools.has_gpu_tool);
                let firmware = details.flatten().and_then(|tools| firmware_line(tools, locale.get()));
                view! {
                    <div class="card">
                    <ul class="rows">
                        {tool_row(UpdateTool::WindowsUpdate, texts.windows_update, texts.windows_update_hint)}
                        {if has_gpu_tool {
                            tool_row(UpdateTool::GpuDriver, texts.gpu, texts.gpu_hint).into_any()
                        } else {
                            view! {
                                <li class="row">
                                    <span class="row-title">{texts.gpu}</span>
                                    <span class="muted">{texts.gpu_missing}</span>
                                </li>
                            }
                                .into_any()
                        }}
                        {tool_row(UpdateTool::Store, texts.store, texts.store_hint)}
                        <li class="row">
                            <span class="row-title">{texts.firmware}</span>
                            {match (details, firmware) {
                                (Some(None), _) => view! { <span role="alert">{texts.tools_failed}</span> }.into_any(),
                                (_, Some(line)) => view! { <span class="mono">{line}</span> }.into_any(),
                                _ => view! { <span class="muted">{locale.get().texts().common.missing}</span> }.into_any(),
                            }}
                            <span class="muted">{texts.firmware_hint}</span>
                        </li>
                    </ul>
                    </div>
                }
            }}
        </section>
    }
}
