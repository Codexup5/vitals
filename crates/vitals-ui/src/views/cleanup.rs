//! The cleanup on the disks screen: what is checked, what the preview says will go, what the run did.

use std::collections::BTreeSet;

use vitals_ipc::{
    Candidate, CandidateKind, CleanupHow, CleanupOutcome, CleanupPreview, CleanupReport, Group,
};

use crate::format;
use crate::i18n::Locale;

/// The ids "Select the safe ones" checks: the whitelist, nothing of the review group, nothing empty.
pub fn safe_ids(candidates: &[Candidate]) -> BTreeSet<String> {
    candidates
        .iter()
        .filter(|candidate| candidate.group == Group::Safe && candidate.bytes > 0)
        .map(|candidate| candidate.id.clone())
        .collect()
}

/// `Selected: 4 · 9.1 GB`, or nothing when nothing is checked.
pub fn selection(
    candidates: &[Candidate],
    selected: &BTreeSet<String>,
    locale: Locale,
) -> Option<String> {
    let chosen: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| selected.contains(&candidate.id))
        .collect();
    if chosen.is_empty() {
        return None;
    }
    let bytes = chosen.iter().map(|candidate| candidate.bytes).sum();
    Some((locale.texts().disks.selected)(
        chosen.len(),
        &format::bytes(bytes, locale),
    ))
}

/// The confirmation in place of the button: what goes for good, and what the panel will not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewView {
    pub plan_id: u64,
    pub lines: Vec<String>,
    /// Nothing would be deleted or run: only cancelling makes sense
    pub is_empty: bool,
}

pub fn preview_view(preview: &CleanupPreview, locale: Locale) -> PreviewView {
    let texts = &locale.texts().disks;
    let count = |how: CleanupHow| preview.items.iter().filter(|item| item.how == how).count();
    let files: u64 = preview
        .items
        .iter()
        .filter(|item| item.how == CleanupHow::Files)
        .map(|item| item.files)
        .sum();
    let bytes = preview
        .items
        .iter()
        .filter(|item| item.how == CleanupHow::Files)
        .map(|item| item.bytes)
        .sum();
    let mut lines = vec![(texts.preview_files)(
        &format::count(files, locale),
        &format::bytes(bytes, locale),
    )];
    let commands = count(CleanupHow::Command);
    if commands > 0 {
        let command_bytes = preview
            .items
            .iter()
            .filter(|item| item.how == CleanupHow::Command)
            .map(|item| item.bytes)
            .sum();
        lines.push((texts.preview_commands)(
            commands,
            &format::bytes(command_bytes, locale),
        ));
    }
    if preview
        .items
        .iter()
        .any(|item| item.needs_admin && item.how == CleanupHow::Files)
    {
        lines.push(texts.preview_admin.to_owned());
    }
    if count(CleanupHow::Manual) > 0 {
        lines.push(texts.preview_manual.to_owned());
    }
    let refused = count(CleanupHow::Refused);
    if refused > 0 {
        lines.push((texts.preview_refused)(refused));
    }
    PreviewView {
        plan_id: preview.plan_id,
        lines,
        is_empty: files == 0 && commands == 0,
    }
}

