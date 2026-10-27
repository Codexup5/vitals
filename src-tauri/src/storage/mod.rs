//! `%APPDATA%\Vitals\vitals.db`: event rows for a year, daily counts and snapshots for good.

mod network;
pub mod queries;
mod schema;

use std::path::Path;
pub use vitals_ipc::DayCounts;

use chrono::{DateTime, Days, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::events::{EventKind, StoredEvent};

pub const DB_FILE: &str = "vitals.db";
const EVENT_RETENTION_DAYS: u64 = 365;
const TOP_APPS: i64 = 3;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("stored JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Where a day's counts came from: counted from the event log, or taken from the old `health.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaySource {
    Collector,
    Import,
}

impl DaySource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Collector => "collector",
            Self::Import => "import",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskFree {
    pub drive: String,
    pub free_gb: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub disks: Vec<DiskFree>,
    pub ram_free_gb: Option<f64>,
    pub uptime_h: Option<f64>,
    pub stability: Option<f64>,
    pub bios: Option<String>,
    pub ram_mts: Option<u32>,
    pub gpu_driver: Option<String>,
}

#[derive(Debug)]
pub struct Storage {
    conn: Connection,
}

fn day_text(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

fn parse_day(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::from_connection(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, StorageError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, StorageError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Keeps each entry once: the log, its record id and its time identify it even after a log is cleared.
    pub fn insert_events(&mut self, events: &[StoredEvent]) -> Result<usize, StorageError> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0;
        {
            let mut insert = tx.prepare_cached(
                "INSERT OR IGNORE INTO events (log, record_id, time, day, provider, event_id, level, kind, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for event in events {
                inserted += insert.execute(params![
                    event.log.as_str(),
                    i64::try_from(event.record_id).unwrap_or(i64::MAX),
                    event.time.timestamp(),
                    day_text(event.day),
                    event.provider,
                    event.event_id,
                    event.level,
                    event.kind.map(EventKind::as_str),
                    event.detail,
                ])?;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// The counts of one day from its stored rows, the same rules as `health-collect.ps1`.
    pub fn count_day(&self, day: NaiveDate) -> Result<DayCounts, StorageError> {
        let day = day_text(day);
        let count = |kind: EventKind| -> Result<u32, StorageError> {
            Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM events WHERE day = ?1 AND kind = ?2",
                params![day, kind.as_str()],
                |row| row.get(0),
            )?)
        };
        let errors = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE day = ?1 AND log = 'System' AND level IN (1, 2)",
            params![day],
            |row| row.get(0),
        )?;
        let mut top = self.conn.prepare_cached(
            "SELECT detail, COUNT(*) AS n FROM events WHERE day = ?1 AND kind = 'appCrash' AND detail IS NOT NULL
             GROUP BY detail ORDER BY n DESC, detail LIMIT ?2",
        )?;
        let top_apps = top
            .query_map(params![day, TOP_APPS], |row| {
                Ok(format!(
                    "{} ×{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DayCounts {
            crashes: count(EventKind::Crash)?,
            bsod: count(EventKind::Bsod)?,
            gpu_reset: count(EventKind::GpuReset)?,
            nv_container: count(EventKind::NvContainer)?,
            whea: count(EventKind::Whea)?,
            iommu: count(EventKind::Iommu)?,
            app_crashes: count(EventKind::AppCrash)?,
            errors,
            top_apps,
        })
    }

    pub fn put_day(
        &self,
        day: NaiveDate,
        counts: &DayCounts,
        source: DaySource,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO days (day, crashes, bsod, gpu_reset, nv_container, whea, iommu, app_crashes, errors, top_apps, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(day) DO UPDATE SET crashes = excluded.crashes, bsod = excluded.bsod,
               gpu_reset = excluded.gpu_reset, nv_container = excluded.nv_container, whea = excluded.whea,
               iommu = excluded.iommu, app_crashes = excluded.app_crashes, errors = excluded.errors,
               top_apps = excluded.top_apps, source = excluded.source",
            params![
                day_text(day),
                counts.crashes,
                counts.bsod,
                counts.gpu_reset,
                counts.nv_container,
                counts.whea,
                counts.iommu,
                counts.app_crashes,
                counts.errors,
                serde_json::to_string(&counts.top_apps)?,
                source.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn has_day(&self, day: NaiveDate) -> Result<bool, StorageError> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM days WHERE day = ?1",
                params![day_text(day)],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    #[cfg(test)]
    pub fn day(&self, day: NaiveDate) -> Result<Option<DayCounts>, StorageError> {
        let row = self
            .conn
            .query_row(
                "SELECT crashes, bsod, gpu_reset, nv_container, whea, iommu, app_crashes, errors, top_apps
                 FROM days WHERE day = ?1",
                params![day_text(day)],
                |row| {
                    Ok((
                        DayCounts {
                            crashes: row.get(0)?,
                            bsod: row.get(1)?,
                            gpu_reset: row.get(2)?,
                            nv_container: row.get(3)?,
                            whea: row.get(4)?,
                            iommu: row.get(5)?,
                            app_crashes: row.get(6)?,
                            errors: row.get(7)?,
                            top_apps: Vec::new(),
                        },
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(mut counts, top_apps)| {
            counts.top_apps = serde_json::from_str(&top_apps)?;
            Ok(counts)
        })
        .transpose()
    }

    /// The last day the collector counted from the event log: the next run starts there.
    pub fn last_collected_day(&self) -> Result<Option<NaiveDate>, StorageError> {
        let text: Option<String> = self.conn.query_row(
            "SELECT MAX(day) FROM days WHERE source = 'collector'",
            [],
            |row| row.get(0),
        )?;
        Ok(text.as_deref().and_then(parse_day))
    }

    pub fn put_snapshot(
        &self,
        day: NaiveDate,
        taken_at: DateTime<Utc>,
        snapshot: &Snapshot,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO snapshots (day, taken_at, body) VALUES (?1, ?2, ?3)
             ON CONFLICT(day) DO UPDATE SET taken_at = excluded.taken_at, body = excluded.body",
            params![
                day_text(day),
                taken_at.timestamp(),
                serde_json::to_string(snapshot)?
            ],
        )?;
        Ok(())
    }

    pub fn put_snapshot_if_absent(
        &self,
        day: NaiveDate,
        snapshot: &Snapshot,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO snapshots (day, taken_at, body) VALUES (?1, NULL, ?2)",
            params![day_text(day), serde_json::to_string(snapshot)?],
        )?;
        Ok(())
    }

    pub fn latest_snapshot(&self) -> Result<Option<Snapshot>, StorageError> {
        let body: Option<String> = self
            .conn
            .query_row(
                "SELECT body FROM snapshots ORDER BY day DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body.map(|body| serde_json::from_str(&body)).transpose()?)
    }

    /// The last day with a crash or a BSOD, and the first recorded day: "days without crashes" counts from one of them.
    pub fn crash_bounds(&self) -> Result<(Option<NaiveDate>, Option<NaiveDate>), StorageError> {
        let last_crash: Option<String> = self.conn.query_row(
            "SELECT MAX(day) FROM days WHERE crashes > 0 OR bsod > 0",
            [],
            |row| row.get(0),
        )?;
        let first_day: Option<String> =
            self.conn
                .query_row("SELECT MIN(day) FROM days", [], |row| row.get(0))?;
        Ok((
            last_crash.as_deref().and_then(parse_day),
            first_day.as_deref().and_then(parse_day),
        ))
    }

    /// Event rows older than a year go; daily counts and snapshots stay.
    pub fn prune_events(&self, now: DateTime<Utc>) -> Result<usize, StorageError> {
        let cutoff = now
            .checked_sub_days(Days::new(EVENT_RETENTION_DAYS))
            .unwrap_or(now)
            .timestamp();
        Ok(self
            .conn
            .execute("DELETE FROM events WHERE time < ?1", params![cutoff])?)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::events::Log;

    fn day(text: &str) -> NaiveDate {
        parse_day(text).unwrap()
    }

    fn row(
        record_id: u64,
        day_text: &str,
        level: u8,
        kind: Option<EventKind>,
        detail: Option<&str>,
    ) -> StoredEvent {
        StoredEvent {
            log: if kind == Some(EventKind::AppCrash) {
                Log::Application
            } else {
                Log::System
            },
            record_id,
            time: Utc.with_ymd_and_hms(2026, 10, 27, 10, 0, 0).unwrap(),
            day: day(day_text),
            provider: "test".to_owned(),
            event_id: 1,
            level,
            kind,
            detail: detail.map(str::to_owned),
        }
    }

    #[test]
    fn counts_a_day_like_the_powershell_collector() {
        let mut storage = Storage::in_memory().unwrap();
        storage
            .insert_events(&[
                row(1, "2026-10-27", 1, Some(EventKind::Crash), None),
                row(
                    2,
                    "2026-10-27",
                    2,
                    Some(EventKind::NvContainer),
                    Some("NVIDIA LocalSystem Container"),
                ),
                row(3, "2026-10-27", 2, None, None),
                row(4, "2026-10-27", 3, Some(EventKind::Whea), None),
                row(
                    5,
                    "2026-10-27",
                    2,
                    Some(EventKind::AppCrash),
                    Some("Steam.exe"),
                ),
                row(
                    6,
                    "2026-10-27",
                    2,
                    Some(EventKind::AppCrash),
                    Some("Steam.exe"),
                ),
                row(
                    7,
                    "2026-10-27",
                    2,
                    Some(EventKind::AppCrash),
                    Some("node.exe"),
                ),
                row(8, "2026-10-26", 2, None, None),
            ])
            .unwrap();
        let counts = storage.count_day(day("2026-10-27")).unwrap();
        assert_eq!(counts.crashes, 1);
        assert_eq!(counts.nv_container, 1);
        assert_eq!(counts.whea, 1);
        assert_eq!(counts.app_crashes, 3);
        // System errors: the crash (1), the NVIDIA service (2) and the plain error (2); WHEA is a warning, apps are Application
        assert_eq!(counts.errors, 3);
        assert_eq!(
            counts.top_apps,
            vec!["Steam.exe ×2".to_owned(), "node.exe ×1".to_owned()]
        );
    }

    #[test]
    fn the_same_entry_is_stored_once() {
        let mut storage = Storage::in_memory().unwrap();
        let entry = row(1, "2026-10-27", 2, None, None);
        assert_eq!(
            storage.insert_events(std::slice::from_ref(&entry)).unwrap(),
            1
        );
        assert_eq!(storage.insert_events(&[entry]).unwrap(), 0);
    }

    #[test]
    fn days_round_trip_and_track_the_collector() {
        let storage = Storage::in_memory().unwrap();
        let counts = DayCounts {
            crashes: 2,
            top_apps: vec!["a.exe ×1".to_owned()],
            ..DayCounts::default()
        };
        storage
            .put_day(day("2026-10-20"), &counts, DaySource::Import)
            .unwrap();
        assert_eq!(storage.last_collected_day().unwrap(), None);
        storage
            .put_day(
                day("2026-10-27"),
                &DayCounts::default(),
                DaySource::Collector,
            )
            .unwrap();
        assert_eq!(storage.day(day("2026-10-20")).unwrap(), Some(counts));
        assert_eq!(
            storage.last_collected_day().unwrap(),
            Some(day("2026-10-27"))
        );
        assert_eq!(
            storage.crash_bounds().unwrap(),
            (Some(day("2026-10-20")), Some(day("2026-10-20")))
        );
    }

    #[test]
    fn prunes_rows_older_than_a_year_only() {
        let mut storage = Storage::in_memory().unwrap();
        let mut old = row(1, "2025-10-01", 2, None, None);
        old.time = Utc.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap();
        storage
            .insert_events(&[old, row(2, "2026-10-27", 2, None, None)])
            .unwrap();
        let now = Utc.with_ymd_and_hms(2026, 10, 27, 12, 0, 0).unwrap();
        assert_eq!(storage.prune_events(now).unwrap(), 1);
        assert_eq!(storage.count_day(day("2026-10-27")).unwrap().errors, 1);
    }

    #[test]
    fn the_latest_snapshot_wins() {
        let storage = Storage::in_memory().unwrap();
        let older = Snapshot {
            ram_free_gb: Some(40.0),
            ..Snapshot::default()
        };
        let newer = Snapshot {
            ram_free_gb: Some(50.0),
            ..Snapshot::default()
        };
        storage
            .put_snapshot_if_absent(day("2026-10-26"), &older)
            .unwrap();
        storage
            .put_snapshot(day("2026-10-27"), Utc::now(), &newer)
            .unwrap();
        assert_eq!(storage.latest_snapshot().unwrap(), Some(newer));
    }
}
