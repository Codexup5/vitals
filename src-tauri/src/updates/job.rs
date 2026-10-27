//! One updates job at a time, in the background: a check lists the packages, a run upgrades the checked ones one by
//! one and checks again. The window polls the status; winget can take minutes, longer than a call may wait.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use vitals_ipc::{UpdatesPhase, UpdatesStatus};

use super::{Package, UpgradeOutcome, UpgradeResult, outcome_of};

/// What the job asks of Windows and winget; the tests give it a fake.
pub trait Host: Send + Sync + 'static {
    /// The packages winget can update, `None` when winget did not answer, `Some(Err)` when it is not installed
    fn list(&self) -> Result<Option<Vec<Package>>, NoWinget>;
    /// Whether a program of the package runs now
    fn is_open(&self, package: &Package) -> bool;
    /// winget's exit code for the upgrade of the package, `None` when it did not finish
    fn upgrade(&self, package: &Package) -> Option<i32>;
    fn installed_version(&self, id: &str) -> Option<String>;
}

/// winget is not on the PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoWinget;

#[derive(Debug)]
pub struct UpdatesJob {
    status: Mutex<UpdatesStatus>,
}

impl Default for UpdatesJob {
    fn default() -> Self {
        Self {
            status: Mutex::new(UpdatesStatus {
                phase: UpdatesPhase::Idle,
                checked_at: None,
                check_failed: false,
                no_winget: false,
                packages: Vec::new(),
                current: None,
                results: Vec::new(),
            }),
        }
    }
}

/// Upgrades one package: an open program is not touched, a finished upgrade has its version read back.
pub fn upgrade_one(package: &Package, host: &impl Host) -> UpgradeResult {
    let result = |outcome, after, code| UpgradeResult {
        id: package.id.clone(),
        name: package.name.clone(),
        before: package.installed.clone(),
        after,
        outcome,
        code,
    };
    if host.is_open(package) {
        return result(UpgradeOutcome::Open, None, None);
    }
    match host.upgrade(package) {
        None => result(UpgradeOutcome::Failed, None, None),
        Some(code) => {
            let outcome = outcome_of(code);
            let after = host.installed_version(&package.id);
            let code = (outcome != UpgradeOutcome::Done).then_some(code);
            result(outcome, after, code)
        }
    }
}

impl UpdatesJob {
    pub fn status(&self) -> UpdatesStatus {
        self.lock().clone()
    }

    fn lock(&self) -> MutexGuard<'_, UpdatesStatus> {
        // A thread that panicked mid-update leaves a status that is still worth showing
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes the job for a phase unless one runs.
    fn begin(&self, phase: UpdatesPhase) -> bool {
        let mut status = self.lock();
        if status.phase != UpdatesPhase::Idle {
            return false;
        }
        status.phase = phase;
        true
    }

    fn check_now(&self, host: &impl Host, now: i64) {
        let listed = host.list();
        let mut status = self.lock();
        status.no_winget = listed.is_err();
        status.check_failed = matches!(listed, Ok(None));
        if let Ok(Some(mut packages)) = listed {
            drop(status);
            for package in &mut packages {
                package.is_open = host.is_open(package);
            }
            status = self.lock();
            status.packages = packages;
            status.checked_at = Some(now);
        }
    }

    /// Lists the packages in the background unless the job is busy.
    pub fn check(
        self: &Arc<Self>,
        host: Arc<impl Host>,
        now: impl Fn() -> i64 + Send + 'static,
    ) -> bool {
        if !self.begin(UpdatesPhase::Checking) {
            return false;
        }
        let job = Arc::clone(self);
        self.spawn(move || {
            job.check_now(host.as_ref(), now());
            job.lock().phase = UpdatesPhase::Idle;
        })
    }

    /// Upgrades the packages of the last check with these ids, in its order, then checks again. An id the check did
    /// not list, or listed cut, is not run.
    pub fn run(
        self: &Arc<Self>,
        ids: &[String],
        host: Arc<impl Host>,
        now: impl Fn() -> i64 + Send + 'static,
    ) -> bool {
        let chosen: Vec<Package> = self
            .lock()
            .packages
            .iter()
            .filter(|package| package.can_upgrade && ids.contains(&package.id))
            .cloned()
            .collect();
        if chosen.is_empty() || !self.begin(UpdatesPhase::Upgrading) {
            return false;
        }
        self.lock().results.clear();
        let job = Arc::clone(self);
        self.spawn(move || {
            for package in &chosen {
                job.lock().current = Some(package.id.clone());
                let result = upgrade_one(package, host.as_ref());
                tracing::info!(
                    "winget upgrade {}: {:?} (code {:?}), {} -> {:?}",
                    result.id,
                    result.outcome,
                    result.code,
                    result.before,
                    result.after
                );
                let mut status = job.lock();
                status.results.push(result);
                status.current = None;
            }
            job.lock().phase = UpdatesPhase::Checking;
            job.check_now(host.as_ref(), now());
            job.lock().phase = UpdatesPhase::Idle;
        })
    }

