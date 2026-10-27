//! Telegram alerts, as the old `alert-check.ps1` sent them: a crash, a BSOD with its code, WHEA, a GPU reset since the
//! last check; C: below 50 GB once a day; the runners offline for 30 minutes while Docker Desktop runs (the gaming
//! mode stops Docker on purpose, so then nothing is said), and once more when they are back.
//!
//! Every check that finds something is written to `alerts-log.jsonl`. Sending is off by default (the brief, §1 p.14):
//! while it is off, the log says what would have gone, which is how the week next to the old alerts is compared.

pub mod telegram;
mod texts;
#[cfg(windows)]
pub mod windows;

use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use vitals_ipc::{AlertRecord, AlertSkip};

use crate::storage::{Storage, StorageError};

pub use texts::AlertTexts;

pub const DISK_MIN_GB: u32 = 50;
pub const RUNNER_GRACE_MINUTES: u32 = 30;
/// The first check looks back as far as the old task's interval
const FIRST_LOOK_BACK: i64 = 15 * 60;
const LOG_FILE: &str = "alerts-log.jsonl";
const RECENT: usize = 20;
const STATE_KEY: &str = "alerts.state";
pub const SENDING_KEY: &str = "alerts.sending";

#[derive(Debug, thiserror::Error)]
pub enum AlertsError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("the alerts log: {0}")]
    Io(#[from] std::io::Error),
    #[error("the alerts state: {0}")]
    Json(#[from] serde_json::Error),
}

/// What the log of events holds since the last check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Unix seconds of each crash, oldest first
    pub crashes: Vec<i64>,
    /// The code of each BSOD, none when the event had none
    pub bsod_codes: Vec<Option<String>>,
    pub whea: u32,
    pub gpu_resets: u32,
}

/// The runners, when they are watched at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runners {
    /// Docker Desktop does not run: the gaming mode or a PC without it; the watch starts over
    DockerOff,
    /// GitHub did not answer: nothing changes
    Unknown,
    Offline(Vec<String>),
    Online,
}

/// What the checks remember between them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    /// Unix seconds
    pub last_check: Option<i64>,
    /// `YYYY-MM-DD` of the day the low disk was said
    pub disk_alert_day: Option<String>,
    pub runners_offline_since: Option<i64>,
    pub runners_alerted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Alert {
    Crashes { count: usize, last: i64 },
    Bsod { codes: Vec<Option<String>> },
    Whea { count: u32 },
    GpuResets { count: u32 },
    LowDisk { free_gb: f64 },
    RunnersOffline { names: Vec<String> },
    RunnersBack,
}

/// One check: what to say and what to remember, from what was found now.
pub fn evaluate(
    state: &State,
    found: &Found,
    free_c_gb: Option<f64>,
    runners: &Runners,
    now: i64,
    today: &str,
) -> (Vec<Alert>, State) {
    let mut alerts = Vec::new();
    let mut next = state.clone();
    if let Some(&last) = found.crashes.last() {
        alerts.push(Alert::Crashes {
            count: found.crashes.len(),
            last,
        });
    }
    if !found.bsod_codes.is_empty() {
        alerts.push(Alert::Bsod {
            codes: found.bsod_codes.clone(),
        });
    }
    if found.whea > 0 {
        alerts.push(Alert::Whea { count: found.whea });
    }
    if found.gpu_resets > 0 {
        alerts.push(Alert::GpuResets {
            count: found.gpu_resets,
        });
    }
    if let Some(free_gb) = free_c_gb
        && free_gb < f64::from(DISK_MIN_GB)
        && state.disk_alert_day.as_deref() != Some(today)
    {
        alerts.push(Alert::LowDisk { free_gb });
        next.disk_alert_day = Some(today.to_owned());
    }
    match runners {
        Runners::DockerOff => {
            next.runners_offline_since = None;
            next.runners_alerted = false;
        }
        Runners::Unknown => {}
        Runners::Offline(names) => match state.runners_offline_since {
            None => next.runners_offline_since = Some(now),
            Some(since)
                if !state.runners_alerted
                    && now - since >= i64::from(RUNNER_GRACE_MINUTES) * 60 =>
            {
                alerts.push(Alert::RunnersOffline {
                    names: names.clone(),
                });
                next.runners_alerted = true;
            }
            Some(_) => {}
        },
        Runners::Online => {
            if state.runners_alerted {
                alerts.push(Alert::RunnersBack);
            }
            next.runners_offline_since = None;
            next.runners_alerted = false;
        }
    }
    next.last_check = Some(now);
    (alerts, next)
}

