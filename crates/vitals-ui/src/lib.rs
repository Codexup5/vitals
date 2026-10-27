//! The Vitals window: Leptos compiled to WebAssembly, inside the Tauri window of the core.
//!
//! `views` computes what a screen shows from the answers of the core and is tested natively; `ui` only draws it.

#![allow(
    clippy::must_use_candidate,
    reason = "the window is the only caller of this crate; a dropped result would be its own bug, not an API misuse"
)]

pub mod format;
pub mod i18n;
pub mod ipc;
pub mod modules;
pub mod preferences;
pub mod texts;
pub mod ui;
pub mod views;
