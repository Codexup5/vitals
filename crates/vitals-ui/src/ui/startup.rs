//! The startup screen: the programs Windows starts at sign-in, each with a button to turn it off or on again. The
//! change is reversible, so it needs no second question; one of every user waits for the UAC prompt.

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{ChangeOutcome, StartupOverview, StartupSetArgs, command};

use super::widgets::{Eyebrow, Failure, Stats, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::startup::{RowAction, StartupRow, StartupView, startup_view};

type Answer<T> = LocalResource<Result<T, IpcError>>;

/// What the last change said, when it needs saying
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    Declined,
    Failed,
}

#[component]
pub fn StartupPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let overview = LocalResource::new(|| async {
        ipc::call::<StartupOverview, _>(command::STARTUP_OVERVIEW, &()).await
    });
    let view_model = move || {
        let answer = overview.get();
        let answer = answer
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale())));
        startup_view(answer, locale())
    };

    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().startup.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().startup.title}</h1>
                    <div class="scan">
                        <button type="button" on:click=move |_| overview.refetch()>
                            {move || locale().texts().startup.refresh}
                        </button>
                    </div>
                </div>
            </header>
            {move || match view_model() {
                StartupView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                StartupView::Error(message) => {
                    view! { <Failure message retry=locale().texts().common.retry on_retry=move || overview.refetch() /> }
                        .into_any()
                }
                StartupView::Empty => view! { <p class="muted">{locale().texts().startup.empty}</p> }.into_any(),
                StartupView::Ready(ready) => {
                    view! {
                        <Stats stats=ready.stats />
                        <Programs rows=ready.rows note=ready.boots_note overview locale=Signal::derive(locale) />
                    }
                        .into_any()
                }
            }}
        </main>
    }
}

#[component]
fn Programs(
    rows: Vec<StartupRow>,
    note: String,
    overview: Answer<StartupOverview>,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = move || &locale.get().texts().startup;
    // The row whose change runs now: its button waits, the others stay free
    let busy = RwSignal::new(None::<String>);
    let notice = RwSignal::new(None::<Notice>);
    let change = move |row: &StartupRow, enable: bool| {
        let args = StartupSetArgs {
            source: row.source,
            name: row.name.clone(),
            enable,
        };
        busy.set(Some(row.key.clone()));
        notice.set(None);
        spawn_local(async move {
            let answer: Result<ChangeOutcome, IpcError> =
                ipc::call(command::STARTUP_SET, &args).await;
            let said = match answer {
                Ok(ChangeOutcome::Done) => None,
                Ok(ChangeOutcome::Declined) => Some(Notice::Declined),
                Err(_) => Some(Notice::Failed),
            };
            // The page may be gone by the time the UAC prompt is answered
            if notice.try_set(said).is_none() {
                busy.set(None);
                overview.refetch();
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="startup-title">
            <h2 id="startup-title">{move || texts().list_title}</h2>
            <p class="muted">{move || texts().hint}</p>
            <p class="muted">{note}</p>
            {move || {
                notice
                    .get()
                    .map(|notice| {
                        let text = match notice {
                            Notice::Declined => texts().declined,
                            Notice::Failed => texts().failed,
                        };
                        view! { <p role="alert">{text}</p> }
                    })
            }}
            <div class="card">
                <ul class="rows">
                    {rows
                        .into_iter()
                        .map(|row| {
                            let action = match row.action.clone() {
                                RowAction::Protected(reason) => view! { <span class="muted">{reason}</span> }.into_any(),
                                RowAction::Toggle { enable, label, admin_note } => {
                                    let key = row.key.clone();
                                    let target = row.clone();
                                    let is_busy = move || busy.get().is_some_and(|busy| busy == key);
                                    view! {
                                        <div class="row-tags">
                                            <button
                                                type="button"
                                                disabled=move || busy.get().is_some()
                                                aria-busy=move || is_busy().to_string()
                                                on:click=move |_| change(&target, enable)
                                            >
                                                {label}
                                            </button>
                                            {admin_note.map(|note| view! { <span class="muted">{note}</span> })}
                                        </div>
                                    }
                                        .into_any()
                                }
                            };
                            view! {
                                <li class="row">
                                    <div class="row-main">
                                        <span class="row-title">{row.title}</span>
                                        <Tag tone=row.state.1>{row.state.0}</Tag>
                                    </div>
                                    <span>{row.publisher}</span>
                                    <span class="mono muted wrap">{row.command}</span>
                                    <div class="row-tags">
                                        {row.impact.map(|(word, tone)| view! { <Tag tone>{word}</Tag> })}
                                        <Tag tone=row.signature.1>{row.signature.0}</Tag>
                                        <span class="muted">{row.place}</span>
                                    </div>
                                    {row.impact_detail.map(|detail| view! { <span class="mono muted">{detail}</span> })}
                                    {row.off_since.map(|since| view! { <span class="muted">{since}</span> })}
                                    {action}
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </div>
        </section>
    }
}
