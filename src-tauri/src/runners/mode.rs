//! The two modes of the PC, as `mode-gaming.ps1` and `mode-coding.ps1` did them: games stop Docker Desktop and WSL,
//! coding starts Docker Desktop and waits for its containers to come up. One switch at a time, in the background.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use vitals_ipc::{Mode, ModeResult, ModeStatus};

/// What a switch asks of Docker and WSL; the tests give it a fake.
pub trait Stack: Send + Sync + 'static {
    /// `docker desktop stop`; whether it ended well
    fn stop_docker(&self) -> bool;
    /// `wsl --shutdown`
    fn shutdown_wsl(&self) -> bool;
    /// `docker desktop start`
    fn start_docker(&self) -> bool;
    /// The running containers, `None` while the engine does not answer
    fn running(&self) -> Option<u32>;
    fn sleep(&self, duration: Duration);
}

/// How often the count of running containers is read while they come up
const POLL: Duration = Duration::from_secs(4);
/// The containers have come up when their count holds this many polls in a row
const STABLE_POLLS: u32 = 3;
/// Past this many polls (two minutes) the switch reports what runs
const MAX_POLLS: u32 = 30;

/// Stops the dev stack; the Windows runners keep working, the ones in containers go offline.
pub fn to_gaming(stack: &impl Stack) -> (bool, u32) {
    let is_stopped = stack.stop_docker();
    let is_shut = stack.shutdown_wsl();
    (is_stopped && is_shut, 0)
}

/// Starts Docker Desktop and waits until the count of running containers stops growing.
pub fn to_coding(stack: &impl Stack) -> (bool, u32) {
    if !stack.start_docker() {
        return (false, stack.running().unwrap_or(0));
    }
    let mut last = None;
    let mut steady = 0;
    for _ in 0..MAX_POLLS {
        let now = stack.running();
        if now.is_some() && now == last {
            steady += 1;
            if steady >= STABLE_POLLS {
                break;
            }
        } else {
            steady = 0;
        }
        last = now;
        stack.sleep(POLL);
    }
    (last.is_some(), last.unwrap_or(0))
}

#[derive(Debug, Default)]
pub struct ModeJob {
    status: Mutex<ModeStatus>,
}

impl ModeJob {
    fn lock(&self) -> MutexGuard<'_, ModeStatus> {
        // A switch that panicked leaves a status that is still worth showing
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn status(&self) -> ModeStatus {
        *self.lock()
    }

    /// Starts a switch unless one runs.
    pub fn start(
        self: &Arc<Self>,
        mode: Mode,
        stack: Arc<impl Stack>,
        now: impl Fn() -> i64 + Send + 'static,
    ) -> bool {
        {
            let mut status = self.lock();
            if status.switching.is_some() {
                return false;
            }
            status.switching = Some(mode);
        }
        let job = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("mode".to_owned())
            .spawn(move || {
                let (is_done, running) = match mode {
                    Mode::Gaming => to_gaming(stack.as_ref()),
                    Mode::Coding => to_coding(stack.as_ref()),
                };
                tracing::info!("mode {mode:?}: done {is_done}, {running} containers run");
                let mut status = job.lock();
                status.switching = None;
                status.last = Some(ModeResult {
                    mode,
                    is_done,
                    running,
                    finished_at: now(),
                });
            });
        if let Err(error) = spawned {
            tracing::error!("the mode thread did not start: {error}");
            self.lock().switching = None;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Instant;

    use super::*;

    struct FakeStack {
        counts: Mutex<Vec<Option<u32>>>,
        starts: bool,
        steps: Mutex<Vec<&'static str>>,
    }

    impl FakeStack {
        fn new(counts: Vec<Option<u32>>, starts: bool) -> Self {
            Self {
                counts: Mutex::new(counts),
                starts,
                steps: Mutex::new(Vec::new()),
            }
        }

        fn step(&self, name: &'static str) {
            self.steps.lock().expect("not poisoned").push(name);
        }
    }

    impl Stack for FakeStack {
        fn stop_docker(&self) -> bool {
            self.step("stop docker");
            true
        }

        fn shutdown_wsl(&self) -> bool {
            self.step("shutdown wsl");
            false
        }

        fn start_docker(&self) -> bool {
            self.step("start docker");
            self.starts
        }

        fn running(&self) -> Option<u32> {
            let mut counts = self.counts.lock().expect("not poisoned");
            if counts.len() > 1 {
                counts.remove(0)
            } else {
                counts.first().copied().flatten()
            }
        }

        fn sleep(&self, _duration: Duration) {}
    }

    #[test]
    fn gaming_stops_docker_then_wsl_and_says_if_a_step_failed() {
        let stack = FakeStack::new(vec![None], true);
        assert_eq!(to_gaming(&stack), (false, 0));
        assert_eq!(
            *stack.steps.lock().expect("not poisoned"),
            ["stop docker", "shutdown wsl"]
        );
    }

    #[test]
    fn coding_waits_until_the_containers_stop_coming_up() {
        let stack = FakeStack::new(
            vec![
                None,
                None,
                Some(1),
                Some(4),
                Some(6),
                Some(6),
                Some(6),
                Some(6),
            ],
            true,
        );
        assert_eq!(to_coding(&stack), (true, 6));
    }

    #[test]
    fn coding_reports_an_engine_that_never_answered() {
        let stack = FakeStack::new(vec![None], true);
        assert_eq!(to_coding(&stack), (false, 0));
        let refused = FakeStack::new(vec![None], false);
        assert_eq!(to_coding(&refused), (false, 0));
    }

    #[test]
    fn one_switch_at_a_time() {
        let job = Arc::new(ModeJob::default());
        let stack = Arc::new(FakeStack::new(vec![Some(3)], true));
        assert!(job.start(Mode::Coding, Arc::clone(&stack), || 7));
        let started = Instant::now();
        while job.status().switching.is_some() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "the switch hangs"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let last = job.status().last.expect("a result");
        assert_eq!(
            (last.mode, last.is_done, last.running, last.finished_at),
            (Mode::Coding, true, 3, 7)
        );
    }
}
