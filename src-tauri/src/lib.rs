mod alerts;
mod app_paths;
mod cleanup;
mod collector;
mod commands;
mod disks;
mod dumps;
mod elevated;
mod events;
mod import;
mod instance;
mod logging;
mod mcp;
mod network;
mod process;
mod runners;
mod scan;
mod sensors;
mod services;
mod startup;
mod storage;
mod system;
mod tray;
mod trust;
mod updates;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::{Local, Utc};
use tauri::{AppHandle, Manager, WindowEvent};

use crate::collector::Collector;
use crate::storage::Storage;

pub const MAIN_WINDOW: &str = "main";

/// The starts that need no panel: an admin action under UAC, Claude's MCP server, the status line. Each runs and ends.
fn run_without_panel(args: &[String]) -> Option<ExitCode> {
    if let Some(code) = elevated::handle(args) {
        return Some(code);
    }
    #[cfg(windows)]
    match args.get(1).map(String::as_str) {
        Some(mcp::FLAG) => return Some(panel_db().map_or(ExitCode::FAILURE, mcp::tools::run)),
        Some(STATUS_FLAG) => return Some(print_status()),
        _ => {}
    }
    None
}

/// Starts the panel: the log, the tray icon, the collector thread, the main window hidden to the tray on close.
#[must_use]
pub fn run() -> ExitCode {
    if let Some(code) = run_without_panel(&std::env::args().collect::<Vec<_>>()) {
        return code;
    }
    // A second start shows the panel that runs and ends here
    #[cfg(windows)]
    let Some(instance) = instance::claim() else {
        return ExitCode::SUCCESS;
    };
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::log_ui_error,
            commands::health_overview,
            commands::health_events,
            commands::minidumps,
            commands::reveal_minidump,
            commands::disks::disks_overview,
            commands::disks::scan_start,
            commands::disks::scan_status,
            commands::disks::reveal_path,
            commands::disks::cleanup_preview,
            commands::disks::cleanup_run,
            commands::updates::updates_status,
            commands::updates::updates_check,
            commands::updates::updates_run,
            commands::updates::updates_tools,
            commands::updates::open_update_tool,
            commands::runners::runners_present,
            commands::runners::runners_overview,
            commands::runners::mode_start,
            commands::runners::mode_status,
            commands::network::network_overview,
            commands::network::network_probe,
            commands::network::devices_overview,
            commands::network::devices_scan,
            commands::network::speed_test_start,
            commands::network::speed_test_status,
            commands::startup::startup_overview,
            commands::startup::startup_set,
            commands::startup::services_overview,
            commands::startup::service_set,
            commands::alerts::alerts_status,
            commands::alerts::alerts_save_bot,
            commands::alerts::alerts_import_legacy,
            commands::alerts::alerts_forget_bot,
            commands::alerts::alerts_test,
            commands::alerts::alerts_set_sending,
            commands::claude::claude_status,
            commands::claude::claude_connect,
            commands::sensors::sensors_live,
            commands::sensors::sensors_history
        ])
        .setup(move |app| {
            #[cfg(windows)]
            {
                let handle = app.handle().clone();
                instance.on_show(move || {
                    if let Err(error) = show_main_window(&handle) {
                        tracing::error!("could not show the window: {error}");
                    }
                });
            }
            let data_dir = app_paths::ensure_data_dir()?;
            logging::init(&data_dir);
            let devices = std::sync::Arc::<network::devices::DevicesJob>::default();
            app.manage(commands::AppState {
                db_path: data_dir.join(storage::DB_FILE),
                data_dir: data_dir.clone(),
                scan: std::sync::Arc::default(),
                plan: std::sync::Mutex::default(),
                updates: std::sync::Arc::default(),
                mode: std::sync::Arc::default(),
                speed: std::sync::Arc::default(),
                devices: std::sync::Arc::clone(&devices),
                sensors: std::sync::Arc::default(),
            });
            let is_russian = system::windows::ui_language_is_russian();
            tray::create(app.handle(), is_russian)?;
            start_network_sampler(data_dir.clone(), devices);
            #[cfg(windows)]
            start_sensor_sampler(data_dir.clone());
            start_collector(app.handle().clone(), data_dir, is_russian);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the panel in the tray; Quit in the tray menu ends it
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == MAIN_WINDOW
            {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    tracing::error!("could not hide the window: {error}");
                }
            }
        })
        .run(tauri::generate_context!());

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("the panel stopped: {error}");
            ExitCode::FAILURE
        }
    }
}

pub const STATUS_FLAG: &str = "--status";
/// The runners of the last check and the last sample of the sensors count only this long: Docker stopped or the
/// panel closed, they are not said
const FRESH_SECONDS: i64 = 15 * 60;

/// The database of the panel, without creating anything.
fn panel_db() -> Option<PathBuf> {
    app_paths::data_dir_from(std::env::var_os("APPDATA"))
        .ok()
        .map(|dir| dir.join(storage::DB_FILE))
        .filter(|path| path.is_file())
}

/// The runners of the last check, when it was recent.
fn fresh_runners(storage: &Storage, now: i64) -> Option<(u32, u32)> {
    let text = storage.setting(alerts::RUNNERS_KEY).ok()??;
    let seen: serde_json::Value = serde_json::from_str(&text).ok()?;
    let at = seen.get("at")?.as_i64()?;
    let count = |key: &str| {
        seen.get(key)?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
    };
    (now - at <= FRESH_SECONDS).then_some((count("online")?, count("total")?))
}

/// The temperature of the card in the last sample of the sensors, when it was recent.
fn fresh_gpu_temp(storage: &Storage, now: i64) -> Option<f64> {
    let point = storage.latest_sensor_sample().ok()??;
    (now - point.time <= FRESH_SECONDS)
        .then_some(point.gpu_temp_c)
        .flatten()
}