    fn spawn(self: &Arc<Self>, work: impl FnOnce() + Send + 'static) -> bool {
        match std::thread::Builder::new()
            .name("updates".to_owned())
            .spawn(work)
        {
            Ok(_) => true,
            Err(error) => {
                tracing::error!("the updates thread did not start: {error}");
                self.lock().phase = UpdatesPhase::Idle;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::*;

    #[derive(Default)]
    struct FakeHost {
        packages: Vec<Package>,
        open: Vec<String>,
        codes: Vec<(String, i32)>,
        upgraded: Mutex<Vec<String>>,
    }

    impl Host for FakeHost {
        fn list(&self) -> Result<Option<Vec<Package>>, NoWinget> {
            let upgraded = self.upgraded.lock().expect("not poisoned");
            Ok(Some(
                self.packages
                    .iter()
                    .filter(|package| !upgraded.contains(&package.id))
                    .cloned()
                    .collect(),
            ))
        }

        fn is_open(&self, package: &Package) -> bool {
            self.open.contains(&package.id)
        }

        fn upgrade(&self, package: &Package) -> Option<i32> {
            let code = self
                .codes
                .iter()
                .find(|(id, _)| *id == package.id)
                .map_or(0, |(_, code)| *code);
            if code == 0 {
                self.upgraded
                    .lock()
                    .expect("not poisoned")
                    .push(package.id.clone());
            }
            Some(code)
        }

        fn installed_version(&self, id: &str) -> Option<String> {
            let upgraded = self.upgraded.lock().expect("not poisoned");
            let package = self.packages.iter().find(|package| package.id == id)?;
            Some(if upgraded.contains(&package.id) {
                package.available.clone()
            } else {
                package.installed.clone()
            })
        }
    }

    fn package(id: &str, can_upgrade: bool) -> Package {
        Package {
            id: id.to_owned(),
            name: id.to_owned(),
            installed: "1.0".to_owned(),
            available: "2.0".to_owned(),
            source: "winget".to_owned(),
            is_open: false,
            can_upgrade,
        }
    }

    fn wait_idle(job: &UpdatesJob) -> UpdatesStatus {
        let started = Instant::now();
        loop {
            let status = job.status();
            if status.phase == UpdatesPhase::Idle {
                return status;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "the job hangs");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn an_open_program_is_not_upgraded() {
        let host = FakeHost {
            open: vec!["A".to_owned()],
            ..FakeHost::default()
        };
        let result = upgrade_one(&package("A", true), &host);
        assert_eq!(result.outcome, UpgradeOutcome::Open);
        assert!(host.upgraded.lock().expect("not poisoned").is_empty());
    }

    #[test]
    fn a_run_upgrades_only_listed_whole_ids_and_reads_versions_back() {
        let host = Arc::new(FakeHost {
            packages: vec![
                package("A", true),
                package("B", true),
                package("Cut…", false),
            ],
            codes: vec![("B".to_owned(), 0x8A15_0109_u32.cast_signed())],
            ..FakeHost::default()
        });
        let job = Arc::new(UpdatesJob::default());
        assert!(job.check(Arc::clone(&host), || 1));
        assert_eq!(wait_idle(&job).packages.len(), 3);

        let ids = ["A", "B", "Cut…", "Not.Listed"].map(str::to_owned);
        assert!(job.run(&ids, Arc::clone(&host), || 2));
        let status = wait_idle(&job);
        let outcomes: Vec<(&str, UpgradeOutcome, Option<&str>)> = status
            .results
            .iter()
            .map(|result| (result.id.as_str(), result.outcome, result.after.as_deref()))
            .collect();
        assert_eq!(
            outcomes,
            [
                ("A", UpgradeOutcome::Done, Some("2.0")),
                ("B", UpgradeOutcome::NeedsRestart, Some("1.0"))
            ]
        );
        // The check after the run no longer lists what was upgraded
        assert_eq!(status.checked_at, Some(2));
        assert!(status.packages.iter().all(|package| package.id != "A"));
    }

    #[test]
    fn nothing_chosen_starts_nothing() {
        let job = Arc::new(UpdatesJob::default());
        assert!(!job.run(&["A".to_owned()], Arc::new(FakeHost::default()), || 0));
        assert_eq!(job.status().phase, UpdatesPhase::Idle);
    }
}
