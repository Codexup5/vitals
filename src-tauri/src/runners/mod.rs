//! Self-hosted runners and Docker: the runners installed on this PC and what GitHub says of them, the containers,
//! and the two modes that stop or bring back the dev stack. Reading is pure here; the tools run in `source` and
//! `mode`.

pub mod mode;
pub mod source;

use std::path::{Path, PathBuf};

use serde::Deserialize;
pub use vitals_ipc::{Container, Runner, RunnerStatus};

/// A runner configured in a folder of this PC, as its `.runner` file names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRunner {
    pub name: String,
    pub repo: String,
    pub folder: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunnerFile {
    agent_name: String,
    git_hub_url: String,
}

/// `owner/name` of `https://github.com/owner/name`; an organisation runner (`https://github.com/owner`) has none.
pub fn repo_of(url: &str) -> Option<String> {
    let path = url
        .trim()
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    let is_part = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|letter| letter.is_ascii_alphanumeric() || "-_.".contains(letter))
    };
    (is_part(owner) && is_part(name) && parts.next().is_none()).then(|| format!("{owner}/{name}"))
}

/// The `.runner` file of a configured runner; the runner writes it with a byte-order mark.
pub fn local_runner(text: &str, folder: &Path) -> Option<LocalRunner> {
    let file: RunnerFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    Some(LocalRunner {
        name: file.agent_name,
        repo: repo_of(&file.git_hub_url)?,
        folder: folder.to_path_buf(),
    })
}

#[derive(Deserialize)]
struct RunnersPage {
    runners: Vec<GithubRunner>,
}

#[derive(Deserialize)]
struct GithubRunner {
    name: String,
    status: String,
    busy: bool,
    labels: Vec<GithubLabel>,
}

#[derive(Deserialize)]
struct GithubLabel {
    name: String,
}

/// The runners of one repository from `gh api repos/<repo>/actions/runners`.
pub fn github_runners(json: &str, repo: &str) -> Option<Vec<Runner>> {
    let page: RunnersPage = serde_json::from_str(json).ok()?;
    Some(
        page.runners
            .into_iter()
            .map(|runner| Runner {
                name: runner.name,
                repo: repo.to_owned(),
                status: Some(if runner.status == "online" {
                    RunnerStatus::Online
                } else {
                    RunnerStatus::Offline
                }),
                is_busy: runner.busy,
                labels: runner.labels.into_iter().map(|label| label.name).collect(),
                folder: None,
                is_listening: None,
            })
            .collect(),
    )
}

/// One row per runner: GitHub's list, with the folder and the listener of the ones installed here; a local runner
/// GitHub did not list (no `gh`, or it was removed there) keeps a row of its own.
pub fn merge(
    mut remote: Vec<Runner>,
    local: &[LocalRunner],
    is_listening: impl Fn(&Path) -> bool,
) -> Vec<Runner> {
    for runner in local {
        let listening = Some(is_listening(&runner.folder));
        match remote
            .iter_mut()
            .find(|row| row.name == runner.name && row.repo == runner.repo)
        {
            Some(row) => {
                row.folder = Some(runner.folder.clone());
                row.is_listening = listening;
            }
            None => remote.push(Runner {
                name: runner.name.clone(),
                repo: runner.repo.clone(),
                status: None,
                is_busy: false,
                labels: Vec::new(),
                folder: Some(runner.folder.clone()),
                is_listening: listening,
            }),
        }
    }
    remote.sort_by(|left, right| (&left.repo, &left.name).cmp(&(&right.repo, &right.name)));
    remote
}

