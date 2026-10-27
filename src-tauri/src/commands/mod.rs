//! Commands the window calls through `invoke`: argument checks, a call into the domain, an error the UI shows by kind.
#![allow(
    clippy::needless_pass_by_value,
    reason = "tauri deserializes command arguments and state into owned values"
)]

pub mod disks;
pub mod network;
pub mod runners;
pub mod startup;
pub mod updates;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Days, Local, NaiveDate};
use serde::Serialize;
use tauri::State;

use vitals_ipc::{
    CommandErrorBody, CommandErrorKind, EVENT_PAGE_SIZE, EventPage, HealthOverview, MAX_RANGE_DAYS,
};

use crate::collector;
use crate::dumps::{self, Dump};
use crate::storage::queries::EventFilter;
use crate::storage::{Storage, StorageError};

/// Where the database lives; every command opens its own connection, the collector keeps its own.
#[derive(Debug)]
pub struct AppState {
    pub db_path: PathBuf,
    /// `%APPDATA%\Vitals`: the cleanup log and the files of an elevated action
    pub data_dir: PathBuf,
    pub scan: Arc<crate::scan::job::ScanJob>,
    /// The last cleanup preview: a run executes this one, by its id, or nothing
    pub plan: std::sync::Mutex<Option<crate::cleanup::plan::Plan>>,
    pub updates: Arc<crate::updates::job::UpdatesJob>,
    pub mode: Arc<crate::runners::mode::ModeJob>,
    pub speed: Arc<crate::network::speed::SpeedJob>,
    /// The devices scan: the button and the hourly scan of the sampler share it
    pub devices: Arc<crate::network::devices::DevicesJob>,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("the database could not be read: {0}")]
    Storage(#[from] StorageError),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = match self {
            Self::Storage(_) => CommandErrorKind::Storage,
            Self::Invalid(_) => CommandErrorKind::Invalid,
            Self::Io(_) => CommandErrorKind::Io,
        };
        CommandErrorBody {
            kind,
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub(crate) fn open(state: &AppState) -> Result<Storage, CommandError> {
    Ok(Storage::open(&state.db_path)?)
}

fn parse_day(text: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| CommandError::Invalid(format!("not a date: {text}")))
}

/// An unexpected error of the interface, written to the panel's log: it never leaves the PC.
#[tauri::command]
pub fn log_ui_error(message: &str, context: &str) {
    tracing::error!(target: "ui", "{message} {context}");
}

/// The last `days` days (today included) and every month of the history.
#[tauri::command]
pub fn health_overview(
    state: State<'_, AppState>,
    days: u32,
) -> Result<HealthOverview, CommandError> {
    if !(1..=MAX_RANGE_DAYS).contains(&days) {
        return Err(CommandError::Invalid(format!("range of {days} days")));
    }
    let storage = open(&state)?;
    let today = Local::now().date_naive();
    let from = today
        .checked_sub_days(Days::new(u64::from(days - 1)))
        .unwrap_or(today);
    Ok(HealthOverview {
        summary: collector::summary_from(&storage, today)?,
        first_day: storage
            .first_day()?
            .map(|day| day.format("%Y-%m-%d").to_string()),
        days: storage.days_between(from, today)?,
        months: storage.months()?,
    })
}

#[tauri::command]
pub fn health_events(
    state: State<'_, AppState>,
    kinds: Vec<String>,
    from: Option<String>,
    to: Option<String>,
    page: u32,
) -> Result<EventPage, CommandError> {
    let filter = EventFilter {
        kinds,
        from: from.as_deref().map(parse_day).transpose()?,
        to: to.as_deref().map(parse_day).transpose()?,
    };
    let offset = page
        .checked_mul(EVENT_PAGE_SIZE)
        .ok_or_else(|| CommandError::Invalid(format!("page {page}")))?;
    Ok(open(&state)?.events_page(&filter, offset, EVENT_PAGE_SIZE)?)
}

#[tauri::command]
pub fn minidumps(state: State<'_, AppState>) -> Result<Vec<Dump>, CommandError> {
    let files = dumps::list_files(&dumps::minidump_dir())?;
    Ok(dumps::match_codes(files, &open(&state)?.bsod_times()?))
}

/// Opens Explorer with the dump selected; the name must be a bare `.dmp` file of the dump folder.
#[tauri::command]
pub fn reveal_minidump(name: &str) -> Result<(), CommandError> {
    if !dumps::is_plain_dump_name(name) {
        return Err(CommandError::Invalid(format!("not a dump name: {name}")));
    }
    let path = dumps::minidump_dir().join(name);
    if !path.is_file() {
        return Err(CommandError::Invalid(format!("no such dump: {name}")));
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()?;
    Ok(())
}