/// The message: the name of the PC, then a line per alert.
pub fn message(computer: &str, lines: &[String]) -> String {
    std::iter::once(computer.to_owned())
        .chain(lines.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn read_state(storage: &Storage, now: i64) -> Result<State, AlertsError> {
    let state: State = match storage.setting(STATE_KEY)? {
        Some(text) => serde_json::from_str(&text)?,
        None => State::default(),
    };
    Ok(State {
        last_check: Some(state.last_check.unwrap_or(now - FIRST_LOOK_BACK)),
        ..state
    })
}

pub fn is_sending(storage: &Storage) -> Result<bool, StorageError> {
    Ok(storage.setting(SENDING_KEY)?.as_deref() == Some("true"))
}

fn append_log(data_dir: &Path, record: &AlertRecord) -> Result<(), AlertsError> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join(LOG_FILE))?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

/// The last checks that found something, newest first.
pub fn recent(data_dir: &Path) -> Vec<AlertRecord> {
    let Ok(text) = std::fs::read_to_string(data_dir.join(LOG_FILE)) else {
        return Vec::new();
    };
    text.lines()
        .rev()
        .filter_map(|line| serde_json::from_str(line).ok())
        .take(RECENT)
        .collect()
}

/// What happens to the lines of one check: sent, or written with the reason they were not.
pub fn deliver(
    lines: Vec<String>,
    now: i64,
    is_sending: bool,
    bot: Option<&telegram::Bot>,
    send: impl FnOnce(&telegram::Bot, &str) -> Result<(), telegram::SendError>,
    computer: &str,
) -> AlertRecord {
    let reason = match (is_sending, bot) {
        (false, _) => Some(AlertSkip::SendingOff),
        (true, None) => Some(AlertSkip::NoBot),
        (true, Some(bot)) => send(bot, &message(computer, &lines))
            .map_err(|error| tracing::warn!("the alert did not go: {error}"))
            .err()
            .map(|()| AlertSkip::Failed),
    };
    AlertRecord {
        time: now,
        lines,
        was_sent: reason.is_none(),
        reason,
    }
}

/// One check after a pass of the collector; it never stops the collector, a failure is logged.
#[cfg(windows)]
pub fn check(db_path: &Path, data_dir: &Path, is_russian: bool, now: i64) {
    if let Err(error) = check_once(db_path, data_dir, is_russian, now) {
        tracing::warn!("the alerts check failed: {error}");
    }
}

#[cfg(windows)]
fn check_once(
    db_path: &Path,
    data_dir: &Path,
    is_russian: bool,
    now: i64,
) -> Result<(), AlertsError> {
    let storage = Storage::open(db_path)?;
    let state = read_state(&storage, now)?;
    let found = storage.alert_events_since(state.last_check.unwrap_or(now))?;
    let free_c_gb = crate::system::windows::fast_facts()
        .disks
        .iter()
        .find(|(letter, _)| *letter == 'C')
        .map(|(_, free)| crate::system::round1(gigabytes(*free)));
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let (alerts, next) = evaluate(&state, &found, free_c_gb, &watch_runners(), now, &today);
    storage.set_setting(STATE_KEY, &serde_json::to_string(&next)?)?;
    if alerts.is_empty() {
        return Ok(());
    }
    let texts = AlertTexts::of(is_russian);
    let lines = texts::lines(&alerts, texts);
    let bot = windows::load_bot(data_dir);
    let record = deliver(
        lines,
        now,
        is_sending(&storage)?,
        bot.as_ref(),
        |bot, text| telegram::send(bot, text, data_dir),
        &computer_name(),
    );
    tracing::info!(
        "alerts: {} lines, sent: {}",
        record.lines.len(),
        record.was_sent
    );
    append_log(data_dir, &record)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "a free space in bytes is shown with one decimal of a gigabyte"
)]
fn gigabytes(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Vitals".to_owned())
}

