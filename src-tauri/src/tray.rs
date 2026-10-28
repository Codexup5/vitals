use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use crate::collector::HealthSummary;

const TRAY_ID: &str = "vitals";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// The tray icon: a left click or "Open" shows the window, "Quit" ends the panel; the tooltip is the mini summary.
pub fn create<R: Runtime>(app: &AppHandle<R>, is_russian: bool) -> tauri::Result<TrayIcon<R>> {
    let (open_text, quit_text) = if is_russian {
        ("Открыть Vitals", "Выход")
    } else {
        ("Open Vitals", "Quit")
    };
    let open = MenuItem::with_id(app, MENU_OPEN, open_text, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, quit_text, true, None::<&str>)?;
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

pub fn show_summary<R: Runtime>(app: &AppHandle<R>, summary: &HealthSummary, is_russian: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Err(error) = tray.set_tooltip(Some(tooltip_text(summary, is_russian))) {
        tracing::warn!("could not set the tray tooltip: {error}");
    }
}

fn russian_days(count: i64) -> &'static str {
    match (count % 10, count % 100) {
        (1, 11) | (2..=4, 12..=14) | (0 | 5..=9, _) => "дней",
        (1, _) => "день",
        _ => "дня",
    }
}

/// "Vitals · 14 дней без падений · C: 207.6 ГБ свободно"; a part with no data yet is left out.
/// The line of `vitals.exe --status` for the status line of Claude Code: the tooltip and, when known, the runners.
pub fn status_line(
    summary: &HealthSummary,
    runners: Option<(u32, u32)>,
    is_russian: bool,
) -> String {
    let line = tooltip_text(summary, is_russian);
    let Some((online, total)) = runners else {
        return line;
    };
    let word = if is_russian {
        "раннеры"
    } else {
        "runners"
    };
    format!("{line} · {word} {online}/{total}")
}

pub fn tooltip_text(summary: &HealthSummary, is_russian: bool) -> String {
    let mut parts = vec!["Vitals".to_owned()];
    if let Some(days) = summary.days_without_crash {
        parts.push(if is_russian {
            format!("{days} {} без падений", russian_days(days))
        } else {
            format!(
                "{days} {} without crashes",
                if days == 1 { "day" } else { "days" }
            )
        });
    }
    if let Some(free) = summary.system_drive_free_gb {
        parts.push(if is_russian {
            format!("C: {} ГБ свободно", format!("{free:.1}").replace('.', ","))
        } else {
            format!("C: {free:.1} GB free")
        });
    }
    parts.join(" · ")
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
        tracing::error!("could not show the window: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_plural_of_days() {
        let forms: Vec<&str> = [0, 1, 2, 4, 5, 11, 12, 14, 21, 22, 25, 101, 111]
            .into_iter()
            .map(russian_days)
            .collect();
        assert_eq!(
            forms,
            [
                "дней", "день", "дня", "дня", "дней", "дней", "дней", "дней", "день", "дня",
                "дней", "день", "дней"
            ]
        );
    }

    #[test]
    fn tooltip_in_both_languages_and_without_data() {
        let summary = HealthSummary {
            days_without_crash: Some(14),
            system_drive_free_gb: Some(207.6),
        };
        assert_eq!(
            tooltip_text(&summary, true),
            "Vitals · 14 дней без падений · C: 207,6 ГБ свободно"
        );
        assert_eq!(
            tooltip_text(&summary, false),
            "Vitals · 14 days without crashes · C: 207.6 GB free"
        );
        let empty = HealthSummary {
            days_without_crash: None,
            system_drive_free_gb: None,
        };
        assert_eq!(tooltip_text(&empty, true), "Vitals");
    }
}
