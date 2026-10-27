//! Commands the window calls through `invoke`: argument checks, a call into the domain, an error the UI shows by kind.
#![allow(
    clippy::needless_pass_by_value,
    reason = "tauri deserializes command arguments and state into owned values"
)]

pub mod disks;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Days, Local, NaiveDate};
use serde::Serialize;
use tauri::State;

use crate::collector::{self, HealthSummary};
use crate::dumps::{self, Dump};
use crate::storage::queries::{DayRow, EventFilter, EventPage};
use crate::storage::{Storage, StorageError};

const PAGE_SIZE: u32 = 50;
const MAX_RANGE_DAYS: u32 = 366;

/// Where the database lives; every command opens its own connection, the collector keeps its own.
#[derive(Debug)]
pub struct AppState {
    pub db_path: PathBuf,
    pub scan: Arc<crate::scan::job::ScanJob>,
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
        use serde::ser::SerializeStruct;
        let kind = match self {
            Self::Storage(_) => "storage",
            Self::Invalid(_) => "invalid",
            Self::Io(_) => "io",
        };
        let mut state = serializer.serialize_struct("CommandError", 2)?;
        state.serialize_field("kind", kind)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthOverview {
    pub summary: HealthSummary,
    pub first_day: Option<String>,
    pub days: Vec<DayRow>,
    pub months: Vec<DayRow>,
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
        .checked_mul(PAGE_SIZE)
        .ok_or_else(|| CommandError::Invalid(format!("page {page}")))?;
    Ok(open(&state)?.events_page(&filter, offset, PAGE_SIZE)?)
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