/// `vitals.exe --status`: one line for the status line of Claude Code, from the database in milliseconds.
#[cfg(windows)]
fn print_status() -> ExitCode {
    use std::io::Write as _;

    let Some(storage) = panel_db().and_then(|path| Storage::open(&path).ok()) else {
        return ExitCode::FAILURE;
    };
    let Ok(summary) = collector::summary_from(&storage, Local::now().date_naive()) else {
        return ExitCode::FAILURE;
    };
    let now = Utc::now().timestamp();
    let line = tray::status_line(
        &summary,
        fresh_runners(&storage, now),
        fresh_gpu_temp(&storage, now),
        system::windows::ui_language_is_russian(),
    );
    if writeln!(std::io::stdout(), "{line}").is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The old PowerShell collector's history, imported once per day it lacks (stage 8 retires that collector).
fn legacy_health_json() -> Option<PathBuf> {
    if app_paths::e2e_root().is_some() {
        return None;
    }
    let profile = std::env::var_os("USERPROFILE")?;
    let path = Path::new(&profile)
        .join(".olegpc")
        .join("health")
        .join("health.json");
    path.is_file().then_some(path)
}

fn start_collector(app: AppHandle, data_dir: PathBuf, is_russian: bool) {
    let spawned = std::thread::Builder::new()
        .name("collector".to_owned())
        .spawn(move || {
            let storage = match Storage::open(&data_dir.join(storage::DB_FILE)) {
                Ok(storage) => storage,
                Err(error) => {
                    tracing::error!("the database did not open, the collector is off: {error}");
                    return;
                }
            };
            if let Some(path) = legacy_health_json() {
                match import::import_file(&storage, &path) {
                    Ok(report) => tracing::info!(
                        "health.json: {} days imported, {} skipped",
                        report.imported,
                        report.skipped
                    ),
                    Err(error) => tracing::warn!("health.json was not imported: {error}"),
                }
            }
            let mut collector = Collector::new(storage);
            let db_path = data_dir.join(storage::DB_FILE);
            loop {
                match collector.run_once(Utc::now()) {
                    Ok(summary) => {
                        tray::show_summary(&app, &summary, is_russian);
                        #[cfg(windows)]
                        alerts::check(&db_path, &data_dir, is_russian, Utc::now().timestamp());
                    }
                    Err(error) => {
                        tracing::error!("the collector run failed: {error}");
                        if let Ok(summary) = collector.summary(Local::now().date_naive()) {
                            tray::show_summary(&app, &summary, is_russian);
                        }
                    }
                }
                std::thread::sleep(collector::INTERVAL);
            }
        });
    if let Err(error) = spawned {
        tracing::error!("the collector thread did not start: {error}");
    }
}

/// Measures the way out every five minutes, as the collector counts events: the network chart needs no open window.
/// Every hour it also looks for the devices of the network, so a new one shows up without a click.
fn start_network_sampler(data_dir: PathBuf, devices: std::sync::Arc<network::devices::DevicesJob>) {
    /// Twelve rounds of five minutes
    const SCAN_EVERY: u32 = 12;
    let spawned = std::thread::Builder::new()
        .name("network".to_owned())
        .spawn(move || {
            let storage = match Storage::open(&data_dir.join(storage::DB_FILE)) {
                Ok(storage) => storage,
                Err(error) => {
                    tracing::error!(
                        "the database did not open, the network sampler is off: {error}"
                    );
                    return;
                }
            };
            let db_path = data_dir.join(storage::DB_FILE);
            let mut round = 0_u32;
            loop {
                if round.is_multiple_of(SCAN_EVERY) {
                    devices.run_once(|| network::devices::scan_and_store(&db_path));
                }
                round = round.wrapping_add(1);
                let now = Utc::now().timestamp();
                #[cfg(windows)]
                if let Some(sample) = network::sample(now)
                    && let Err(error) = storage.put_net_sample(&sample)
                {
                    tracing::warn!("the network sample was not saved: {error}");
                }
                if let Err(error) =
                    storage.prune_net_samples(now - network::KEEP_DAYS * 24 * 60 * 60)
                {
                    tracing::warn!("old network samples were not pruned: {error}");
                }
                std::thread::sleep(collector::INTERVAL);
            }
        });
    if let Err(error) = spawned {
        tracing::error!("the network sampler did not start: {error}");
    }
}

/// Samples the sensors every five minutes for their history, as the collector counts events: the processor's
/// counters stay open on this thread, so a sample's load is the mean of the five minutes before it.
#[cfg(windows)]
fn start_sensor_sampler(data_dir: PathBuf) {
    let spawned = std::thread::Builder::new()
        .name("sensors".to_owned())
        .spawn(move || {
            let storage = match Storage::open(&data_dir.join(storage::DB_FILE)) {
                Ok(storage) => storage,
                Err(error) => {
                    tracing::error!(
                        "the database did not open, the sensors sampler is off: {error}"
                    );
                    return;
                }
            };
            let reader = sensors::windows::Reader::open();
            std::thread::sleep(sensors::FIRST_INTERVAL);
            loop {
                let now = Utc::now().timestamp();
                let point = sensors::point_of(&reader.read(now));
                if let Err(error) = storage.put_sensor_sample(&point) {
                    tracing::warn!("the sensors sample was not saved: {error}");
                }
                if let Err(error) =
                    storage.prune_sensor_samples(now - sensors::KEEP_DAYS * 24 * 60 * 60)
                {
                    tracing::warn!("old sensors samples were not pruned: {error}");
                }
                std::thread::sleep(collector::INTERVAL);
            }
        });
    if let Err(error) = spawned {
        tracing::error!("the sensors sampler did not start: {error}");
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
