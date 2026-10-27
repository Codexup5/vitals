//! The collector: counts the days from the event log, keeps the machine snapshot, and says what the tray shows.

use std::time::{Duration, Instant};
pub use vitals_ipc::HealthSummary;

use chrono::{DateTime, Days, Local, NaiveDate, TimeZone, Utc};

use crate::storage::{DaySource, Snapshot, Storage, StorageError};
use crate::system::{self, SlowFacts};

/// The brief, §12: with the window closed only this light collector runs, every five minutes.
pub const INTERVAL: Duration = Duration::from_secs(5 * 60);
const SLOW_FACTS_EVERY: Duration = Duration::from_secs(60 * 60);
/// How far back the first run counts when the database has no counted day yet.
const FIRST_RUN_DAYS: u64 = 29;

#[derive(Debug, thiserror::Error)]
pub enum CollectorError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[cfg(windows)]
    #[error(transparent)]
    EventLog(#[from] crate::events::windows::EventLogError),
}

/// The first day to count again: the last counted day (it may have been partial), or a month back on the first run,
/// but never a day the event log no longer covers completely — that day keeps its imported counts.
pub fn first_day_to_count(
    last_counted: Option<NaiveDate>,
    today: NaiveDate,
    oldest_in_log: Option<NaiveDate>,
) -> NaiveDate {
    let wanted = last_counted.unwrap_or_else(|| {
        today
            .checked_sub_days(Days::new(FIRST_RUN_DAYS))
            .unwrap_or(today)
    });
    let first_full = oldest_in_log
        .and_then(|oldest| oldest.succ_opt())
        .unwrap_or(wanted);
    wanted.max(first_full).min(today)
}

pub fn summarize(
    last_crash: Option<NaiveDate>,
    first_day: Option<NaiveDate>,
    today: NaiveDate,
    snapshot: Option<&Snapshot>,
    system_drive: &str,
) -> HealthSummary {
    let since = last_crash.or(first_day);
    HealthSummary {
        days_without_crash: since.map(|day| (today - day).num_days().max(0)),
        system_drive_free_gb: snapshot.and_then(|snapshot| {
            snapshot
                .disks
                .iter()
                .find(|disk| disk.drive.eq_ignore_ascii_case(system_drive))
                .map(|disk| disk.free_gb)
        }),
    }
}

pub fn summary_from(storage: &Storage, today: NaiveDate) -> Result<HealthSummary, StorageError> {
    let (last_crash, first_day) = storage.crash_bounds()?;
    let snapshot = storage.latest_snapshot()?;
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
    Ok(summarize(
        last_crash,
        first_day,
        today,
        snapshot.as_ref(),
        system_drive.trim_end_matches(':'),
    ))
}

fn local_midnight_utc(day: NaiveDate) -> DateTime<Utc> {
    Local
        .from_local_datetime(&day.and_time(chrono::NaiveTime::MIN))
        .earliest()
        .map_or_else(
            || day.and_time(chrono::NaiveTime::MIN).and_utc(),
            |time| time.with_timezone(&Utc),
        )
}

#[derive(Debug)]
pub struct Collector {
    storage: Storage,
    slow: Option<(Instant, SlowFacts)>,
}

impl Collector {
    pub fn new(storage: Storage) -> Self {
        Self {
            storage,
            slow: None,
        }
    }

    /// One pass: prune, count the days from the event log, take the snapshot, return what the tray shows.
    #[cfg(windows)]
    pub fn run_once(&mut self, now: DateTime<Utc>) -> Result<HealthSummary, CollectorError> {
        use crate::events::{Log, to_stored, windows as event_log};

        self.storage.prune_events(now)?;
        let today = now.with_timezone(&Local).date_naive();
        let oldest = event_log::oldest_time(Log::System)?
            .map(|time| time.with_timezone(&Local).date_naive());
        let from = first_day_to_count(self.storage.last_collected_day()?, today, oldest);
        let since = local_midnight_utc(from);

        let mut stored = Vec::new();
        for log in [Log::System, Log::Application] {
            stored.extend(
                event_log::read_since(log, since)?
                    .into_iter()
                    .filter_map(|event| to_stored(event, &Local)),
            );
        }
        let inserted = self.storage.insert_events(&stored)?;
        let mut day = from;
        while day <= today {
            let counts = self.storage.count_day(day)?;
            self.storage.put_day(day, &counts, DaySource::Collector)?;
            let Some(next) = day.succ_opt() else { break };
            day = next;
        }
        tracing::info!(
            "collected {} entries ({inserted} new) for {from}..={today}",
            stored.len()
        );

        let slow = match &self.slow {
            Some((at, facts)) if at.elapsed() < SLOW_FACTS_EVERY => facts.clone(),
            _ => {
                let facts = system::windows::slow_facts();
                self.slow = Some((Instant::now(), facts.clone()));
                facts
            }
        };
        let snapshot = system::to_snapshot(&system::windows::fast_facts(), &slow);
        self.storage.put_snapshot(today, now, &snapshot)?;
        self.summary(today)
    }

    pub fn summary(&self, today: NaiveDate) -> Result<HealthSummary, CollectorError> {
        Ok(summary_from(&self.storage, today)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::DiskFree;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn the_first_run_counts_a_month_back_within_the_log() {
        let today = day("2026-10-27");
        assert_eq!(first_day_to_count(None, today, None), day("2026-09-28"));
        // The log starts on 10.10 mid-day: 10.10 keeps its imported counts, counting starts on 11.10
        assert_eq!(
            first_day_to_count(None, today, Some(day("2026-10-10"))),
            day("2026-10-11")
        );
    }

    #[test]
    fn a_later_run_recounts_from_the_last_counted_day() {
        let today = day("2026-10-28");
        assert_eq!(
            first_day_to_count(Some(day("2026-10-27")), today, Some(day("2026-10-01"))),
            day("2026-10-27")
        );
        assert_eq!(
            first_day_to_count(Some(day("2026-10-28")), today, Some(day("2026-10-28"))),
            today
        );
    }

    #[test]
    fn days_without_crash_and_the_system_drive() {
        let snapshot = Snapshot {
            disks: vec![
                DiskFree {
                    drive: "C".to_owned(),
                    free_gb: 207.6,
                },
                DiskFree {
                    drive: "D".to_owned(),
                    free_gb: 193.0,
                },
            ],
            ..Snapshot::default()
        };
        let summary = summarize(
            Some(day("2026-10-13")),
            Some(day("2026-07-13")),
            day("2026-10-27"),
            Some(&snapshot),
            "C",
        );
        assert_eq!(
            summary,
            HealthSummary {
                days_without_crash: Some(14),
                system_drive_free_gb: Some(207.6)
            }
        );
        let fresh = summarize(None, None, day("2026-10-27"), None, "C");
        assert_eq!(
            fresh,
            HealthSummary {
                days_without_crash: None,
                system_drive_free_gb: None
            }
        );
    }
}
