//! The Optimization section: focus chips, the settings of the chosen focuses with a check each, the preview of a run
//! with its UAC line, the outcome with "Restart Explorer" when it is needed, and the journal with "Revert".

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{
    ApplyOutcome, Focus, JournalRun, OptimizeOverview, OptimizePlan, PreviewArgs, RevertArgs,
    TweakId, command,
};

use super::widgets::{Eyebrow, Failure, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::disks::Tone;
use crate::views::optimize::{
    FocusGroup, HiddenLine, JournalLine, OptimizeView, OutcomeView, PreviewView, ValueLine,
    default_checks, focus_chips, journal_view, maximum_checks, optimize_view, outcome_view,
    preview_view,
};

type Answer<T> = LocalResource<Result<T, IpcError>>;

/// What the last call of a button came to, when it needs saying
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Idle,
    Busy,
    Failed,
}

#[component]
pub fn OptimizePage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = Signal::derive(move || store.locale());
    let overview: Answer<OptimizeOverview> = LocalResource::new(|| async {
        ipc::call::<OptimizeOverview, _>(command::OPTIMIZE_OVERVIEW, &()).await
    });
    let journal: Answer<Vec<JournalRun>> = LocalResource::new(|| async {
        ipc::call::<Vec<JournalRun>, _>(command::OPTIMIZE_JOURNAL, &()).await
    });
    let focuses = RwSignal::new(Vec::<Focus>::new());
    let checked = RwSignal::new(Vec::<TweakId>::new());
    let preview = RwSignal::new(None::<OptimizePlan>);
    let outcome = RwSignal::new(None::<ApplyOutcome>);
    let step = RwSignal::new(Step::Idle);

    // The checks follow the defaults whenever the machine is read again or the focuses change
    Effect::new(move |_| {
        let chosen = focuses.get();
        if let Some(Ok(read)) = overview.get().as_ref() {
            checked.set(default_checks(read, &chosen));
        }
        preview.set(None);
    });

    let view_model = move || {
        let answer = overview.get();
        let answer = answer
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale.get())));
        optimize_view(answer, &focuses.get(), &checked.get(), locale.get())
    };
    let set_checks = move |pick: fn(&OptimizeOverview, &[Focus]) -> Vec<TweakId>| {
        if let Some(Ok(read)) = overview.get_untracked().as_ref() {
            checked.set(pick(read, &focuses.get_untracked()));
            preview.set(None);
        }
    };
    let ask_preview = move || {
        step.set(Step::Busy);
        outcome.set(None);
        let args = PreviewArgs {
            checked: checked.get_untracked(),
            enable_protection: false,
        };
        spawn_local(async move {
            let answer: Result<OptimizePlan, IpcError> =
                ipc::call(command::OPTIMIZE_PREVIEW, &args).await;
            let next = if answer.is_ok() {
                Step::Idle
            } else {
                Step::Failed
            };
            if step.try_set(next).is_none() {
                preview.set(answer.ok());
            }
        });
    };

    view! {
        <main class="page">
            <Heading overview journal locale />
            <p class="muted">{move || locale.get().texts().optimize.hint}</p>
            {move || match view_model() {
                OptimizeView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                OptimizeView::Error(message) => {
                    view! { <Failure message retry=locale.get().texts().common.retry on_retry=move || overview.refetch() /> }
                        .into_any()
                }
                OptimizeView::Ready(ready) => {
                    let has_checks = ready.to_apply > 0;
                    view! {
                        <Focuses machine=ready.machine summary=ready.summary focuses set_checks locale />
                        {ready
                            .groups
                            .into_iter()
                            .map(|group| view! { <Group group checked preview locale /> })
                            .collect_view()}
                        <Hidden lines=ready.hidden locale />
                        <PreviewButton has_checks step ask_preview locale />
                    }
                        .into_any()
                }
            }}
            {move || (step.get() == Step::Failed).then(|| view! { <p role="alert">{locale.get().texts().optimize.failed}</p> })}
            {move || {
                preview
                    .get()
                    .map(|plan| {
                        view! { <Preview view=preview_view(&plan, locale.get()) checked preview outcome step overview journal locale /> }
                    })
            }}
            {move || outcome.get().map(|done| view! { <Outcome view=outcome_view(&done, locale.get()) locale /> })}
            <Journal journal overview locale />
        </main>
    }
}