/// A `Runner.Listener.exe` of the runner's own `bin` folder runs.
pub fn is_listening_in(folder: &Path, running: &[PathBuf]) -> bool {
    let listener = folder.join("bin").join("Runner.Listener.exe");
    running
        .iter()
        .any(|exe| exe.as_os_str().eq_ignore_ascii_case(listener.as_os_str()))
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DockerLine {
    names: String,
    image: String,
    state: String,
    status: String,
    #[serde(default)]
    labels: String,
}

/// The containers of `docker ps --all --format '{{json .}}'`, one JSON object a line.
pub fn containers_of(output: &str) -> Vec<Container> {
    let mut containers: Vec<Container> = output
        .lines()
        .filter_map(|line| serde_json::from_str::<DockerLine>(line.trim()).ok())
        .map(|line| Container {
            project: line
                .labels
                .split(',')
                .find_map(|label| label.strip_prefix("com.docker.compose.project="))
                .map(str::to_owned),
            name: line.names,
            image: line.image,
            state: line.state,
            status: line.status,
        })
        .collect();
    containers
        .sort_by(|left, right| (&left.project, &left.name).cmp(&(&right.project, &right.name)));
    containers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repository_url_reads_as_owner_and_name() {
        assert_eq!(
            repo_of("https://github.com/Codexup5/vitals").as_deref(),
            Some("Codexup5/vitals")
        );
        assert_eq!(repo_of("https://github.com/Codexup5"), None);
        assert_eq!(repo_of("https://github.com/a/b/c"), None);
        assert_eq!(repo_of("https://github.com/a/b;rm"), None);
        assert_eq!(repo_of("https://example.com/a/b"), None);
    }

    #[test]
    fn the_runner_file_reads_past_its_byte_order_mark() {
        let text = "\u{feff}{\n  \"agentId\": 2,\n  \"agentName\": \"OlegPC-vitals\",\n  \"gitHubUrl\": \"https://github.com/Codexup5/vitals\",\n  \"workFolder\": \"_work\"\n}";
        let runner = local_runner(text, Path::new(r"C:\actions-runner-vitals")).expect("reads");
        assert_eq!(runner.name, "OlegPC-vitals");
        assert_eq!(runner.repo, "Codexup5/vitals");
        assert_eq!(local_runner("not json", Path::new("C:\\x")), None);
    }

    const RUNNERS: &str = r#"{"total_count":2,"runners":[
        {"id":1,"name":"OlegPC-kit-linux-1","os":"Linux","status":"online","busy":true,
         "labels":[{"id":1,"name":"self-hosted","type":"read-only"},{"id":2,"name":"kit-linux","type":"custom"}]},
        {"id":2,"name":"OlegPC-kit-windows","os":"Windows","status":"offline","busy":false,"labels":[]}]}"#;

    #[test]
    fn github_rows_take_the_folder_and_listener_of_local_runners() {
        let remote = github_runners(RUNNERS, "Codexup5/kit").expect("reads");
        assert_eq!(remote[0].status, Some(RunnerStatus::Online));
        assert!(remote[0].is_busy);
        assert_eq!(remote[0].labels, ["self-hosted", "kit-linux"]);
        let local = [
            LocalRunner {
                name: "OlegPC-kit-windows".to_owned(),
                repo: "Codexup5/kit".to_owned(),
                folder: PathBuf::from(r"C:\actions-runner-svc"),
            },
            LocalRunner {
                name: "OlegPC-vitals".to_owned(),
                repo: "Codexup5/vitals".to_owned(),
                folder: PathBuf::from(r"C:\actions-runner-vitals"),
            },
        ];
        let running = [PathBuf::from(
            r"C:\actions-runner-vitals\bin\Runner.Listener.exe",
        )];
        let rows = merge(remote, &local, |folder| is_listening_in(folder, &running));
        let summary: Vec<(&str, Option<RunnerStatus>, Option<bool>)> = rows
            .iter()
            .map(|row| (row.name.as_str(), row.status, row.is_listening))
            .collect();
        assert_eq!(
            summary,
            [
                ("OlegPC-kit-linux-1", Some(RunnerStatus::Online), None),
                (
                    "OlegPC-kit-windows",
                    Some(RunnerStatus::Offline),
                    Some(false)
                ),
                ("OlegPC-vitals", None, Some(true)),
            ]
        );
    }

    #[test]
    fn containers_keep_their_compose_project() {
        let output = concat!(
            r#"{"Names":"kit-search-ollama-1","Image":"ollama/ollama:0.12.10","State":"running","Status":"Up 11 hours (healthy)","Labels":"com.docker.compose.config-hash=d0,com.docker.compose.project=kit-search,desktop.docker.io/ports.scheme=v2"}"#,
            "\n",
            r#"{"Names":"admiring_lichterman","Image":"mcr.microsoft.com/playwright/mcp:v0.0.83","State":"exited","Status":"Exited (0) 2 hours ago","Labels":""}"#,
            "\nnot json\n"
        );
        let containers = containers_of(output);
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].name, "admiring_lichterman");
        assert_eq!(containers[0].project, None);
        assert_eq!(containers[1].project.as_deref(), Some("kit-search"));
        assert_eq!(containers[1].status, "Up 11 hours (healthy)");
    }
}
