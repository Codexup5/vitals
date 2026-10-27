//! Commands the window calls through `invoke`; each one is thin and has its permission in `capabilities/`.

/// An unexpected error of the interface, written to the panel's log: it never leaves the PC.
#[tauri::command]
pub fn log_ui_error(message: &str, context: &str) {
    tracing::error!(target: "ui", "{message} {context}");
}
