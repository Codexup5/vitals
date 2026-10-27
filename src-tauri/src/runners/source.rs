//! Where the runners screen reads from: the runner folders of this PC, `gh` for GitHub's view, `docker` for the
//! containers. The calls run side by side; each has a deadline, and a missing tool is a state, not an error.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use vitals_ipc::{DockerState, GithubAccess, RunnersOverview};

use super::{LocalRunner, containers_of, github_runners, is_listening_in, local_runner, merge};
use crate::process::{RunError, output};

const GH_TIMEOUT: Duration = Duration::from_secs(10);
const DOCKER_TIMEOUT: Duration = Duration::from_secs(10);
/// The runner's own installer suggests `C:\actions-runner`; a second one on the same PC gets a suffix
const RUNNER_FOLDER_PREFIX: &str = "actions-runner";

fn system_drive() -> PathBuf {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
    PathBuf::from(format!("{drive}\\"))
}

/// The configured runners in `C:\actions-runner*`.
pub fn local_runners(root: &Path) -> Vec<LocalRunner> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut runners: Vec<LocalRunner> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .starts_with(RUNNER_FOLDER_PREFIX)
        })
        .filter_map(|entry| {
            let folder = entry.path();
            let text = std::fs::read_to_string(folder.join(".runner")).ok()?;
            local_runner(&text, &folder)
        })
        .collect();
    runners.sort_by(|left, right| left.folder.cmp(&right.folder));
    runners
}

/// A program on the PATH, as Windows would find it.
pub fn find_on_path(program: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|folder| folder.join(program))
        .find(|candidate| candidate.is_file())
}

/// The runners screen is shown when there is something to show: Docker or a runner folder.
pub fn is_present() -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    find_on_path("docker.exe", &path).is_some() || !local_runners(&system_drive()).is_empty()
}

enum Github {
    Runners(Vec<vitals_ipc::Runner>),
    NoGh,
    Failed,
}

fn ask_github(repo: &str) -> Github {
    let endpoint = format!("repos/{repo}/actions/runners");
    match output("gh", &["api", &endpoint], GH_TIMEOUT) {
        Ok(done) if done.code == 0 => {
            github_runners(&done.stdout, repo).map_or(Github::Failed, Github::Runners)
        }
        Err(RunError::Missing) => Github::NoGh,
        Ok(_) | Err(_) => Github::Failed,
    }
}

fn ask_docker() -> (DockerState, Vec<vitals_ipc::Container>) {
    match output(
        "docker",
        &["ps", "--all", "--format", "{{json .}}"],
        DOCKER_TIMEOUT,
    ) {
        Ok(done) if done.code == 0 => (DockerState::Running, containers_of(&done.stdout)),
        Err(RunError::Missing) => (DockerState::Missing, Vec::new()),
        // The CLI is there, its engine is not: Docker Desktop is stopped
        Ok(_) | Err(_) => (DockerState::Stopped, Vec::new()),
    }
}

#[cfg(windows)]
fn running() -> Vec<PathBuf> {
    crate::system::windows::running_exes()
}

#[cfg(not(windows))]
fn running() -> Vec<PathBuf> {
    Vec::new()
}

pub fn overview() -> RunnersOverview {
    let local = local_runners(&system_drive());
    let repos: BTreeSet<&str> = local.iter().map(|runner| runner.repo.as_str()).collect();
    let (answers, (docker, containers)) = std::thread::scope(|scope| {
        let asks: Vec<_> = repos
            .iter()
            .map(|repo| scope.spawn(move || ask_github(repo)))
            .collect();
        let docker = scope.spawn(ask_docker);
        let answers: Vec<Github> = asks
            .into_iter()
            .map(|ask| ask.join().unwrap_or(Github::Failed))
            .collect();
        (
            answers,
            docker.join().unwrap_or((DockerState::Stopped, Vec::new())),
        )
    });
    let mut remote = Vec::new();
    let mut github = GithubAccess::Ok;
    for answer in answers {
        match answer {
            Github::Runners(runners) => remote.extend(runners),
            Github::NoGh => github = GithubAccess::NoGh,
            Github::Failed if github == GithubAccess::Ok => github = GithubAccess::Partial,
            Github::Failed => {}
        }
    }
    let running = running();
    RunnersOverview {
        runners: merge(remote, &local, |folder| is_listening_in(folder, &running)),
        github,
        docker,
        containers,
    }
}

/// Docker Desktop starts and stops in minutes on a busy PC; WSL shuts down in seconds
const DESKTOP_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const WSL_TIMEOUT: Duration = Duration::from_secs(2 * 60);

fn is_ok(program: &str, args: &[&str], timeout: Duration) -> bool {
    output(program, args, timeout)
        .map_err(|error| tracing::warn!("{program} {args:?}: {error}"))
        .is_ok_and(|done| done.code == 0)
}

/// The real stack: Docker Desktop's own CLI and `wsl`.
#[derive(Debug, Default)]
pub struct DockerStack;

impl super::mode::Stack for DockerStack {
    fn stop_docker(&self) -> bool {
        is_ok("docker", &["desktop", "stop"], DESKTOP_TIMEOUT)
    }

    fn shutdown_wsl(&self) -> bool {
        is_ok("wsl", &["--shutdown"], WSL_TIMEOUT)
    }

    fn start_docker(&self) -> bool {
        is_ok("docker", &["desktop", "start"], DESKTOP_TIMEOUT)
    }

    fn running(&self) -> Option<u32> {
        let done = output("docker", &["ps", "--quiet"], DOCKER_TIMEOUT).ok()?;
        (done.code == 0).then(|| {
            u32::try_from(
                done.stdout
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count(),
            )
            .unwrap_or(u32::MAX)
        })
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_configured_runner_folders_count() {
        let root = tempfile::tempdir().expect("temp dir");
        let configured = root.path().join("actions-runner-vitals");
        std::fs::create_dir(&configured).expect("folder");
        std::fs::write(
            configured.join(".runner"),
            "\u{feff}{\"agentName\":\"OlegPC-vitals\",\"gitHubUrl\":\"https://github.com/Codexup5/vitals\"}",
        )
        .expect("file");
        // A cache folder of the same prefix, and a runner folder never configured
        std::fs::create_dir(root.path().join("actions-runner-cache")).expect("folder");
        std::fs::create_dir(root.path().join("other")).expect("folder");
        let runners = local_runners(root.path());
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].repo, "Codexup5/vitals");
        assert_eq!(runners[0].folder, configured);
    }

    #[test]
    fn a_program_is_found_on_the_path() {
        let folder = tempfile::tempdir().expect("temp dir");
        std::fs::write(folder.path().join("tool.exe"), b"").expect("file");
        let path =
            std::env::join_paths([Path::new(r"C:\no-such-folder"), folder.path()]).expect("joins");
        assert_eq!(
            find_on_path("tool.exe", &path),
            Some(folder.path().join("tool.exe"))
        );
        assert_eq!(find_on_path("other.exe", &path), None);
    }
}
