mod app_paths;
mod tray;

use std::process::ExitCode;

use tauri::{Manager, WindowEvent};

pub const MAIN_WINDOW: &str = "main";

/// Starts the panel: the tray icon, the main window hidden to the tray on close, the data folder in `%APPDATA%\Vitals`.
#[must_use]
pub fn run() -> ExitCode {
    let result = tauri::Builder::default()
        .setup(|app| {
            app_paths::ensure_data_dir()?;
            tray::create(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the panel in the tray; Quit in the tray menu ends it
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == MAIN_WINDOW {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        eprintln!("vitals: could not hide the window: {error}");
                    }
                }
            }
        })
        .run(tauri::generate_context!());

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vitals: {error}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return Ok(());
    };
    window.unminimize()?;
    window.show()?;
    window.set_focus()
}
