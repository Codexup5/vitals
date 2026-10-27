//! What the screens read: days of a range, months, a page of the event feed, the BSOD times.

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{OptionalExtension, ToSql, params, params_from_iter};
pub use vitals_ipc::{DayRow, EventPage, EventRow};

use super::{DayCounts, Storage, StorageError, day_text, parse_day};

/// The kinds of the feed filter: the stored kinds plus `error`, a System error of no known kind.
pub const PLAIN_ERROR: &str = "error";

/// When a BSOD was logged, and its bugcheck code
pub type BsodTime = (DateTime<Utc>, Option<String>);

#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Empty means every kind
    pub kinds: Vec<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

const COUNT_COLUMNS: &str =
    "crashes, bsod, gpu_reset, nv_container, whea, iommu, app_crashes, errors";

fn counts_from(row: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<DayCounts> {
    Ok(DayCounts {
        crashes: row.get(first)?,
        bsod: row.get(first + 1)?,
        gpu_reset: row.get(first + 2)?,
        nv_container: row.get(first + 3)?,
        whea: row.get(first + 4)?,
        iommu: row.get(first + 5)?,
        app_crashes: row.get(first + 6)?,
        errors: row.get(first + 7)?,
        top_apps: Vec::new(),
    })
}

impl Storage {
    /// Every recorded day from `from` to `to`, oldest first; a day the panel never saw is absent.
    pub fn days_between(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DayRow>, StorageError> {
        let mut statement = self.conn.prepare_cached(&format!(
            "SELECT day, {COUNT_COLUMNS}, top_apps FROM days WHERE day BETWEEN ?1 AND ?2 ORDER BY day"
        ))?;
        let rows = statement
            .query_map(params![day_text(from), day_text(to)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    counts_from(row, 1)?,
                    row.get::<_, String>(9)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(day, mut counts, top_apps)| {
                counts.top_apps = serde_json::from_str(&top_apps)?;
                Ok(DayRow { day, counts })
            })
            .collect()
    }

    /// The whole history by calendar month (`2026-10`), oldest first.
    pub fn months(&self) -> Result<Vec<DayRow>, StorageError> {
        let sums = COUNT_COLUMNS
            .split(", ")
            .map(|column| format!("SUM({column})"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut statement = self.conn.prepare_cached(&format!(
            "SELECT substr(day, 1, 7) AS month, {sums} FROM days GROUP BY month ORDER BY month"
        ))?;
        Ok(statement
            .query_map([], |row| {
                Ok(DayRow {
                    day: row.get(0)?,
                    counts: counts_from(row, 1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn events_page(
        &self,
        filter: &EventFilter,
        offset: u32,
        limit: u32,
    ) -> Result<EventPage, StorageError> {
        let mut conditions = Vec::new();
        let mut values: Vec<Box<dyn ToSql>> = Vec::new();
        if !filter.kinds.is_empty() {
            let mut alternatives = Vec::new();
            let known: Vec<&String> = filter
                .kinds
                .iter()
                .filter(|kind| *kind != PLAIN_ERROR)
                .collect();
            if !known.is_empty() {
                alternatives.push(format!("kind IN ({})", vec!["?"; known.len()].join(", ")));
                values.extend(
                    known
                        .into_iter()
                        .map(|kind| Box::new(kind.clone()) as Box<dyn ToSql>),
                );
            }
            if filter.kinds.iter().any(|kind| kind == PLAIN_ERROR) {
                alternatives.push("kind IS NULL".to_owned());
            }
            conditions.push(format!("({})", alternatives.join(" OR ")));
        }
        if let Some(from) = filter.from {
            conditions.push("day >= ?".to_owned());
            values.push(Box::new(day_text(from)));
        }
        if let Some(to) = filter.to {
            conditions.push("day <= ?".to_owned());
            values.push(Box::new(day_text(to)));
        }
        let clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let total: u32 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM events {clause}"),
            params_from_iter(values.iter().map(AsRef::as_ref)),
            |row| row.get(0),
        )?;
        values.push(Box::new(limit));
        values.push(Box::new(offset));
        let mut statement = self.conn.prepare(&format!(
            "SELECT time, log, provider, event_id, level, COALESCE(kind, '{PLAIN_ERROR}'), detail
             FROM events {clause} ORDER BY time DESC, record_id DESC LIMIT ? OFFSET ?"
        ))?;
        let items = statement
            .query_map(params_from_iter(values.iter().map(AsRef::as_ref)), |row| {
                Ok(EventRow {
                    time: row.get(0)?,
                    log: row.get(1)?,
                    provider: row.get(2)?,
                    event_id: row.get(3)?,
                    level: row.get(4)?,
                    kind: row.get(5)?,
                    detail: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EventPage { items, total })
    }

    /// Every stored BSOD with its code: the dump list matches its files to these by time.
    pub fn bsod_times(&self) -> Result<Vec<BsodTime>, StorageError> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT time, detail FROM events WHERE kind = 'bsod' ORDER BY time")?;
        Ok(statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .filter_map(|row| match row {
                Ok((time, code)) => DateTime::from_timestamp(time, 0).map(|time| Ok((time, code))),
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Keeps the last ten scans: the newest is what the screen shows, the older ones are kept for comparison.
    pub fn put_scan(&self, finished_at: i64, body: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO scans (finished_at, body) VALUES (?1, ?2)",
            params![finished_at, body],
        )?;
        self.conn.execute(
            "DELETE FROM scans WHERE id NOT IN (SELECT id FROM scans ORDER BY id DESC LIMIT 10)",
            [],
        )?;
        Ok(())
    }

    pub fn latest_scan(&self) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn
            .query_row(
                "SELECT body FROM scans ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn first_day(&self) -> Result<Option<NaiveDate>, StorageError> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT MIN(day) FROM days", [], |row| row.get(0))?;
        Ok(text.as_deref().and_then(parse_day))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::events::{EventKind, Log, StoredEvent};
    use crate::storage::DaySource;

    fn day(text: &str) -> NaiveDate {
        parse_day(text).unwrap()
    }

    fn event(
        record_id: u64,
        hour: u32,
        kind: Option<EventKind>,
        detail: Option<&str>,
    ) -> StoredEvent {
        StoredEvent {
            log: Log::System,
            record_id,
            time: Utc.with_ymd_and_hms(2026, 10, 27, hour, 0, 0).unwrap(),
            day: day("2026-10-27"),
            provider: "test".to_owned(),
            event_id: 1,
            level: 2,
            kind,
            detail: detail.map(str::to_owned),
        }
    }

    #[test]
    fn days_of_a_range_and_months_of_the_history() {
        let storage = Storage::in_memory().unwrap();
        for (text, crashes) in [("2026-09-30", 1), ("2026-10-01", 2), ("2026-10-27", 3)] {
            let counts = DayCounts {
                crashes,
                ..DayCounts::default()
            };
            storage
                .put_day(day(text), &counts, DaySource::Import)
                .unwrap();
        }
        let range = storage
            .days_between(day("2026-10-01"), day("2026-10-31"))
            .unwrap();
        assert_eq!(
            range.iter().map(|row| row.day.as_str()).collect::<Vec<_>>(),
            ["2026-10-01", "2026-10-27"]
        );
        let months = storage.months().unwrap();
        assert_eq!(
            months
                .iter()
                .map(|row| (row.day.as_str(), row.counts.crashes))
                .collect::<Vec<_>>(),
            [("2026-09", 1), ("2026-10", 5)]
        );
        assert_eq!(storage.first_day().unwrap(), Some(day("2026-09-30")));
    }

    #[test]
    fn the_feed_filters_by_kind_newest_first_and_pages() {
        let mut storage = Storage::in_memory().unwrap();
        storage
            .insert_events(&[
                event(1, 1, Some(EventKind::Crash), None),
                event(2, 2, None, None),
                event(3, 3, Some(EventKind::Bsod), Some("0x0000004e")),
                event(4, 4, None, None),
            ])
            .unwrap();
        let all = storage.events_page(&EventFilter::default(), 0, 2).unwrap();
        assert_eq!(all.total, 4);
        assert_eq!(
            all.items
                .iter()
                .map(|row| row.kind.as_str())
                .collect::<Vec<_>>(),
            ["error", "bsod"]
        );

        let filter = EventFilter {
            kinds: vec!["crash".to_owned(), PLAIN_ERROR.to_owned()],
            ..EventFilter::default()
        };
        let filtered = storage.events_page(&filter, 1, 10).unwrap();
        assert_eq!(filtered.total, 3);
        assert_eq!(
            filtered
                .items
                .iter()
                .map(|row| row.kind.as_str())
                .collect::<Vec<_>>(),
            ["error", "crash"]
        );

        let outside = EventFilter {
            from: Some(day("2026-10-28")),
            ..EventFilter::default()
        };
        assert_eq!(storage.events_page(&outside, 0, 10).unwrap().total, 0);
        assert_eq!(storage.bsod_times().unwrap().len(), 1);
    }

    #[test]
    fn keeps_the_last_ten_scans_and_returns_the_newest() {
        let storage = Storage::in_memory().unwrap();
        assert_eq!(storage.latest_scan().unwrap(), None);
        for index in 0..12 {
            storage.put_scan(index, &format!("scan {index}")).unwrap();
        }
        assert_eq!(storage.latest_scan().unwrap().as_deref(), Some("scan 11"));
    }
}
