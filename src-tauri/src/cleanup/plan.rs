//! A cleanup in two steps: the preview lists what each checked candidate will delete, the run executes exactly that
//! list and writes one line to `cleanup-log.jsonl`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use vitals_ipc::{
    CleanupHow, CleanupItemReport, CleanupOutcome, CleanupPreview, CleanupReport, PreviewItem,
};

use super::execute::{self, Tally, Target};
use super::snapshot::{Filter, take};
use super::sources::{TEMP_AGE, TEMP_KEEP};
use super::{Candidate, CandidateKind, Places, elevated, guard, process};

pub const LOG_FILE: &str = "cleanup-log.jsonl";

#[derive(Debug, Clone)]
enum Action {
    Files { target: Target, needs_admin: bool },
    Command { folder: Option<PathBuf> },
    Manual,
    Refused,
}

#[derive(Debug, Clone)]
struct Item {
    candidate: Candidate,
    action: Action,
}

/// The snapshot a run executes.
#[derive(Debug, Clone)]
pub struct Plan {
    pub id: u64,
    items: Vec<Item>,
}

fn action_of(candidate: &Candidate, places: &Places, now: SystemTime) -> Action {
    if candidate.kind == CandidateKind::Vhdx {
        return Action::Manual;
    }
    if process::command_of(candidate.kind).is_some() {
        return Action::Command {
            folder: candidate.path.clone(),
        };
    }
    let Some(path) = candidate.path.as_deref() else {
        return Action::Refused;
    };
    let Ok(root) = guard::allowed_root(candidate.kind, path, places) else {
        return Action::Refused;
    };
    let filter = match candidate.kind {
        CandidateKind::UserTemp | CandidateKind::WindowsTemp => Filter::OlderThan {
            cutoff: now.checked_sub(TEMP_AGE).unwrap_or(SystemTime::UNIX_EPOCH),
            skip_names: if candidate.kind == CandidateKind::UserTemp {
                TEMP_KEEP
            } else {
                &[]
            },
        },
        _ => Filter::All,
    };
    Action::Files {
        target: Target {
            entries: take(&root, filter),
            root,
            removes_root: candidate.kind == CandidateKind::StaleNodeModules,
        },
        needs_admin: candidate.needs_admin,
    }
}

/// Lists what each chosen candidate will delete; a candidate the guard refuses gets no files.
pub fn preview(id: u64, chosen: &[Candidate], places: &Places, now: SystemTime) -> Plan {
    Plan {
        id,
        items: chosen
            .iter()
            .map(|candidate| Item {
                action: action_of(candidate, places, now),
                candidate: candidate.clone(),
            })
            .collect(),
    }
}

impl Plan {
    pub fn summary(&self) -> CleanupPreview {
        CleanupPreview {
            plan_id: self.id,
            items: self
                .items
                .iter()
                .map(|item| {
                    let (how, files, bytes) = match &item.action {
                        Action::Files { target, .. } => (
                            CleanupHow::Files,
                            u64::try_from(target.entries.len()).unwrap_or(u64::MAX),
                            target.entries.iter().map(|entry| entry.size).sum(),
                        ),
                        Action::Command { .. } => (CleanupHow::Command, 0, item.candidate.bytes),
                        Action::Manual => (CleanupHow::Manual, 0, 0),
                        Action::Refused => (CleanupHow::Refused, 0, 0),
                    };
                    PreviewItem {
                        candidate_id: item.candidate.id.clone(),
                        kind: item.candidate.kind,
                        how,
                        files,
                        bytes,
                        needs_admin: item.candidate.needs_admin,
                    }
                })
                .collect(),
        }
    }
}

fn run_item(item: &Item, places: &Places, work_dir: &Path) -> (CleanupOutcome, Tally) {
    match &item.action {
        Action::Files {
            target,
            needs_admin: false,
        } => (
            CleanupOutcome::Done,
            execute::run(target, &guard::forbidden(places)),
        ),
        Action::Files {
            target,
            needs_admin: true,
        } => match elevated::run(target, work_dir) {
            Ok(tally) => (CleanupOutcome::Done, tally),
            Err(elevated::ElevatedError::Declined) => (CleanupOutcome::Declined, Tally::default()),
            Err(error) => {
                tracing::warn!("the Windows temp cleanup failed: {error}");
                (CleanupOutcome::Failed, Tally::default())
            }
        },
        Action::Command { folder } => {
            match process::clean(item.candidate.kind, folder.as_deref()) {
                Some(freed) => (
                    CleanupOutcome::Done,
                    Tally {
                        freed,
                        ..Tally::default()
                    },
                ),
                None => (CleanupOutcome::Failed, Tally::default()),
            }
        }
        Action::Manual | Action::Refused => (CleanupOutcome::Skipped, Tally::default()),
    }
}

