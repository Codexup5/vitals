//! The updates screen: the winget packages with their versions, the job of the core, what the last run did, and the
//! tools the panel only opens.

use std::collections::BTreeSet;

use vitals_ipc::{
    Package, UpdateTools, UpdatesPhase, UpdatesStatus, UpgradeOutcome, UpgradeResult,
};

use crate::format;
use crate::i18n::Locale;
use crate::views::disks::Tone;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRow {
    pub id: String,
    pub name: String,
    pub versions: String,
    pub is_open: bool,
    /// winget cut the id: the package is listed, not upgraded from the panel
    pub can_upgrade: bool,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultLine {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatesView {
    Loading,
    Error(&'static str),
    NoWinget,
    Ready {
        /// When the list was checked, or what the job does now
        status_label: String,
        is_busy: bool,
        check_failed: bool,
        rows: Vec<PackageRow>,
        results: Vec<ResultLine>,
    },
}

/// The ids "Select all" checks: whatever winget can be told exactly.
pub fn selectable_ids(packages: &[Package]) -> BTreeSet<String> {
    packages
        .iter()
        .filter(|package| package.can_upgrade)
        .map(|package| package.id.clone())
        .collect()
}

/// `0x8A150108` for winget's HRESULTs, the number itself for an installer's code.
pub fn code_text(code: Option<i32>) -> String {
    match code {
        Some(code) if code < 0 => format!("0x{:08X}", code.cast_unsigned()),
        Some(code) => code.to_string(),
        None => "—".to_owned(),
    }
}

pub fn result_line(result: &UpgradeResult, locale: Locale) -> ResultLine {
    let texts = &locale.texts().updates;
    let name = result.name.as_str();
    let (text, tone) = match result.outcome {
        UpgradeOutcome::Done => match result.after.as_deref() {
            Some(after) if after != result.before => {
                ((texts.done)(name, &result.before, after), Tone::Ok)
            }
            after => (
                (texts.unchanged)(name, after.unwrap_or(&result.before)),
                Tone::Warn,
            ),
        },
        UpgradeOutcome::Open => ((texts.was_open)(name), Tone::Warn),
        UpgradeOutcome::NeedsRestart => ((texts.needs_restart)(name), Tone::Warn),
        UpgradeOutcome::Declined => ((texts.declined)(name), Tone::Warn),
        UpgradeOutcome::Failed => ((texts.failed)(name, &code_text(result.code)), Tone::Danger),
    };
    ResultLine { text, tone }
}

fn status_label(status: &UpdatesStatus, locale: Locale) -> String {
    let texts = &locale.texts().updates;
    match status.phase {
        UpdatesPhase::Checking => texts.checking.to_owned(),
        UpdatesPhase::Upgrading => {
            let current = status.current.as_deref().map(|id| {
                status
                    .packages
                    .iter()
                    .find(|package| package.id == id)
                    .map_or(id, |package| package.name.as_str())
            });
            current.map_or_else(|| texts.checking.to_owned(), |name| (texts.upgrading)(name))
        }
        UpdatesPhase::Idle => status.checked_at.and_then(format::local_time).map_or_else(
            || texts.never_checked.to_owned(),
            |time| (texts.checked)(&format::date_time(time, locale)),
        ),
    }
}

/// The screen from the status of the core: `None` while the first answer is on its way.
pub fn updates_view(
    answer: Option<Result<&UpdatesStatus, &'static str>>,
    locale: Locale,
) -> UpdatesView {
    let status = match answer {
        None => return UpdatesView::Loading,
        Some(Err(message)) => return UpdatesView::Error(message),
        Some(Ok(status)) => status,
    };
    if status.no_winget {
        return UpdatesView::NoWinget;
    }
    let texts = &locale.texts().updates;
    let rows = status
        .packages
        .iter()
        .map(|package| PackageRow {
            id: package.id.clone(),
            name: package.name.clone(),
            versions: (texts.versions)(&package.installed, &package.available),
            is_open: package.is_open,
            can_upgrade: package.can_upgrade,
            is_current: status.current.as_deref() == Some(package.id.as_str()),
        })
        .collect();
    UpdatesView::Ready {
        status_label: status_label(status, locale),
        is_busy: status.phase != UpdatesPhase::Idle,
        check_failed: status.check_failed,
        rows,
        results: status
            .results
            .iter()
            .map(|result| result_line(result, locale))
            .collect(),
    }
}

/// `ASRock X870 Pro RS · version 4.60 of 17/09/2026`, or nothing when Windows did not say.
pub fn firmware_line(tools: &UpdateTools, locale: Locale) -> Option<String> {
    let firmware = &tools.firmware;
    let version = firmware.version.as_deref()?;
    let missing = locale.texts().common.missing;
    let released = firmware
        .released
        .as_deref()
        .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .map_or_else(|| missing.to_owned(), |day| format::date(day, locale));
    Some((locale.texts().updates.firmware_line)(
        firmware.board.as_deref().unwrap_or(missing),
        version,
        &released,
    ))
}

#[cfg(test)]
mod tests {
    use vitals_ipc::Firmware;

    use super::*;

    fn package(id: &str, can_upgrade: bool) -> Package {
        Package {
            id: id.to_owned(),
            name: format!("{id} app"),
            installed: "1.0".to_owned(),
            available: "2.0".to_owned(),
            source: "winget".to_owned(),
            is_open: false,
            can_upgrade,
        }
    }

    fn status(phase: UpdatesPhase) -> UpdatesStatus {
        UpdatesStatus {
            phase,
            checked_at: None,
            check_failed: false,
            no_winget: false,
            packages: vec![package("A", true), package("Cut…", false)],
            current: None,
            results: Vec::new(),
        }
    }

    #[test]
    fn select_all_leaves_out_what_winget_cannot_be_told() {
        let ids = selectable_ids(&status(UpdatesPhase::Idle).packages);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), ["A"]);
    }

    #[test]
    fn the_status_says_what_the_job_does() {
        let mut upgrading = status(UpdatesPhase::Upgrading);
        upgrading.current = Some("A".to_owned());
        let UpdatesView::Ready {
            status_label,
            is_busy,
            rows,
            ..
        } = updates_view(Some(Ok(&upgrading)), Locale::En)
        else {
            panic!("ready");
        };
        assert_eq!(status_label, "Updating: A app");
        assert!(is_busy);
        assert!(rows[0].is_current);
        assert_eq!(rows[0].versions, "1.0 → 2.0");

        let UpdatesView::Ready { status_label, .. } =
            updates_view(Some(Ok(&status(UpdatesPhase::Idle))), Locale::En)
        else {
            panic!("ready");
        };
        assert_eq!(status_label, "Not checked yet");
    }

    #[test]
    fn no_winget_and_no_answer_have_their_own_states() {
        let mut missing = status(UpdatesPhase::Idle);
        missing.no_winget = true;
        assert_eq!(
            updates_view(Some(Ok(&missing)), Locale::En),
            UpdatesView::NoWinget
        );
        assert_eq!(updates_view(None, Locale::En), UpdatesView::Loading);
    }

    fn result(outcome: UpgradeOutcome, after: Option<&str>, code: Option<i32>) -> UpgradeResult {
        UpgradeResult {
            id: "Git.Git".to_owned(),
            name: "Git".to_owned(),
            before: "2.51.2".to_owned(),
            after: after.map(str::to_owned),
            outcome,
            code,
        }
    }

    #[test]
    fn a_result_names_the_versions_or_what_to_do() {
        let done = result_line(
            &result(UpgradeOutcome::Done, Some("2.56.0.3"), None),
            Locale::En,
        );
        assert_eq!(done.text, "Git: 2.51.2 → 2.56.0.3");
        assert_eq!(done.tone, Tone::Ok);
        let same = result_line(
            &result(UpgradeOutcome::Done, Some("2.51.2"), None),
            Locale::En,
        );
        assert_eq!(same.tone, Tone::Warn);
        let failed = result_line(
            &result(
                UpgradeOutcome::Failed,
                None,
                Some(0x8A15_0108_u32.cast_signed()),
            ),
            Locale::En,
        );
        assert_eq!(failed.text, "Git: failed (code 0x8A150108)");
        assert_eq!(code_text(Some(1603)), "1603");
    }

    #[test]
    fn the_firmware_line_needs_a_version() {
        let tools = UpdateTools {
            has_gpu_tool: true,
            firmware: Firmware {
                board: Some("ASRock X870 Pro RS".to_owned()),
                version: Some("4.60".to_owned()),
                released: Some("2026-09-17".to_owned()),
            },
        };
        assert_eq!(
            firmware_line(&tools, Locale::En).as_deref(),
            Some("ASRock X870 Pro RS · version 4.60 of 17/09/2026")
        );
        let unknown = UpdateTools {
            has_gpu_tool: false,
            firmware: Firmware {
                board: None,
                version: None,
                released: None,
            },
        };
        assert_eq!(firmware_line(&unknown, Locale::En), None);
    }
}
