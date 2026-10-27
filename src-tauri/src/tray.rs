use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Runtime};

const TRAY_ID: &str = "vitals";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// The tray icon: a left click or "Open" shows the window, "Quit" ends the panel.
/// The mini summary in the tooltip arrives with the collector (stage 1); until then the tooltip is the name.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<TrayIcon<R>> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Открыть Vitals / Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Выход / Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Vitals")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu(app, &event))
        .on_tray_icon_event(|tray, event| handle_click(tray, &event));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

fn handle_menu<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    match event.id.as_ref() {
        MENU_OPEN => show(app),
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

fn handle_click<R: Runtime>(tray: &TrayIcon<R>, event: &TrayIconEvent) {
    if let &TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        show(tray.app_handle());
    }
}

fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Err(error) = crate::show_main_window(app) {
        eprintln!("vitals: could not show the window: {error}");
    }
}
