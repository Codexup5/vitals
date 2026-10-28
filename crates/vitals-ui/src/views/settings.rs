//! The settings screen: the Telegram bot, sending on or off, and what the recent checks found.

use vitals_ipc::{AlertRecord, AlertSkip, AlertsStatus, ClaudeStatus, ConnectOutcome, TestOutcome};

use crate::format;
use crate::i18n::Locale;
use crate::views::disks::Tone;
use crate::views::runners::Badge;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRow {
    pub key: String,
    pub time: String,
    pub lines: Vec<String>,
    pub outcome: Badge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertsReady {
    pub hint: String,
    pub has_bot: bool,
    pub bot_line: String,
    /// "Use the OLEGPC bot" is offered while no bot is saved and the old file is there
    pub offers_legacy: bool,
    pub is_sending: bool,
    pub sending: Badge,
    pub sending_button: &'static str,
    pub last_check: String,
    pub recent: Vec<RecordRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertsView {
    Loading,
    Error(&'static str),
    Ready(AlertsReady),
}

fn outcome_of(record: &AlertRecord, locale: Locale) -> Badge {
    let texts = &locale.texts().settings;
    match (record.was_sent, record.reason) {
        (true, _) => (texts.sent, Tone::Ok),
        (false, Some(AlertSkip::SendingOff) | None) => (texts.skipped_off, Tone::Accent),
        (false, Some(AlertSkip::NoBot)) => (texts.skipped_no_bot, Tone::Warn),
        (false, Some(AlertSkip::Failed)) => (texts.skipped_failed, Tone::Danger),
    }
}

fn time_of(seconds: i64, locale: Locale) -> String {
    format::local_time(seconds).map_or_else(String::new, |time| format::date_time(time, locale))
}

pub fn alerts_view(
    answer: Option<Result<&AlertsStatus, &'static str>>,
    locale: Locale,
) -> AlertsView {
    let status = match answer {
        None => return AlertsView::Loading,
        Some(Err(message)) => return AlertsView::Error(message),
        Some(Ok(status)) => status,
    };
    let texts = &locale.texts().settings;
    AlertsView::Ready(AlertsReady {
        hint: (texts.alerts_hint)(status.disk_min_gb, status.runner_grace_minutes),
        has_bot: status.has_bot,
        bot_line: match &status.chat_hint {
            Some(hint) if status.has_bot => (texts.bot_saved)(hint),
            _ => texts.no_bot.to_owned(),
        },
        offers_legacy: status.has_legacy_bot && !status.has_bot,
        is_sending: status.is_sending,
        sending: if status.is_sending {
            (texts.sending_on, Tone::Ok)
        } else {
            (texts.sending_off, Tone::Accent)
        },
        sending_button: if status.is_sending {
            texts.turn_off_sending
        } else {
            texts.turn_on_sending
        },
        last_check: status.last_check.map_or_else(
            || texts.never_checked.to_owned(),
            |time| (texts.last_check)(&time_of(time, locale)),
        ),
        recent: status
            .recent
            .iter()
            .enumerate()
            .map(|(index, record)| RecordRow {
                key: format!("{}-{index}", record.time),
                time: time_of(record.time, locale),
                lines: record.lines.clone(),
                outcome: outcome_of(record, locale),
            })
            .collect(),
    })
}

/// The Claude Code block: whether the server is connected, the button, the status line command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeReady {
    pub state: Badge,
    /// None without Claude Code on the PC
    pub button: Option<&'static str>,
    pub status_command: String,
}

pub fn claude_view(status: &ClaudeStatus, locale: Locale) -> ClaudeReady {
    let texts = &locale.texts().settings;
    let state = match (status.has_cli, status.is_connected) {
        (_, true) => (texts.claude_connected, Tone::Ok),
        (true, false) => (texts.claude_not_connected, Tone::Accent),
        (false, false) => (texts.claude_no_cli, Tone::Warn),
    };
    ClaudeReady {
        state,
        button: status.has_cli.then_some(if status.is_connected {
            texts.reconnect
        } else {
            texts.connect
        }),
        status_command: status.status_command.clone(),
    }
}

pub fn connect_line(outcome: ConnectOutcome, locale: Locale) -> (&'static str, Tone) {
    let texts = &locale.texts().settings;
    match outcome {
        ConnectOutcome::Done => (texts.connect_done, Tone::Ok),
        ConnectOutcome::NoCli => (texts.claude_no_cli, Tone::Warn),
        ConnectOutcome::Failed => (texts.connect_failed, Tone::Danger),
    }
}

/// What the screen says after the test button.
pub fn test_line(outcome: TestOutcome, locale: Locale) -> (&'static str, Tone) {
    let texts = &locale.texts().settings;
    match outcome {
        TestOutcome::Sent => (texts.test_sent, Tone::Ok),
        TestOutcome::NoBot => (texts.test_no_bot, Tone::Warn),
        TestOutcome::Refused => (texts.test_refused, Tone::Danger),
        TestOutcome::Unreachable => (texts.test_unreachable, Tone::Warn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> AlertsStatus {
        AlertsStatus {
            has_bot: false,
            chat_hint: None,
            is_sending: false,
            has_legacy_bot: true,
            disk_min_gb: 50,
            runner_grace_minutes: 30,
            last_check: None,
            recent: vec![AlertRecord {
                time: 1_761_600_000,
                lines: vec!["⚠️ Аварийная перезагрузка: 1".to_owned()],
                was_sent: false,
                reason: Some(AlertSkip::SendingOff),
            }],
        }
    }

    fn ready(status: &AlertsStatus) -> AlertsReady {
        match alerts_view(Some(Ok(status)), Locale::Ru) {
            AlertsView::Ready(ready) => ready,
            other => panic!("not ready: {other:?}"),
        }
    }

    #[test]
    fn without_a_bot_the_old_one_is_offered_and_nothing_is_sent() {
        let ready = ready(&status());
        assert!(ready.offers_legacy && !ready.has_bot);
        assert_eq!(ready.sending_button, "Включить отправку");
        assert_eq!(
            ready.recent[0].outcome,
            ("Не отправлено: отправка выключена", Tone::Accent)
        );
        assert!(ready.last_check.starts_with("Проверок ещё не было"));
    }

    #[test]
    fn a_saved_bot_names_its_chat_and_hides_the_old_one() {
        let mut saved = status();
        saved.has_bot = true;
        saved.chat_hint = Some("…7890".to_owned());
        saved.is_sending = true;
        let ready = ready(&saved);
        assert!(!ready.offers_legacy);
        assert!(ready.bot_line.contains("чат …7890"));
        assert_eq!(ready.sending, ("Отправка включена", Tone::Ok));
    }

    #[test]
    fn claude_is_offered_only_where_it_is() {
        let mut status = ClaudeStatus {
            has_cli: false,
            is_connected: false,
            status_command: r#""C:\Vitals\vitals.exe" --status"#.to_owned(),
        };
        assert_eq!(claude_view(&status, Locale::Ru).button, None);
        status.has_cli = true;
        assert_eq!(
            claude_view(&status, Locale::Ru).button,
            Some("Подключить к Claude Code")
        );
        status.is_connected = true;
        let ready = claude_view(&status, Locale::Ru);
        assert_eq!(ready.button, Some("Подключить заново"));
        assert_eq!(ready.state.1, Tone::Ok);
    }

    #[test]
    fn a_refused_test_says_why() {
        assert_eq!(
            test_line(TestOutcome::Refused, Locale::Ru).0,
            "Telegram отказал: неверный токен или номер чата."
        );
    }
}