#[component]
fn Heading(
    overview: Answer<OptimizeOverview>,
    journal: Answer<Vec<JournalRun>>,
    locale: Signal<Locale>,
) -> impl IntoView {
    view! {
        <header class="page-header">
            <div class="page-heading">
                <Eyebrow>{move || locale.get().texts().optimize.eyebrow}</Eyebrow>
                <h1>{move || locale.get().texts().optimize.title}</h1>
                <div class="scan">
                    <button
                        type="button"
                        on:click=move |_| {
                            overview.refetch();
                            journal.refetch();
                        }
                    >
                        {move || locale.get().texts().optimize.refresh}
                    </button>
                </div>
            </div>
        </header>
    }
}

#[component]
fn PreviewButton(
    has_checks: bool,
    step: RwSignal<Step>,
    ask_preview: impl Fn() + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get().texts().optimize;
    view! {
        <div class="optimize-actions">
            <button
                type="button"
                disabled=move || !has_checks || step.get() == Step::Busy
                on:click=move |_| ask_preview()
            >
                {texts.preview}
            </button>
            {(!has_checks).then(|| view! { <span class="muted">{texts.nothing_checked}</span> })}
        </div>
    }
}

#[component]
fn Focuses(
    machine: String,
    summary: String,
    focuses: RwSignal<Vec<Focus>>,
    set_checks: impl Fn(fn(&OptimizeOverview, &[Focus]) -> Vec<TweakId>) + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get().texts().optimize;
    let flip = move |focus: Focus| {
        focuses.update(|chosen| {
            if let Some(at) = chosen.iter().position(|one| *one == focus) {
                chosen.remove(at);
            } else {
                chosen.push(focus);
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="focuses-title">
            <h2 id="focuses-title">{texts.focuses}</h2>
            <p class="muted">{machine}</p>
            <div class="toggles">
                {focus_chips(&focuses.get_untracked(), locale.get())
                    .into_iter()
                    .map(|chip| {
                        view! {
                            <button
                                type="button"
                                aria-pressed=chip.is_on.to_string()
                                on:click=move |_| flip(chip.focus)
                            >
                                {chip.label}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
            <div class="toggles">
                <span class="muted">{summary}</span>
                <button type="button" on:click=move |_| set_checks(default_checks)>
                    {texts.defaults}
                </button>
                <button type="button" on:click=move |_| set_checks(maximum_checks)>
                    {texts.maximum}
                </button>
            </div>
        </section>
    }
}

#[component]
fn Hidden(lines: Vec<HiddenLine>, locale: Signal<Locale>) -> impl IntoView {
    (!lines.is_empty()).then(|| {
        view! {
            <section class="section" aria-labelledby="hidden-title">
                <h2 id="hidden-title">{locale.get().texts().optimize.hidden_title}</h2>
                <ul class="optimize-hidden">
                    {lines
                        .into_iter()
                        .map(|line| {
                            view! {
                                <li>
                                    <span class="row-title">{line.title}</span>
                                    " · "
                                    <span class="muted">{line.reason}</span>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </section>
        }
    })
}

#[component]
fn Values(values: Vec<ValueLine>) -> impl IntoView {
    view! {
        <ul class="optimize-values">
            {values
                .into_iter()
                .map(|value| {
                    view! {
                        <li class="mono">
                            <span class="wrap">{value.name}</span>
                            ": "
                            <span>{value.was}</span>
                            " → "
                            <span>{value.becomes}</span>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}

#[component]
fn Group(
    group: FocusGroup,
    checked: RwSignal<Vec<TweakId>>,
    preview: RwSignal<Option<OptimizePlan>>,
    locale: Signal<Locale>,
) -> impl IntoView {
    let toggle = move |id: TweakId| {
        checked.update(|ids| {
            if let Some(at) = ids.iter().position(|one| *one == id) {
                ids.remove(at);
            } else {
                ids.push(id);
            }
        });
        preview.set(None);
    };
    view! {
        <section class="section">
            <h2>{group.title}</h2>
            <div class="card">
                <ul class="rows">
                    {group
                        .lines
                        .into_iter()
                        .map(|line| {
                            let id = line.id;
                            view! {
                                <li class="row">
                                    <div class="row-main">
                                        <label class="row-check">
                                            <input
                                                type="checkbox"
                                                disabled=line.is_done
                                                prop:checked=move || checked.with(|ids| ids.contains(&id))
                                                on:change=move |_| toggle(id)
                                            />
                                            <span class="row-title">{line.title}</span>
                                        </label>
                                        <Tag tone=line.state.1>{line.state.0}</Tag>
                                    </div>
                                    <span class="muted">{line.hint}</span>
                                    <div class="row-tags">
                                        <span class="muted">{line.effect}</span>
                                        {line.admin.map(|admin| view! { <Tag tone=Tone::Warn>{admin}</Tag> })}
                                    </div>
                                    <details class="optimize-details">
                                        <summary>{move || locale.get().texts().optimize.values}</summary>
                                        <Values values=line.values />
                                    </details>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </div>
        </section>
    }
}

#[component]
fn Preview(
    view: PreviewView,
    checked: RwSignal<Vec<TweakId>>,
    preview: RwSignal<Option<OptimizePlan>>,
    outcome: RwSignal<Option<ApplyOutcome>>,
    step: RwSignal<Step>,
    overview: Answer<OptimizeOverview>,
    journal: Answer<Vec<JournalRun>>,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get().texts().optimize;
    let protection = RwSignal::new(false);
    let apply = move || {
        step.set(Step::Busy);
        let args = PreviewArgs {
            checked: checked.get_untracked(),
            enable_protection: protection.get_untracked(),
        };
        spawn_local(async move {
            let answer: Result<ApplyOutcome, IpcError> =
                ipc::call(command::OPTIMIZE_APPLY, &args).await;
            let next = if answer.is_ok() {
                Step::Idle
            } else {
                Step::Failed
            };
            // The page may be gone by the time the UAC prompt is answered
            if step.try_set(next).is_none() {
                outcome.set(answer.ok());
                preview.set(None);
                overview.refetch();
                journal.refetch();
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="preview-title">
            <h2 id="preview-title">{texts.preview_title}</h2>
            {view
                .groups
                .into_iter()
                .map(|group| {
                    view! {
                        <h3>{group.title}</h3>
                        <div class="card">
                            <ul class="rows">
                                {group
                                    .lines
                                    .into_iter()
                                    .map(|line| {
                                        view! {
                                            <li class="row">
                                                <div class="row-main">
                                                    <span class="row-title">{line.title}</span>
                                                    <span class="muted">{line.effect}</span>
                                                </div>
                                                <Values values=line.values />
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        </div>
                    }
                })
                .collect_view()}
            {view.already.map(|already| view! { <p class="muted">{already}</p> })}
            <p>{view.rights_line}</p>
            {view
                .needs_uac
                .then(|| {
                    view! {
                        <label class="row-check">
                            <input
                                type="checkbox"
                                prop:checked=move || protection.get()
                                on:change=move |_| protection.update(|on| *on = !*on)
                            />
                            <span>{texts.protection}</span>
                        </label>
                    }
                })}
            <div class="optimize-actions">
                <button
                    type="button"
                    disabled=move || view.is_empty || step.get() == Step::Busy
                    aria-busy=move || (step.get() == Step::Busy).to_string()
                    on:click=move |_| apply()
                >
                    {move || if step.get() == Step::Busy { texts.applying.to_owned() } else { view.apply_label.clone() }}
                </button>
                <button type="button" on:click=move |_| preview.set(None)>
                    {texts.cancel}
                </button>
            </div>
        </section>
    }
}

#[component]
fn Outcome(view: OutcomeView, locale: Signal<Locale>) -> impl IntoView {
    let texts = &locale.get().texts().optimize;
    let explorer = RwSignal::new(None::<Result<(), ()>>);
    let is_restarting = RwSignal::new(false);
    let restart = move || {
        is_restarting.set(true);
        spawn_local(async move {
            let answer: Result<(), IpcError> = ipc::call(command::EXPLORER_RESTART, &()).await;
            if is_restarting.try_set(false).is_some() {
                return;
            }
            explorer.set(Some(answer.map_err(|_| ())));
        });
    };
    view! {
        <section class="section" aria-labelledby="outcome-title" role="status">
            <h2 id="outcome-title" class="visually-hidden">{texts.preview_title}</h2>
            <p>
                <Tag tone=view.tone>{view.headline}</Tag>
            </p>
            {(!view.failures.is_empty())
                .then(|| {
                    view! {
                        <ul class="optimize-hidden">
                            {view
                                .failures
                                .into_iter()
                                .map(|(title, message)| {
                                    view! {
                                        <li>
                                            <span class="row-title">{title}</span>
                                            " · "
                                            <span class="mono muted">{message}</span>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    }
                })}
            {view.restore.map(|restore| view! { <p class="muted">{restore}</p> })}
            {view.then.into_iter().map(|line| view! { <p>{line}</p> }).collect_view()}
            {view
                .offers_explorer
                .then(|| {
                    view! {
                        <div class="optimize-actions">
                            <button
                                type="button"
                                disabled=move || is_restarting.get()
                                aria-busy=move || is_restarting.get().to_string()
                                on:click=move |_| restart()
                            >
                                {move || if is_restarting.get() { texts.restarting } else { texts.restart_explorer }}
                            </button>
                            {move || match explorer.get() {
                                Some(Ok(())) => Some(view! { <span class="muted">{texts.restarted}</span> }.into_any()),
                                Some(Err(())) => Some(view! { <span role="alert">{texts.failed}</span> }.into_any()),
                                None => None,
                            }}
                        </div>
                    }
                })}
        </section>
    }
}

#[component]
fn Journal(
    journal: Answer<Vec<JournalRun>>,
    overview: Answer<OptimizeOverview>,
    locale: Signal<Locale>,
) -> impl IntoView {
    let busy = RwSignal::new(None::<i64>);
    let result = RwSignal::new(None::<OutcomeView>);
    let revert = move |run: i64| {
        busy.set(Some(run));
        result.set(None);
        spawn_local(async move {
            let answer: Result<ApplyOutcome, IpcError> =
                ipc::call(command::OPTIMIZE_REVERT, &RevertArgs { run }).await;
            if busy.try_set(None).is_some() {
                return;
            }
            result.set(
                answer
                    .ok()
                    .map(|done| outcome_view(&done, locale.get_untracked())),
            );
            journal.refetch();
            overview.refetch();
        });
    };
    let lines = move || {
        journal.get().as_ref().map(|answer| {
            answer
                .as_ref()
                .map(|runs| journal_view(runs, locale.get()))
                .map_err(|error| error.text(locale.get()))
        })
    };
    view! {
        <section class="section" aria-labelledby="journal-title">
            <h2 id="journal-title">{move || locale.get().texts().optimize.journal_title}</h2>
            <p class="muted">{move || locale.get().texts().optimize.journal_hint}</p>
            {move || result.get().map(|view| view! { <p role="status"><Tag tone=view.tone>{view.headline}</Tag></p> })}
            {move || match lines() {
                None => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                Some(Err(message)) => {
                    view! { <Failure message retry=locale.get().texts().common.retry on_retry=move || journal.refetch() /> }
                        .into_any()
                }
                Some(Ok(lines)) if lines.is_empty() => {
                    view! { <p class="muted">{locale.get().texts().optimize.journal_empty}</p> }.into_any()
                }
                Some(Ok(lines)) => {
                    view! {
                        <div class="card">
                            <ul class="rows">
                                {lines.into_iter().map(|line| view! { <JournalItem line busy revert locale /> }).collect_view()}
                            </ul>
                        </div>
                    }
                        .into_any()
                }
            }}
        </section>
    }
}

#[component]
fn JournalItem(
    line: JournalLine,
    busy: RwSignal<Option<i64>>,
    revert: impl Fn(i64) + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get().texts().optimize;
    let run = line.run;
    let is_busy = move || busy.get() == Some(run);
    view! {
        <li class="row">
            <div class="row-main">
                <span class="row-title">{line.label}</span>
                {line.is_reverted.then(|| view! { <Tag tone=Tone::Accent>{texts.reverted}</Tag> })}
            </div>
            <details class="optimize-details">
                <summary>{texts.values}</summary>
                <ul class="optimize-values">
                    {line
                        .changes
                        .into_iter()
                        .map(|(title, value)| {
                            view! {
                                <li>
                                    <span>{title}</span>
                                    <span class="mono wrap">{format!(" · {}: {} → {}", value.name, value.was, value.becomes)}</span>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </details>
            {(!line.is_reverted)
                .then(|| {
                    view! {
                        <button
                            type="button"
                            class="row-action"
                            disabled=move || busy.get().is_some()
                            aria-busy=move || is_busy().to_string()
                            on:click=move |_| revert(run)
                        >
                            {move || if is_busy() { texts.reverting } else { texts.revert }}
                        </button>
                    }
                })}
        </li>
    }
}
