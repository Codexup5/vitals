//! Calls into the core through Tauri's `invoke`. A call has a deadline, its answer is read into the type of
//! `vitals-ipc`, and every failure is one `IpcError` the screens show by kind.

use serde::Serialize;
use serde::de::DeserializeOwned;
use vitals_ipc::{CommandErrorBody, CommandErrorKind};

use crate::i18n::Locale;

/// A command that has not answered by then is reported, not waited for.
#[cfg(target_arch = "wasm32")]
const DEADLINE_MS: u32 = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcErrorKind {
    /// The core could not read its database
    Storage,
    /// The core refused the arguments, or the answer did not read as its type
    Invalid,
    Io,
    Timeout,
    /// Not in the Vitals window: a plain browser has no core to call
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcError {
    pub kind: IpcErrorKind,
    pub message: String,
}

impl IpcError {
    fn new(kind: IpcErrorKind, message: String) -> Self {
        Self { kind, message }
    }

    /// What happened and what to do, by the kind of the failure.
    pub fn text(&self, locale: Locale) -> &'static str {
        let texts = &locale.texts().common;
        match self.kind {
            IpcErrorKind::Storage => texts.storage,
            IpcErrorKind::Timeout => texts.timeout,
            IpcErrorKind::Unavailable => texts.unavailable,
            IpcErrorKind::Invalid | IpcErrorKind::Io => texts.unexpected,
        }
    }
}

/// The error a command reported, or the core not answering at all.
fn from_rejection(command: &str, rejection: Result<CommandErrorBody, String>) -> IpcError {
    match rejection {
        Ok(body) => IpcError::new(
            match body.kind {
                CommandErrorKind::Storage => IpcErrorKind::Storage,
                CommandErrorKind::Invalid => IpcErrorKind::Invalid,
                CommandErrorKind::Io => IpcErrorKind::Io,
            },
            format!("{command}: {}", body.message),
        ),
        Err(raw) => IpcError::new(IpcErrorKind::Unavailable, format!("{command}: {raw}")),
    }
}

#[cfg(target_arch = "wasm32")]
mod bridge {
    use wasm_bindgen::prelude::*;

    // `withGlobalTauri` puts the API on the window; outside Tauri (a plain browser) there is none, and the call
    // rejects instead of throwing, so it becomes `Unavailable` like any other refusal
    #[wasm_bindgen(inline_js = "export function invoke(command, args) {
        const core = window.__TAURI__ && window.__TAURI__.core;
        return core ? core.invoke(command, args) : Promise.reject('not in the Vitals window');
    }")]
    extern "C" {
        #[wasm_bindgen(catch)]
        pub async fn invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;
    }
}

/// Calls a command of the core and reads its answer as `T`.
///
/// # Errors
///
/// The kind the command reported, a timeout, an answer of another shape, or no core outside the Vitals window.
#[cfg(target_arch = "wasm32")]
pub async fn call<T: DeserializeOwned, A: Serialize>(
    command: &'static str,
    args: &A,
) -> Result<T, IpcError> {
    use futures::future::{Either, select};

    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    let args = args
        .serialize(&serializer)
        .map_err(|error| IpcError::new(IpcErrorKind::Invalid, format!("{command}: {error}")))?;
    let call = Box::pin(bridge::invoke(command, args));
    let deadline = gloo_timers::future::TimeoutFuture::new(DEADLINE_MS);
    let answer = match select(call, deadline).await {
        Either::Left((answer, _)) => answer,
        Either::Right(((), _)) => {
            return Err(IpcError::new(
                IpcErrorKind::Timeout,
                format!("{command}: no answer in {DEADLINE_MS} ms"),
            ));
        }
    };
    match answer {
        Ok(value) => serde_wasm_bindgen::from_value(value)
            .map_err(|error| IpcError::new(IpcErrorKind::Invalid, format!("{command}: {error}"))),
        Err(rejection) => {
            let body = serde_wasm_bindgen::from_value::<CommandErrorBody>(rejection.clone())
                .map_err(|_| format!("{rejection:?}"));
            Err(from_rejection(command, body))
        }
    }
}

/// Native builds (the tests) have no core to call.
///
/// # Errors
///
/// Always `Unavailable`.
#[cfg(not(target_arch = "wasm32"))]
#[allow(
    clippy::unused_async,
    reason = "the same signature as the window build"
)]
pub async fn call<T: DeserializeOwned, A: Serialize>(
    command: &'static str,
    _args: &A,
) -> Result<T, IpcError> {
    Err(from_rejection(
        command,
        Err("not in the Vitals window".to_owned()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reported_error_keeps_its_kind() {
        let error = from_rejection(
            "health_overview",
            Ok(CommandErrorBody {
                kind: CommandErrorKind::Storage,
                message: "locked".to_owned(),
            }),
        );
        assert_eq!(error.kind, IpcErrorKind::Storage);
        assert_eq!(error.text(Locale::En), Locale::En.texts().common.storage);
    }

    #[test]
    fn no_core_means_unavailable() {
        let error = from_rejection("minidumps", Err("undefined".to_owned()));
        assert_eq!(error.kind, IpcErrorKind::Unavailable);
    }
}
