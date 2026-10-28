//! The settings screen: the start at sign-in, the Telegram bot, sending on or off, what the recent checks found, and
//! Claude Code.

use vitals_ipc::{
    AlertRecord, AlertSkip, AlertsStatus, AutostartStatus, ClaudeStatus, ConnectOutcome,
    TestOutcome,
};

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
    /// The old OLEGPC tasks still on: what to say, and the commands that turn them off
    pub old_tasks: Option<(String, Vec<String>)>,
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
        old_tasks: (!status.old_tasks_on.is_empty()).then(|| {
            (
                (texts.old_tasks_on)(&status.old_tasks_on.join(", ")),
                status
                    .old_tasks_on
                    .iter()
                    .map(|task| format!(r#"schtasks /Change /TN "\{task}" /DISABLE"#))
                    .collect(),
            )
        }),
    })
}

/// The start at sign-in: its state, the copy it starts when not this one, and the button with what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartReady {
    pub state: Badge,
    pub other_copy: Option<String>,
    pub button: &'static str,
    /// What the button asks: on (for this copy) or off
    pub enable: bool,
}

pub fn autostart_view(status: &AutostartStatus, locale: Locale) -> AutostartReady {
    let texts = &locale.texts().settings;
    match (&status.other_copy, status.is_enabled) {
        (Some(path), true) => AutostartReady {
            state: (texts.autostart_on, Tone::Warn),
            other_copy: Some((texts.autostart_other)(path)),
            button: texts.autostart_this_copy,
            enable: true,
        },
        (_, true) => AutostartReady {
            state: (texts.autostart_on, Tone::Ok),
            other_copy: None,
            button: texts.autostart_turn_off,
            enable: false,
        },
        (_, false) => AutostartReady {
            state: (texts.autostart_off, Tone::Accent),
            other_copy: None,
            button: texts.autostart_turn_on,
            enable: true,
        },
    }
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
            old_tasks_on: Vec::new(),
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
    fn the_old_tasks_are_named_only_while_they_are_on() {
        assert_eq!(ready(&status()).old_tasks, None);
        let mut old = status();
        old.old_tasks_on = vec![
            r"OLEGPC\Health collect".to_owned(),
            r"OLEGPC\Alerts".to_owned(),
        ];
        let (hint, commands) = ready(&old).old_tasks.unwrap();
        assert!(hint.contains(r"OLEGPC\Health collect, OLEGPC\Alerts"));
        assert_eq!(
            commands,
            [
                r#"schtasks /Change /TN "\OLEGPC\Health collect" /DISABLE"#,
                r#"schtasks /Change /TN "\OLEGPC\Alerts" /DISABLE"#
            ]
        );
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
    fn the_start_at_sign_in_offers_the_other_way_or_this_copy() {
        let mut status = AutostartStatus {
            is_enabled: false,
            other_copy: None,
        };
        let off = autostart_view(&status, Locale::En);
        assert_eq!(
            (off.state, off.button, off.enable),
            (
                ("Does not start at sign-in", Tone::Accent),
                "Start at sign-in",
                true
            )
        );
        status.is_enabled = true;
        let on = autostart_view(&status, Locale::En);
        assert_eq!((on.button, on.enable), ("Do not start at sign-in", false));
        status.other_copy = Some(r"C:\Old\vitals.exe".to_owned());
        let other = autostart_view(&status, Locale::En);
        assert_eq!(
            (other.state.1, other.button, other.enable),
            (Tone::Warn, "Start this copy", true)
        );
        assert!(
            other
                .other_copy
                .is_some_and(|line| line.ends_with(r"C:\Old\vitals.exe"))
        );
    }

    #[test]
    fn a_refused_test_says_why() {
        assert_eq!(
            test_line(TestOutcome::Refused, Locale::Ru).0,
            "Telegram отказал: неверный токен или номер чата."
        );
    }
}
