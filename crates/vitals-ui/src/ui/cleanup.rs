//! The cleanup bar under the recommendations: what is checked, then the confirmation in place of the button (the
//! deletion is irreversible), then what the run did.

use std::collections::BTreeSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{
    Candidate, CleanupPreview, CleanupPreviewArgs, CleanupReport, CleanupRunArgs, command,
};

use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::views::cleanup::{PreviewView, preview_view, report_lines, safe_ids, selection};

/// Where the cleanup is; it lives on the page, so a refresh of the screen keeps the confirmation and the report
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Previewing,
    Confirm(PreviewView),
    Running,
    Done(Vec<String>),
    Failed(&'static str),
}

#[component]
pub fn CleanupBar(
    candidates: Signal<Vec<Candidate>>,
    selected: RwSignal<BTreeSet<String>>,
    stage: RwSignal<Stage>,
    on_cleaned: impl Fn() + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = move || &locale.get().texts().disks;

    let preview = move || {
        let ids: Vec<String> = selected.get_untracked().into_iter().collect();
        stage.set(Stage::Previewing);
        spawn_local(async move {
            let answer: Result<CleanupPreview, IpcError> =
                ipc::call(command::CLEANUP_PREVIEW, &CleanupPreviewArgs { ids }).await;
            // The page may be gone by the time the core answers: a disposed signal takes nothing
            let Some(locale) = locale.try_get_untracked() else {
                return;
            };
            let texts = &locale.texts().disks;
            stage.try_set(match answer {
                Ok(preview) => Stage::Confirm(preview_view(&preview, locale)),
                Err(_) => Stage::Failed(texts.preview_failed),
            });
        });
    };
    let run = move |plan_id: u64| {
        stage.set(Stage::Running);
        spawn_local(async move {
            let answer: Result<CleanupReport, IpcError> =
                ipc::call(command::CLEANUP_RUN, &CleanupRunArgs { plan_id }).await;
            let Some(locale) = locale.try_get_untracked() else {
                return;
            };
            match answer {
                Ok(report) => {
                    stage.try_set(Stage::Done(report_lines(&report, locale)));
                    selected.try_set(BTreeSet::new());
                }
                Err(_) => {
                    stage.try_set(Stage::Failed(locale.texts().disks.run_failed));
                }
            }
            on_cleaned();
        });
    };

    view! {
        <div class="card cleanup">
            <div class="toggles">
                <button type="button" on:click=move |_| selected.set(safe_ids(&candidates.get()))>
                    {move || texts().select_safe}
                </button>
                <button
                    type="button"
                    disabled=move || selected.with(BTreeSet::is_empty)
                    on:click=move |_| selected.set(BTreeSet::new())
                >
                    {move || texts().clear_selection}
                </button>
            </div>
            <p class="mono" role="status">
                {move || selection(&candidates.get(), &selected.get(), locale.get())}
            </p>
            {move || match stage.get() {
                Stage::Idle | Stage::Done(_) | Stage::Failed(_) => {
                    view! {
                        <button
                            type="button"
                            class="danger"
                            disabled=move || selected.with(BTreeSet::is_empty)
                            on:click=move |_| preview()
                        >
                            {texts().clean}
                        </button>
                    }
                        .into_any()
                }
                Stage::Previewing | Stage::Running => {
                    view! {
                        <button type="button" disabled=true aria-busy="true">
                            {texts().cleaning}
                        </button>
                    }
                        .into_any()
                }
                Stage::Confirm(view_model) => {
                    view! { <ConfirmDelete view_model run cancel=move || stage.set(Stage::Idle) locale /> }.into_any()
                }
            }}
            {move || match stage.get() {
                Stage::Done(lines) => Some(
                    view! {
                        <div role="status" class="report">
                            {lines.into_iter().map(|line| view! { <p>{line}</p> }).collect_view()}
                        </div>
                    }
                        .into_any(),
                ),
                Stage::Failed(message) => Some(view! { <p role="alert">{message}</p> }.into_any()),
                _ => None,
            }}
        </div>
    }
}

/// The question in place of the button: the deletion cannot be undone, so it is asked once more, here.
#[component]
fn ConfirmDelete(
    view_model: PreviewView,
    run: impl Fn(u64) + Copy + Send + Sync + 'static,
    cancel: impl Fn() + Copy + Send + Sync + 'static,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = &locale.get_untracked().texts().disks;
    let plan_id = view_model.plan_id;
    view! {
        <div class="confirm" role="alertdialog" aria-labelledby="cleanup-confirm-title">
            <h3 id="cleanup-confirm-title">{texts.preview_title}</h3>
            {view_model.lines.into_iter().map(|line| view! { <p>{line}</p> }).collect_view()}
            <div class="toggles">
                <button type="button" class="danger" disabled=view_model.is_empty on:click=move |_| run(plan_id)>
                    {texts.confirm}
                </button>
                <button type="button" on:click=move |_| cancel()>
                    {texts.cancel}
                </button>
            </div>
        </div>
    }
}