/// Executes the plan, item by item; one failed item never stops the others.
pub fn run(plan: &Plan, places: &Places, work_dir: &Path, finished_at: i64) -> CleanupReport {
    let mut total = Tally::default();
    let items = plan
        .items
        .iter()
        .map(|item| {
            let (outcome, tally) = run_item(item, places, work_dir);
            total.add(tally);
            CleanupItemReport {
                candidate_id: item.candidate.id.clone(),
                kind: item.candidate.kind,
                outcome,
                tally,
            }
        })
        .collect();
    CleanupReport {
        finished_at,
        items,
        total,
    }
}

/// Appends the report as one JSON line: the history of every cleanup stays on the PC.
pub fn log(report: &CleanupReport, data_dir: &Path) -> std::io::Result<()> {
    let line = serde_json::to_string(report).map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join(LOG_FILE))?;
    writeln!(file, "{line}")
}

/// The candidates of a scan after a run: what each one freed is gone from its size.
pub fn shrink(candidates: &mut [Candidate], report: &CleanupReport) {
    for candidate in candidates {
        if let Some(item) = report
            .items
            .iter()
            .find(|item| item.candidate_id == candidate.id)
        {
            candidate.bytes = candidate.bytes.saturating_sub(item.tally.freed);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use vitals_ipc::Group;

    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn places(root: &Path) -> Places {
        Places::under(root)
    }

    #[test]
    fn preview_and_run_share_one_snapshot_and_every_run_is_logged() {
        let dir = tempfile::tempdir().unwrap();
        let places = places(dir.path());
        write(&places.temp.join("old.tmp"), 40);
        write(&places.temp.join("claude/session.json"), 1000);
        let project = places.code_roots[0].join("app");
        write(&project.join(".turbo/cache.bin"), 60);
        let candidates = vec![
            Candidate::new(
                Group::Safe,
                CandidateKind::UserTemp,
                Some(places.temp.clone()),
                40,
            ),
            Candidate::new(
                Group::Safe,
                CandidateKind::Turbo,
                Some(project.join(".turbo")),
                60,
            ),
            // A Turbo candidate pointing at Downloads never gets files
            Candidate::new(
                Group::Safe,
                CandidateKind::Turbo,
                Some(places.home.join("Downloads")),
                9,
            ),
        ];
        fs::create_dir_all(places.home.join("Downloads")).unwrap();
        let in_a_month = SystemTime::now() + Duration::from_secs(30 * 24 * 60 * 60);
        let plan = preview(7, &candidates, &places, in_a_month);
        let summary = plan.summary();
        assert_eq!(
            summary.items[0].bytes, 40,
            "claude sessions stay out of the preview"
        );
        assert_eq!(summary.items[2].how, CleanupHow::Refused);

        // Written after the preview: not on the list, so it stays
        write(&places.temp.join("late.tmp"), 5);
        let report = run(&plan, &places, dir.path(), 1_793_097_600);
        assert_eq!((report.total.deleted, report.total.freed), (2, 100));
        assert!(places.temp.join("late.tmp").exists());
        assert!(places.temp.join("claude/session.json").exists());

        log(&report, dir.path()).unwrap();
        log(&report, dir.path()).unwrap();
        let lines = fs::read_to_string(dir.path().join(LOG_FILE)).unwrap();
        assert_eq!(lines.lines().count(), 2);
        let logged: CleanupReport = serde_json::from_str(lines.lines().next().unwrap()).unwrap();
        assert_eq!(logged, report);

        let mut scanned = candidates;
        shrink(&mut scanned, &report);
        assert_eq!(scanned[0].bytes, 0);
    }

    #[test]
    fn a_disk_image_is_never_run_by_the_panel() {
        let dir = tempfile::tempdir().unwrap();
        let places = places(dir.path());
        let vhdx = Candidate::new(
            Group::Review,
            CandidateKind::Vhdx,
            Some(dir.path().join("ext4.vhdx")),
            1,
        );
        let plan = preview(1, &[vhdx], &places, SystemTime::now());
        assert_eq!(plan.summary().items[0].how, CleanupHow::Manual);
        let report = run(&plan, &places, dir.path(), 0);
        assert_eq!(report.items[0].outcome, CleanupOutcome::Skipped);
    }
}