/// The runners are watched only while Docker Desktop runs, as the old alerts did.
#[cfg(windows)]
fn watch_runners() -> Runners {
    use vitals_ipc::{GithubAccess, RunnerStatus};

    let is_docker_running = crate::system::windows::running_exes().iter().any(|exe| {
        exe.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Docker Desktop.exe"))
    });
    if !is_docker_running {
        return Runners::DockerOff;
    }
    let overview = crate::runners::source::overview();
    if overview.github != GithubAccess::Ok {
        return Runners::Unknown;
    }
    let offline: Vec<String> = overview
        .runners
        .iter()
        .filter(|runner| runner.status == Some(RunnerStatus::Offline))
        .map(|runner| runner.name.clone())
        .collect();
    if offline.is_empty() {
        Runners::Online
    } else {
        Runners::Offline(offline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_761_600_000;

    fn found_crash() -> Found {
        Found {
            crashes: vec![NOW - 600, NOW - 300],
            bsod_codes: vec![Some("0x0000004e".to_owned()), None],
            whea: 1,
            gpu_resets: 0,
        }
    }

    #[test]
    fn crashes_and_bsods_since_the_last_check_are_said() {
        let (alerts, next) = evaluate(
            &State::default(),
            &found_crash(),
            Some(120.0),
            &Runners::DockerOff,
            NOW,
            "2026-10-28",
        );
        assert_eq!(
            alerts,
            vec![
                Alert::Crashes {
                    count: 2,
                    last: NOW - 300
                },
                Alert::Bsod {
                    codes: vec![Some("0x0000004e".to_owned()), None]
                },
                Alert::Whea { count: 1 },
            ]
        );
        assert_eq!(next.last_check, Some(NOW));
    }

    #[test]
    fn a_low_disk_is_said_once_a_day() {
        let (alerts, next) = evaluate(
            &State::default(),
            &Found::default(),
            Some(42.5),
            &Runners::DockerOff,
            NOW,
            "2026-10-28",
        );
        assert_eq!(alerts, vec![Alert::LowDisk { free_gb: 42.5 }]);
        let (again, _) = evaluate(
            &next,
            &Found::default(),
            Some(41.0),
            &Runners::DockerOff,
            NOW + 300,
            "2026-10-28",
        );
        assert_eq!(again, []);
        let (tomorrow, _) = evaluate(
            &next,
            &Found::default(),
            Some(41.0),
            &Runners::DockerOff,
            NOW,
            "2026-10-29",
        );
        assert_eq!(tomorrow.len(), 1);
    }

    #[test]
    fn runners_are_said_after_the_grace_and_once_back() {
        let offline = Runners::Offline(vec!["kit-linux-1".to_owned()]);
        let (first, state) = evaluate(
            &State::default(),
            &Found::default(),
            None,
            &offline,
            NOW,
            "d",
        );
        assert_eq!(first, []);
        assert_eq!(state.runners_offline_since, Some(NOW));
        let (early, state) = evaluate(
            &state,
            &Found::default(),
            None,
            &offline,
            NOW + 29 * 60,
            "d",
        );
        assert_eq!(early, []);
        let (late, state) = evaluate(
            &state,
            &Found::default(),
            None,
            &offline,
            NOW + 30 * 60,
            "d",
        );
        assert_eq!(
            late,
            vec![Alert::RunnersOffline {
                names: vec!["kit-linux-1".to_owned()]
            }]
        );
        let (still, state) = evaluate(
            &state,
            &Found::default(),
            None,
            &offline,
            NOW + 45 * 60,
            "d",
        );
        assert_eq!(still, []);
        let (unknown, state) = evaluate(
            &state,
            &Found::default(),
            None,
            &Runners::Unknown,
            NOW + 50 * 60,
            "d",
        );
        assert!(unknown.is_empty() && state.runners_alerted);
        let (back, state) = evaluate(
            &state,
            &Found::default(),
            None,
            &Runners::Online,
            NOW + 55 * 60,
            "d",
        );
        assert_eq!(back, vec![Alert::RunnersBack]);
        assert!(!state.runners_alerted && state.runners_offline_since.is_none());
    }

    #[test]
    fn the_gaming_mode_starts_the_watch_over_without_a_word() {
        let state = State {
            runners_offline_since: Some(NOW - 3600),
            runners_alerted: true,
            ..State::default()
        };
        let (alerts, next) = evaluate(
            &state,
            &Found::default(),
            None,
            &Runners::DockerOff,
            NOW,
            "d",
        );
        assert_eq!(alerts, []);
        assert!(!next.runners_alerted && next.runners_offline_since.is_none());
    }

    #[test]
    fn off_means_written_not_sent() {
        let bot = telegram::Bot {
            token: "1:a".to_owned(),
            chat_id: "1".to_owned(),
        };
        let never = |_: &telegram::Bot, _: &str| -> Result<(), telegram::SendError> {
            panic!("must not send")
        };
        let record = deliver(vec!["x".to_owned()], NOW, false, Some(&bot), never, "PC");
        assert_eq!(
            (record.was_sent, record.reason),
            (false, Some(AlertSkip::SendingOff))
        );
        let record = deliver(vec!["x".to_owned()], NOW, true, None, never, "PC");
        assert_eq!(record.reason, Some(AlertSkip::NoBot));
        let mut sent = String::new();
        let record = deliver(
            vec!["x".to_owned()],
            NOW,
            true,
            Some(&bot),
            |_, text| {
                sent = text.to_owned();
                Ok(())
            },
            "OLEGPC",
        );
        assert!(record.was_sent);
        assert_eq!(sent, "OLEGPC\nx");
    }

    #[test]
    fn the_log_reads_back_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for time in [1, 2] {
            let record = AlertRecord {
                time,
                lines: vec!["x".to_owned()],
                was_sent: false,
                reason: Some(AlertSkip::SendingOff),
            };
            append_log(dir.path(), &record).unwrap();
        }
        let times: Vec<i64> = recent(dir.path())
            .iter()
            .map(|record| record.time)
            .collect();
        assert_eq!(times, [2, 1]);
    }
}
