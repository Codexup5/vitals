//! The settings screen: the start at sign-in, the Telegram bot (saved sealed by the core, never shown back), a test
//! message, sending on or off, the records of the recent checks for the week next to the old alerts, and Claude Code.

use leptos::prelude::*;
use leptos::task::spawn_local;
use vitals_ipc::{
    AlertsSaveBotArgs, AlertsSetSendingArgs, AlertsStatus, AutostartSetArgs, AutostartStatus,
    ClaudeStatus, ConnectOutcome, TestOutcome, command,
};

use super::widgets::{Eyebrow, Failure, Tag};
use crate::i18n::Locale;
use crate::ipc::{self, IpcError};
use crate::preferences::PreferencesStore;
use crate::views::disks::Tone;
use crate::views::settings::{
    AlertsReady, AlertsView, alerts_view, autostart_view, claude_view, connect_line, test_line,
};

type Answer<T> = LocalResource<Result<T, IpcError>>;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TestArgs {
    is_russian: bool,
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let store = PreferencesStore::get();
    let locale = move || store.locale();
    let status = LocalResource::new(|| async {
        ipc::call::<AlertsStatus, _>(command::ALERTS_STATUS, &()).await
    });
    let view_model = move || {
        let answer = status.get();
        let answer = answer
            .as_ref()
            .map(|answer| answer.as_ref().map_err(|error| error.text(locale())));
        alerts_view(answer, locale())
    };
    // Kept on the page: reading the status again redraws the block below, and the line of the last action stays
    let said = RwSignal::new(None::<(&'static str, Tone)>);
    let is_busy = RwSignal::new(false);
    view! {
        <main class="page">
            <header class="page-header">
                <div class="page-heading">
                    <Eyebrow>{move || locale().texts().settings.eyebrow}</Eyebrow>
                    <h1>{move || locale().texts().settings.title}</h1>
                </div>
            </header>
            <Autostart locale=Signal::derive(locale) />
            <section class="section" aria-labelledby="alerts-title">
                <h2 id="alerts-title">{move || locale().texts().settings.alerts_title}</h2>
                {move || match view_model() {
                    AlertsView::Loading => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                    AlertsView::Error(message) => {
                        view! { <Failure message retry=locale().texts().common.retry on_retry=move || status.refetch() /> }
                            .into_any()
                    }
                    AlertsView::Ready(ready) => {
                        view! { <Alerts ready status said is_busy locale=Signal::derive(locale) /> }.into_any()
                    }
                }}
            </section>
            <Claude locale=Signal::derive(locale) />
        </main>
    }
}

#[component]
fn Autostart(locale: Signal<Locale>) -> impl IntoView {
    let texts = move || &locale.get().texts().settings;
    let status = LocalResource::new(|| async {
        ipc::call::<AutostartStatus, _>(command::AUTOSTART_STATUS, &()).await
    });
    // The answer of a change replaces the status shown; a failure is said under the block
    let changed = RwSignal::new(None::<AutostartStatus>);
    let has_failed = RwSignal::new(false);
    let is_busy = RwSignal::new(false);
    let change = move |enable: bool| {
        is_busy.set(true);
        has_failed.set(false);
        spawn_local(async move {
            let answer: Result<AutostartStatus, IpcError> =
                ipc::call(command::AUTOSTART_SET, &AutostartSetArgs { enable }).await;
            let is_ok = answer.is_ok();
            if changed.try_set(answer.ok()).is_none() {
                has_failed.set(!is_ok);
                is_busy.set(false);
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="autostart-title">
            <h2 id="autostart-title">{move || texts().autostart_title}</h2>
            <p class="muted">{move || texts().autostart_hint}</p>
            {move || match changed.get().map(Ok).or_else(|| status.get()) {
                None => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                Some(Err(error)) => {
                    view! { <Failure message=error.text(locale.get()) retry=locale.get().texts().common.retry on_retry=move || status.refetch() /> }
                        .into_any()
                }
                Some(Ok(answer)) => {
                    let ready = autostart_view(&answer, locale.get());
                    let enable = ready.enable;
                    view! {
                        <div class="card settings-card">
                            <div class="row-main">
                                <Tag tone=ready.state.1>{ready.state.0}</Tag>
                                <button
                                    type="button"
                                    disabled=move || is_busy.get()
                                    aria-busy=move || is_busy.get().to_string()
                                    on:click=move |_| change(enable)
                                >
                                    {ready.button}
                                </button>
                            </div>
                            {ready.other_copy.map(|line| view! { <p class="muted wrap">{line}</p> })}
                        </div>
                    }
                        .into_any()
                }
            }}
            {move || {
                has_failed
                    .get()
                    .then(|| view! { <p role="alert" class="mode-line" data-tone="danger">{texts().autostart_failed}</p> })
            }}
        </section>
    }
}

#[component]
fn Claude(locale: Signal<Locale>) -> impl IntoView {
    let texts = move || &locale.get().texts().settings;
    let status = LocalResource::new(|| async {
        ipc::call::<ClaudeStatus, _>(command::CLAUDE_STATUS, &()).await
    });
    let said = RwSignal::new(None::<(&'static str, Tone)>);
    let is_busy = RwSignal::new(false);
    let connect = move || {
        is_busy.set(true);
        said.set(None);
        spawn_local(async move {
            let answer: Result<ConnectOutcome, IpcError> =
                ipc::call(command::CLAUDE_CONNECT, &()).await;
            let locale = locale.get_untracked();
            let line = match answer {
                Ok(outcome) => connect_line(outcome, locale),
                Err(_) => (locale.texts().settings.connect_failed, Tone::Danger),
            };
            if said.try_set(Some(line)).is_none() {
                is_busy.set(false);
                status.refetch();
            }
        });
    };
    view! {
        <section class="section" aria-labelledby="claude-title">
            <h2 id="claude-title">{move || texts().claude_title}</h2>
            <p class="muted">{move || texts().claude_hint}</p>
            {move || match status.get() {
                None => view! { <div class="skeleton" aria-busy="true"></div> }.into_any(),
                Some(Err(error)) => {
                    view! { <Failure message=error.text(locale.get()) retry=locale.get().texts().common.retry on_retry=move || status.refetch() /> }
                        .into_any()
                }
                Some(Ok(answer)) => {
                    let ready = claude_view(&answer, locale.get());
                    view! {
                        <div class="card settings-card">
                            <div class="row-main">
                                <Tag tone=ready.state.1>{ready.state.0}</Tag>
                                {ready
                                    .button
                                    .map(|label| {
                                        view! {
                                            <button
                                                type="button"
                                                disabled=move || is_busy.get()
                                                aria-busy=move || is_busy.get().to_string()
                                                on:click=move |_| connect()
                                            >
                                                {label}
                                            </button>
                                        }
                                    })}
                            </div>
                        </div>
                        <h3>{texts().status_title}</h3>
                        <p class="muted">{texts().status_hint}</p>
                        <p class="card mono wrap">{ready.status_command}</p>
                    }
                        .into_any()
                }
            }}
            {move || said.get().map(|(text, tone)| view! { <p role="status" class="mode-line" data-tone=tone.code()>{text}</p> })}
        </section>
    }
}

#[component]
fn Alerts(
    ready: AlertsReady,
    status: Answer<AlertsStatus>,
    said: RwSignal<Option<(&'static str, Tone)>>,
    is_busy: RwSignal<bool>,
    locale: Signal<Locale>,
) -> impl IntoView {
    let texts = move || &locale.get().texts().settings;
    let token = RwSignal::new(String::new());
    let chat = RwSignal::new(String::new());
    // Every action says how it went in one line and reads the status again
    let finish = move |line: (&'static str, Tone)| {
        if said.try_set(Some(line)).is_none() {
            is_busy.set(false);
            status.refetch();
        }
    };
    let save = move || {
        let args = AlertsSaveBotArgs {
            token: token.get_untracked(),
            chat_id: chat.get_untracked(),
        };
        is_busy.set(true);
        spawn_local(async move {
            let answer: Result<(), IpcError> = ipc::call(command::ALERTS_SAVE_BOT, &args).await;
            let texts = &locale.get_untracked().texts().settings;
            if answer.is_ok() {
                token.set(String::new());
                chat.set(String::new());
            }
            finish(match answer {
                Ok(()) => (texts.saved, Tone::Ok),
                Err(_) => (texts.invalid, Tone::Danger),
            });
        });
    };
    let import = move || {
        is_busy.set(true);
        spawn_local(async move {
            let answer: Result<bool, IpcError> =
                ipc::call(command::ALERTS_IMPORT_LEGACY, &()).await;
            let texts = &locale.get_untracked().texts().settings;
            finish(match answer {
                Ok(true) => (texts.imported, Tone::Ok),
                Ok(false) => (texts.import_failed, Tone::Warn),
                Err(_) => (texts.failed, Tone::Danger),
            });
        });
    };
    let forget = move || {
        is_busy.set(true);
        spawn_local(async move {
            let answer: Result<(), IpcError> = ipc::call(command::ALERTS_FORGET_BOT, &()).await;
            let texts = &locale.get_untracked().texts().settings;
            finish(if answer.is_ok() {
                (texts.forgotten, Tone::Ok)
            } else {
                (texts.failed, Tone::Danger)
            });
        });
    };
    let test = move || {
        is_busy.set(true);
        let args = TestArgs {
            is_russian: locale.get_untracked() == Locale::Ru,
        };
        spawn_local(async move {
            let answer: Result<TestOutcome, IpcError> =
                ipc::call(command::ALERTS_TEST, &args).await;
            let locale = locale.get_untracked();
            finish(match answer {
                Ok(outcome) => test_line(outcome, locale),
                Err(_) => (locale.texts().settings.failed, Tone::Danger),
            });
        });
    };
    let is_sending = ready.is_sending;
    let toggle = move || {
        is_busy.set(true);
        let args = AlertsSetSendingArgs {
            is_sending: !is_sending,
        };
        spawn_local(async move {
            let answer: Result<(), IpcError> = ipc::call(command::ALERTS_SET_SENDING, &args).await;
            if answer.is_err() {
                finish((locale.get_untracked().texts().settings.failed, Tone::Danger));
            } else if said.try_set(None).is_none() {
                is_busy.set(false);
                status.refetch();
            }
        });
    };
    let busy = move || is_busy.get();
    let has_bot = ready.has_bot;
    view! {
        <p class="muted">{ready.hint}</p>
        <div class="card settings-card">
            <p>{ready.bot_line}</p>
            {(!has_bot)
                .then(|| {
                    view! {
                        <form class="fields" on:submit=move |event| {
                            event.prevent_default();
                            save();
                        }>
                            <label class="field">
                                <span>{move || texts().token}</span>
                                <input
                                    type="password"
                                    autocomplete="off"
                                    spellcheck="false"
                                    prop:value=move || token.get()
                                    on:input=move |event| token.set(event_target_value(&event))
                                />
                            </label>
                            <label class="field">
                                <span>{move || texts().chat}</span>
                                <input
                                    type="text"
                                    inputmode="numeric"
                                    autocomplete="off"
                                    spellcheck="false"
                                    prop:value=move || chat.get()
                                    on:input=move |event| chat.set(event_target_value(&event))
                                />
                            </label>
                            <div class="toggles">
                                <button type="submit" disabled=busy>{move || texts().save}</button>
                                {ready
                                    .offers_legacy
                                    .then(|| {
                                        view! {
                                            <button type="button" disabled=busy on:click=move |_| import()>
                                                {move || texts().import_legacy}
                                            </button>
                                        }
                                    })}
                            </div>
                        </form>
                    }
                })}
            {has_bot
                .then(|| {
                    view! {
                        <div class="toggles">
                            <button type="button" disabled=busy aria-busy=move || busy().to_string() on:click=move |_| test()>
                                {move || texts().test}
                            </button>
                            <button type="button" disabled=busy on:click=move |_| forget()>
                                {move || texts().forget}
                            </button>
                        </div>
                    }
                })}
            {move || said.get().map(|(text, tone)| view! { <p role="status" class="mode-line" data-tone=tone.code()>{text}</p> })}
        </div>
        <div class="card settings-card">
            <div class="row-main">
                <Tag tone=ready.sending.1>{ready.sending.0}</Tag>
                <button type="button" disabled=move || busy() || (!has_bot && !is_sending) on:click=move |_| toggle()>
                    {ready.sending_button}
                </button>
            </div>
            <p class="muted">{move || texts().week_hint}</p>
        </div>
        <h3>{move || texts().recent_title}</h3>
        <p class="muted">{ready.last_check}</p>
        {if ready.recent.is_empty() {
            view! { <p class="muted">{move || texts().recent_empty}</p> }.into_any()
        } else {
            view! {
                <div class="card">
                    <ul class="rows">
                        {ready
                            .recent
                            .into_iter()
                            .map(|row| {
                                view! {
                                    <li class="row">
                                        <div class="row-main">
                                            <span class="mono">{row.time}</span>
                                            <Tag tone=row.outcome.1>{row.outcome.0}</Tag>
                                        </div>
                                        {row.lines.into_iter().map(|line| view! { <span>{line}</span> }).collect_view()}
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                </div>
            }
                .into_any()
        }}
    }
}
