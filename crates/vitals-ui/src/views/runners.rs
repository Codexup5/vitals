//! The runners screen: the runners with GitHub's status and this PC's, the containers by compose project, and what
//! the last switch between the modes did.

use vitals_ipc::{
    Container, DockerState, GithubAccess, Mode, ModeStatus, Runner, RunnerStatus, RunnersOverview,
};

use crate::i18n::Locale;
use crate::views::disks::Tone;

/// A word and the tone that repeats it
pub type Badge = (&'static str, Tone);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerRow {
    pub key: String,
    pub name: String,
    pub repo: String,
    /// The custom labels; `self-hosted`, the OS and the architecture say nothing new
    pub labels: String,
    pub github: Badge,
    pub local: Option<Badge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRow {
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: Badge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGroup {
    pub title: String,
    pub rows: Vec<ContainerRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnersReady {
    pub runners: Vec<RunnerRow>,
    pub github_note: Option<&'static str>,
    pub docker_label: String,
    pub docker_tone: Tone,
    pub projects: Vec<ProjectGroup>,
    pub running: usize,
    pub is_docker_running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnersView {
    Loading,
    Error(&'static str),
    Ready(RunnersReady),
}

/// Labels every runner has: they say nothing about this one
const PLAIN_LABELS: &[&str] = &["self-hosted", "x64", "arm64", "linux", "windows", "macos"];

fn runner_row(runner: &Runner, locale: Locale) -> RunnerRow {
    let texts = &locale.texts().runners;
    let github = match (runner.status, runner.is_busy) {
        (Some(RunnerStatus::Online), true) => (texts.busy, Tone::Accent),
        (Some(RunnerStatus::Online), false) => (texts.online, Tone::Ok),
        (Some(RunnerStatus::Offline), _) => (texts.offline, Tone::Danger),
        (None, _) => (texts.no_status, Tone::Warn),
    };
    let local = runner.is_listening.map(|is_listening| {
        if is_listening {
            (texts.listening, Tone::Ok)
        } else {
            (texts.not_listening, Tone::Warn)
        }
    });
    RunnerRow {
        key: format!("{}/{}", runner.repo, runner.name),
        name: runner.name.clone(),
        repo: runner.repo.clone(),
        labels: runner
            .labels
            .iter()
            .filter(|label| !PLAIN_LABELS.contains(&label.to_lowercase().as_str()))
            .cloned()
            .collect::<Vec<_>>()
            .join(" · "),
        github,
        local,
    }
}

fn state_badge(state: &str, locale: Locale) -> Badge {
    let texts = &locale.texts().runners;
    match state {
        "running" => (texts.state_running, Tone::Ok),
        "restarting" => (texts.state_restarting, Tone::Warn),
        "paused" => (texts.state_paused, Tone::Warn),
        "created" => (texts.state_created, Tone::Accent),
        // exited, dead, removing: the container does not run
        _ => (texts.state_exited, Tone::Accent),
    }
}

/// The containers grouped by compose project, the loose ones last.
fn projects_of(containers: &[Container], locale: Locale) -> Vec<ProjectGroup> {
    let mut groups: Vec<ProjectGroup> = Vec::new();
    let mut loose = Vec::new();
    for container in containers {
        let row = ContainerRow {
            name: container.name.clone(),
            image: container.image.clone(),
            status: container.status.clone(),
            state: state_badge(&container.state, locale),
        };
        match &container.project {
            Some(project) => match groups.iter_mut().find(|group| group.title == *project) {
                Some(group) => group.rows.push(row),
                None => groups.push(ProjectGroup {
                    title: project.clone(),
                    rows: vec![row],
                }),
            },
            None => loose.push(row),
        }
    }
    if !loose.is_empty() {
        groups.push(ProjectGroup {
            title: locale.texts().runners.no_project.to_owned(),
            rows: loose,
        });
    }
    groups
}

pub fn runners_view(
    answer: Option<Result<&RunnersOverview, &'static str>>,
    locale: Locale,
) -> RunnersView {
    let overview = match answer {
        None => return RunnersView::Loading,
        Some(Err(message)) => return RunnersView::Error(message),
        Some(Ok(overview)) => overview,
    };
    let texts = &locale.texts().runners;
    let running = overview
        .containers
        .iter()
        .filter(|container| container.state == "running")
        .count();
    let (docker_label, docker_tone) = match overview.docker {
        DockerState::Running => (
            (texts.docker_running)(running, overview.containers.len()),
            Tone::Ok,
        ),
        DockerState::Stopped => (texts.docker_stopped.to_owned(), Tone::Warn),
        DockerState::Missing => (texts.docker_missing.to_owned(), Tone::Accent),
    };
    RunnersView::Ready(RunnersReady {
        runners: overview
            .runners
            .iter()
            .map(|runner| runner_row(runner, locale))
            .collect(),
        github_note: match overview.github {
            GithubAccess::Ok => None,
            GithubAccess::NoGh => Some(texts.no_gh),
            GithubAccess::Partial => Some(texts.partial),
        },
        docker_label,
        docker_tone,
        projects: projects_of(&overview.containers, locale),
        running,
        is_docker_running: overview.docker == DockerState::Running,
    })
}

/// What the switch does now, or what the last one did.
pub fn mode_line(status: &ModeStatus, locale: Locale) -> Option<(String, Tone)> {
    let texts = &locale.texts().runners;
    if let Some(mode) = status.switching {
        let text = match mode {
            Mode::Gaming => texts.switching_gaming,
            Mode::Coding => texts.switching_coding,
        };
        return Some((text.to_owned(), Tone::Accent));
    }
    let last = status.last?;
    Some(match (last.mode, last.is_done) {
        (Mode::Gaming, true) => (texts.gaming_done.to_owned(), Tone::Ok),
        (Mode::Gaming, false) => (texts.gaming_failed.to_owned(), Tone::Warn),
        (Mode::Coding, true) => ((texts.coding_done)(last.running), Tone::Ok),
        (Mode::Coding, false) => (texts.coding_failed.to_owned(), Tone::Warn),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use vitals_ipc::ModeResult;

    use super::*;

    fn runner(
        name: &str,
        status: Option<RunnerStatus>,
        is_busy: bool,
        is_listening: Option<bool>,
    ) -> Runner {
        Runner {
            name: name.to_owned(),
            repo: "Codexup5/kit".to_owned(),
            status,
            is_busy,
            labels: ["self-hosted", "X64", "Linux", "kit-linux"]
                .map(str::to_owned)
                .to_vec(),
            folder: is_listening.map(|_| PathBuf::from(r"C:\actions-runner")),
            is_listening,
        }
    }

    fn container(name: &str, state: &str, project: Option<&str>) -> Container {
        Container {
            name: name.to_owned(),
            image: "image".to_owned(),
            state: state.to_owned(),
            status: "Up".to_owned(),
            project: project.map(str::to_owned),
        }
    }

    fn overview() -> RunnersOverview {
        RunnersOverview {
            runners: vec![
                runner("busy", Some(RunnerStatus::Online), true, None),
                runner("local", Some(RunnerStatus::Offline), false, Some(false)),
                runner("unknown", None, false, Some(true)),
            ],
            github: GithubAccess::Partial,
            docker: DockerState::Running,
            containers: vec![
                container("loose", "exited", None),
                container("ollama", "running", Some("kit-search")),
                container("milvus", "running", Some("kit-search")),
            ],
        }
    }

    #[test]
    fn a_runner_row_says_github_and_this_pc_apart() {
        let RunnersView::Ready(ready) = runners_view(Some(Ok(&overview())), Locale::En) else {
            panic!("ready");
        };
        let badges: Vec<(&str, Option<&str>)> = ready
            .runners
            .iter()
            .map(|row| (row.github.0, row.local.map(|local| local.0)))
            .collect();
        assert_eq!(
            badges,
            [
                ("Running a job", None),
                ("Offline", Some("Not running on this PC")),
                ("Status unknown", Some("Running on this PC")),
            ]
        );
        assert_eq!(ready.runners[0].labels, "kit-linux");
        assert_eq!(ready.github_note, Some(Locale::En.texts().runners.partial));
    }

    #[test]
    fn containers_group_by_project_with_loose_ones_last() {
        let RunnersView::Ready(ready) = runners_view(Some(Ok(&overview())), Locale::En) else {
            panic!("ready");
        };
        let groups: Vec<(&str, usize)> = ready
            .projects
            .iter()
            .map(|group| (group.title.as_str(), group.rows.len()))
            .collect();
        assert_eq!(groups, [("kit-search", 2), ("No project", 1)]);
        assert_eq!(ready.docker_label, "Docker runs · 2 of 3 containers up");
        assert_eq!(ready.projects[1].rows[0].state.0, "Stopped");
    }

    #[test]
    fn the_mode_line_tells_the_switch_and_its_result() {
        let switching = ModeStatus {
            switching: Some(Mode::Gaming),
            last: None,
        };
        assert_eq!(
            mode_line(&switching, Locale::En)
                .map(|line| line.0)
                .as_deref(),
            Some("Stopping Docker and WSL…")
        );
        let done = ModeStatus {
            switching: None,
            last: Some(ModeResult {
                mode: Mode::Coding,
                is_done: true,
                running: 6,
                finished_at: 0,
            }),
        };
        assert_eq!(
            mode_line(&done, Locale::En),
            Some((
                "Coding mode: Docker runs, 6 containers up.".to_owned(),
                Tone::Ok
            ))
        );
        assert_eq!(mode_line(&ModeStatus::default(), Locale::En), None);
    }
}
