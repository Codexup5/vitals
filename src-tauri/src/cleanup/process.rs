//! The tools that own a cache clean it themselves: `pnpm store prune`, `npm cache verify`, `docker … prune`. Each runs
//! with fixed arguments, no shell string built from data, and a deadline.

use std::path::Path;
use std::time::Duration;

use super::{CandidateKind, docker};
use crate::scan::{Progress, WalkOptions, walk};

/// A prune of a big store can take minutes; past this it is stopped and reported as failed
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Runs a program with a deadline; its standard output on success.
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    crate::process::output(program, args, timeout)
        .ok()
        .filter(|output| output.code == 0)
        .map(|output| output.stdout)
}

/// The program and arguments that clean a candidate kind, if a tool does it.
pub fn command_of(kind: CandidateKind) -> Option<(&'static str, &'static [&'static str])> {
    // pnpm and npm are .cmd scripts: only cmd starts them
    match kind {
        CandidateKind::PnpmStore => Some(("cmd", &["/D", "/C", "pnpm", "store", "prune"])),
        CandidateKind::NpmCache => Some(("cmd", &["/D", "/C", "npm", "cache", "verify"])),
        CandidateKind::DockerDangling => Some(("docker", &["image", "prune", "--force"])),
        CandidateKind::DockerBuildCache => Some(("docker", &["builder", "prune", "--force"])),
        CandidateKind::DockerUnusedImages => {
            Some(("docker", &["image", "prune", "--all", "--force"]))
        }
        _ => None,
    }
}

/// `Total reclaimed space: 1.2GB` from the output of a docker prune.
pub fn reclaimed(output: &str) -> Option<u64> {
    output
        .lines()
        .find_map(|line| line.trim().strip_prefix("Total reclaimed space:"))
        .and_then(docker::parse_size)
}

fn size_of(path: &Path) -> u64 {
    walk(
        path,
        WalkOptions {
            keep_depth: 0,
            skip_names: &[],
        },
        &Progress::default(),
    )
    .bytes
}

/// Runs the tool of a kind; the bytes it freed, measured on its folder or read from its own report.
pub fn clean(kind: CandidateKind, folder: Option<&Path>) -> Option<u64> {
    let (program, args) = command_of(kind)?;
    let before = folder.map(size_of);
    let output = run(program, args, COMMAND_TIMEOUT)?;
    match (before, folder) {
        (Some(before), Some(folder)) => Some(before.saturating_sub(size_of(folder))),
        _ => Some(reclaimed(&output).unwrap_or(0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tool_kinds_have_a_command() {
        assert!(command_of(CandidateKind::PnpmStore).is_some());
        assert!(command_of(CandidateKind::DockerUnusedImages).is_some());
        assert!(command_of(CandidateKind::UserTemp).is_none());
        assert!(command_of(CandidateKind::Vhdx).is_none());
    }

    #[test]
    fn reads_what_docker_reclaimed() {
        let output = "Deleted Images:\ndeleted: sha256:abc\n\nTotal reclaimed space: 1.5GB\n";
        assert_eq!(reclaimed(output), Some(1_500_000_000));
        assert_eq!(reclaimed("nothing"), None);
    }
}