/// What the run did, and what went wrong item by item.
pub fn report_lines(report: &CleanupReport, locale: Locale) -> Vec<String> {
    let texts = &locale.texts().disks;
    let total = report.total;
    let skipped = total.busy + total.changed + total.refused;
    let mut lines = vec![(texts.report)(
        &format::count(total.deleted, locale),
        &format::count(skipped, locale),
        &format::bytes(total.freed, locale),
    )];
    if skipped > 0 {
        lines.push((texts.report_skipped)(
            &format::count(total.busy, locale),
            &format::count(total.changed, locale),
            &format::count(total.refused, locale),
        ));
    }
    for item in &report.items {
        match item.outcome {
            CleanupOutcome::Declined if item.kind == CandidateKind::WindowsTemp => {
                lines.push(texts.declined.to_owned());
            }
            CleanupOutcome::Declined | CleanupOutcome::Failed => {
                let (title, _) = locale.texts().disk.candidate(item.kind);
                lines.push((texts.failed)(title));
            }
            CleanupOutcome::Done | CleanupOutcome::Skipped => {}
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use vitals_ipc::{CleanupItemReport, CleanupTally, PreviewItem};

    use super::*;

    fn candidate(group: Group, kind: CandidateKind, bytes: u64) -> Candidate {
        Candidate::new(
            group,
            kind,
            Some(PathBuf::from(format!(r"C:\{kind:?}"))),
            bytes,
        )
    }

    #[test]
    fn select_safe_checks_only_the_whitelist_with_something_in_it() {
        let candidates = [
            candidate(Group::Safe, CandidateKind::UserTemp, 10),
            candidate(Group::Safe, CandidateKind::Turbo, 0),
            candidate(Group::Review, CandidateKind::StaleNodeModules, 99),
        ];
        let ids = safe_ids(&candidates);
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&candidates[0].id));
        assert_eq!(selection(&candidates, &BTreeSet::new(), Locale::En), None);
        assert_eq!(
            selection(&candidates, &ids, Locale::En).as_deref(),
            Some("Selected: 1 · 0.0 MB")
        );
    }

    fn item(how: CleanupHow, kind: CandidateKind, files: u64, bytes: u64) -> PreviewItem {
        PreviewItem {
            candidate_id: format!("{kind:?}"),
            kind,
            how,
            files,
            bytes,
            needs_admin: kind == CandidateKind::WindowsTemp,
        }
    }

    #[test]
    fn the_preview_says_what_goes_for_good_and_what_the_panel_will_not_do() {
        let preview = CleanupPreview {
            plan_id: 3,
            items: vec![
                item(
                    CleanupHow::Files,
                    CandidateKind::UserTemp,
                    1200,
                    3 * 1024 * 1024,
                ),
                item(
                    CleanupHow::Files,
                    CandidateKind::WindowsTemp,
                    5,
                    1024 * 1024,
                ),
                item(CleanupHow::Command, CandidateKind::PnpmStore, 0, 0),
                item(CleanupHow::Manual, CandidateKind::Vhdx, 0, 0),
            ],
        };
        let view = preview_view(&preview, Locale::En);
        assert_eq!(
            view.lines[0],
            "1,205 files, 4.0 MB. The Recycle Bin is not used: they cannot be brought back."
        );
        assert_eq!(view.lines.len(), 4);
        assert!(!view.is_empty);
        let nothing = CleanupPreview {
            plan_id: 4,
            items: vec![item(CleanupHow::Refused, CandidateKind::Turbo, 0, 0)],
        };
        assert!(preview_view(&nothing, Locale::En).is_empty);
    }

    #[test]
    fn the_report_counts_and_names_what_failed() {
        let report = CleanupReport {
            finished_at: 0,
            items: vec![
                CleanupItemReport {
                    candidate_id: "a".to_owned(),
                    kind: CandidateKind::UserTemp,
                    outcome: CleanupOutcome::Done,
                    tally: CleanupTally {
                        deleted: 10,
                        busy: 2,
                        changed: 1,
                        refused: 0,
                        freed: 2 * 1024 * 1024,
                    },
                },
                CleanupItemReport {
                    candidate_id: "b".to_owned(),
                    kind: CandidateKind::WindowsTemp,
                    outcome: CleanupOutcome::Declined,
                    tally: CleanupTally::default(),
                },
            ],
            total: CleanupTally {
                deleted: 10,
                busy: 2,
                changed: 1,
                refused: 0,
                freed: 2 * 1024 * 1024,
            },
        };
        let lines = report_lines(&report, Locale::En);
        assert_eq!(lines[0], "Files deleted: 10 · skipped: 3 · freed: 2.0 MB");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2], Locale::En.texts().disks.declined);
    }
}
