//! The runners screen: runners, containers, and the gaming and coding modes. Stopping Docker and WSL stops other
//! work, so the gaming mode asks once more, in place of its button; while a switch runs the screen asks how far it
//! got once a second.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{Mode, ModeStartArgs, ModeStatus, RunnersOverview, command};

use super::widgets::{Eyebrow, Failure, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::runners::{ProjectGroup, RunnerRow, RunnersView, mode_line, runners_view};

const STATUS_POLL: Duration = Duration::from_secs(1);

type Answer<T> = LocalResource<Result<T, IpcError>>;

fn is_switching(status: Answer<ModeStatus>) -> bool {
    status
        .get()
        .and_then(Result::ok)
        .is_some_and(|status| status.switching.is_some())
}

/// Polls the switch once a second while it runs; when it ends, the runners and containers are read again.
fn watch_mode(status: Answer<ModeStatus>, overview: Answer<RunnersOverview>) {
    let was_switching = StoredValue::new(false);
    Effect::new(move |_| {
        let switching = is_switching(status);
        if was_switching.get_value() && !switching {
            overview.refetch();
        }
        was_switching.set_value(switching);
    });
    if let Ok(poll) = set_interval_with_handle(
        move || {
            if was_switching.get_value() {
                status.refetch();
            }
        },
        STATUS_POLL,
    ) {
        on_cleanup(move || poll.clear());
    }
}

#[component]
pub fn RunnersPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let overview = LocalResource::new(|| async {
        ipc::call::<RunnersOverview, _>(command::RUNNERS_OVERVIEW, &()).await
    });
    let status = LocalResource::new(|| async {
        ipc::call::<ModeStatus, _>(command::MODE_STATUS, &()).await
    });
    watch_mode(status, overview);

    let view_model = move || {
        let answer = overview.get();
        let answer = answer
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale())));
        runners_view(answer, locale())
    };

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().runners.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().runners.title}</h1>
                    <div class="scan">
                        <button type="button" on:click=move |_| overview.refetch()>
                            {move || locale().texts().runners.refresh}
                        </button>
                    </div>
                </div>
            </header>
            {move || match view_model() {
                RunnersView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                RunnersView::Error(message) => {
                    view! { <Failure message retry=locale().texts().common.retry on_retry=move || overview.refetch() /> }
                        .into_any()
                }
                RunnersView::Ready(ready) => {
                    view! {
                        <Modes status running=ready.running locale=Signal::derive(locale) />
                        <Runners rows=ready.runners note=ready.github_note locale=locale() />
                        <Docker label=ready.docker_label tone=ready.docker_tone projects=ready.projects locale=locale() />
                    }
                        .into_any()
                }
            }}
        </main>
    }
}

#[component]
fn Modes(status: Answer<ModeStatus>, running: usize, locale: Signal<Locale>) -> impl IntoView {
    let texts = move || &locale.get().texts().runners;
    let is_confirming = RwSignal::new(false);
    let failed = RwSignal::new(false);
    let start = move |mode: Mode| {
        is_confirming.set(false);
        failed.set(false);
        spawn_local(async move {
            let answer: Result<bool, IpcError> =
                ipc::call(command::MODE_START, &ModeStartArgs { mode }).await;
            // The page may be gone by the time the core answers
            if failed.try_set(answer.is_err()).is_none() {
                status.refetch();
            }
        });
    };
    let line = move || {
        status
            .get()
            .and_then(Result::ok)
            .and_then(|status| mode_line(&status, locale.get()))
    };
    view! {
        <section class="section" aria-labelledby="modes-title">
            <h2 id="modes-title">{move || texts().modes_title}</h2>
            <p class="muted">{move || texts().modes_hint}</p>
            <div class="card modes">
                {move || {
                    if is_confirming.get() {
                        view! {
                            <div class="confirm" role="alertdialog" aria-labelledby="gaming-confirm-title">
                                <h3 id="gaming-confirm-title">{texts().gaming_confirm_title}</h3>
                                <p>{(texts().gaming_confirm)(running)}</p>
                                <div class="toggles">
                                    <button type="button" class="danger" on:click=move |_| start(Mode::Gaming)>
                                        {texts().stop}
                                    </button>
                                    <button type="button" on:click=move |_| is_confirming.set(false)>
                                        {texts().cancel}
                                    </button>
                                </div>
                            </div>
                        }
                            .into_any()
                    } else {
                        let busy = is_switching(status);
                        view! {
                            <div class="mode-choices">
                                <div class="mode-choice">
                                    <button type="button" disabled=busy aria-busy=busy.to_string() on:click=move |_| is_confirming.set(true)>
                                        {texts().gaming}
                                    </button>
                                    <span class="muted">{texts().gaming_hint}</span>
                                </div>
                                <div class="mode-choice">
                                    <button type="button" disabled=busy aria-busy=busy.to_string() on:click=move |_| start(Mode::Coding)>
                                        {texts().coding}
                                    </button>
                                    <span class="muted">{texts().coding_hint}</span>
                                </div>
                            </div>
                        }
                            .into_any()
                    }
                }}
                {move || line().map(|(text, tone)| view! { <p role="status" class="mode-line" data-tone=tone.code()>{text}</p> })}
                {move || failed.get().then(|| view! { <p role="alert">{texts().mode_failed}</p> })}
            </div>
        </section>
    }
}

#[component]
fn Runners(rows: Vec<RunnerRow>, note: Option<&'static str>, locale: Locale) -> impl IntoView {
    let texts = &locale.texts().runners;
    let is_empty = rows.is_empty();
    view! {
        <section class="section" aria-labelledby="runners-title">
            <h2 id="runners-title">{texts.runners_title}</h2>
            <p class="muted">{texts.runners_hint}</p>
            {note.map(|note| view! { <p role="alert" class="muted">{note}</p> })}
            {if is_empty {
                view! { <p class="muted">{texts.no_runners}</p> }.into_any()
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
                                                <Tag tone=row.github.1>{row.github.0}</Tag>
                                            </div>
                                            <span class="mono muted wrap">{row.repo}</span>
                                            <div class="row-tags">
                                                {row.local.map(|(word, tone)| view! { <Tag tone>{word}</Tag> })}
                                                {(!row.labels.is_empty()).then(|| view! { <span class="mono muted">{row.labels}</span> })}
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
fn Docker(
    label: String,
    tone: crate::views::disks::Tone,
    projects: Vec<ProjectGroup>,
    locale: Locale,
) -> impl IntoView {
    let texts = &locale.texts().runners;
    let is_empty = projects.is_empty();
    view! {
        <section class="section" aria-labelledby="docker-title">
            <header class="section-header">
                <h2 id="docker-title">{texts.docker_title}</h2>
                <Tag tone>{label}</Tag>
            </header>
            {is_empty.then(|| view! { <p class="muted">{texts.no_containers}</p> })}
            <div class="groups">
                {projects
                    .into_iter()
                    .map(|group| {
                        let label = group.title.clone();
                        view! {
                            <section class="card" aria-label=label>
                                <h3>{group.title}</h3>
                                <ul class="rows">
                                    {group
                                        .rows
                                        .into_iter()
                                        .map(|row| {
                                            view! {
                                                <li class="row">
                                                    <div class="row-main">
                                                        <span class="row-title">{row.name}</span>
                                                        <Tag tone=row.state.1>{row.state.0}</Tag>
                                                    </div>
                                                    <span class="mono muted wrap">{row.image}</span>
                                                    <span class="muted">{row.status}</span>
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
