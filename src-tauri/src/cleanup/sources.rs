//! Where each candidate of the brief, §3 lives, and how much it holds today.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{Candidate, CandidateKind, Group, docker};
use crate::scan::{Progress, WalkOptions, older_than, walk};

pub(super) const TEMP_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const STALE_PROJECT_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// `%TEMP%\claude` holds the Claude Code sessions: never a candidate
pub(super) const TEMP_KEEP: &[&str] = &["claude"];
/// Folders a project search never enters: they are not projects and are huge
const NOT_PROJECTS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    ".next",
    ".turbo",
    "dist",
    ".pnpm-store",
];
/// What a project's "last change" ignores: build output and installed packages
const NOT_PROJECT_CHANGES: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    ".next",
    ".turbo",
    "dist",
    "coverage",
];
const PROJECT_SEARCH_DEPTH: usize = 5;

/// The folders the preview reads; tests point them into a temporary folder.
#[derive(Debug, Clone)]
pub struct Places {
    pub home: PathBuf,
    pub temp: PathBuf,
    pub windows_temp: PathBuf,
    pub local_app_data: PathBuf,
    pub roaming_app_data: PathBuf,
    pub code_roots: Vec<PathBuf>,
    pub query_docker: bool,
}

impl Places {
    /// Every folder under one root, Docker not asked: the tests and the e2e fixture.
    pub fn under(root: &Path) -> Self {
        Self {
            home: root.join("home"),
            temp: root.join("temp"),
            windows_temp: root.join("windows-temp"),
            local_app_data: root.join("local"),
            roaming_app_data: root.join("roaming"),
            code_roots: vec![root.join("home").join("Desktop").join("Code")],
            query_docker: false,
        }
    }

    pub fn from_env(home: PathBuf, code_roots: Vec<PathBuf>) -> Option<Self> {
        let local_app_data = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
        let roaming_app_data = PathBuf::from(std::env::var_os("APPDATA")?);
        let windows = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        Some(Self {
            home,
            temp: std::env::temp_dir(),
            windows_temp: windows.join("Temp"),
            local_app_data,
            roaming_app_data,
            code_roots,
            query_docker: true,
        })
    }
}

fn size_of(path: &Path, progress: &Progress) -> u64 {
    walk(
        path,
        WalkOptions {
            keep_depth: 0,
            skip_names: &[],
        },
        progress,
    )
    .bytes
}

fn folder(
    group: Group,
    kind: CandidateKind,
    path: PathBuf,
    progress: &Progress,
) -> Option<Candidate> {
    path.is_dir().then(|| {
        let bytes = size_of(&path, progress);
        Candidate::new(group, kind, Some(path), bytes)
    })
}

/// Project folders (with `package.json`) under the code roots, without entering heavy folders.
fn projects(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = roots.iter().map(|root| (root.clone(), 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        if dir.join("package.json").is_file() {
            found.push(dir.clone());
        }
        if depth >= PROJECT_SEARCH_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let name = entry.file_name();
            let is_skipped = NOT_PROJECTS
                .iter()
                .any(|skip| name.eq_ignore_ascii_case(skip));
            if metadata.is_dir() && !crate::scan::is_link(&metadata) && !is_skipped {
                stack.push((entry.path(), depth + 1));
            }
        }
    }
    found.sort();
    found
}

fn code_candidates(places: &Places, now: SystemTime, progress: &Progress) -> Vec<Candidate> {
    let stale_before = now
        .checked_sub(STALE_PROJECT_AGE)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let stale_before = stale_before
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        });
    let mut candidates = Vec::new();
    for project in projects(&places.code_roots) {
        candidates.extend(folder(
            Group::Safe,
            CandidateKind::NextCache,
            project.join(".next").join("cache"),
            progress,
        ));
        candidates.extend(folder(
            Group::Safe,
            CandidateKind::Turbo,
            project.join(".turbo"),
            progress,
        ));
        let modules = project.join("node_modules");
        if !modules.is_dir() {
            continue;
        }
        let changes = walk(
            &project,
            WalkOptions {
                keep_depth: 0,
                skip_names: NOT_PROJECT_CHANGES,
            },
            progress,
        );
        if changes.newest.is_some_and(|newest| newest < stale_before) {
            let mut candidate = Candidate::new(
                Group::Review,
                CandidateKind::StaleNodeModules,
                Some(modules.clone()),
                size_of(&modules, progress),
            );
            // pnpm hard-links files from its store: deleting them frees less than they add up to
            candidate.is_upper_bound = true;
            candidate.last_modified = changes.newest;
            candidates.push(candidate);
        }
    }
    candidates
}

fn docker_candidates() -> Vec<Candidate> {
    let Some(usage) = docker::run(&["system", "df", "--format", "{{json .}}"]) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if let Some(images) = docker::run(&[
        "images",
        "--filter",
        "dangling=true",
        "--format",
        "{{.Size}}",
    ]) {
        let bytes = images.lines().filter_map(docker::parse_size).sum();
        candidates.push(Candidate::new(
            Group::Safe,
            CandidateKind::DockerDangling,
            None,
            bytes,
        ));
    }
    if let Some(bytes) = docker::reclaimable(&usage, "Build Cache") {
        candidates.push(Candidate::new(
            Group::Safe,
            CandidateKind::DockerBuildCache,
            None,
            bytes,
        ));
    }
    if let Some(bytes) = docker::reclaimable(&usage, "Images") {
        candidates.push(Candidate::new(
            Group::Review,
            CandidateKind::DockerUnusedImages,
            None,
            bytes,
        ));
    }
    candidates
}

fn vhdx_files(local_app_data: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = [
        local_app_data.join(r"Docker\wsl\disk\docker_data.vhdx"),
        local_app_data.join(r"Docker\wsl\data\ext4.vhdx"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect();
    if let Ok(packages) = std::fs::read_dir(local_app_data.join("Packages")) {
        files.extend(
            packages
                .flatten()
                .map(|package| package.path().join(r"LocalState\ext4.vhdx"))
                .filter(|path| path.is_file()),
        );
    }
    files
}

/// Every candidate of the brief, §3 found on this PC, with what it holds now. Reads only.
pub fn find_candidates(places: &Places, now: SystemTime, progress: &Progress) -> Vec<Candidate> {
    let week_ago = now.checked_sub(TEMP_AGE).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut candidates = Vec::new();

    let (bytes, _) = older_than(&places.temp, week_ago, TEMP_KEEP);
    candidates.push(Candidate::new(
        Group::Safe,
        CandidateKind::UserTemp,
        Some(places.temp.clone()),
        bytes,
    ));

    let mut windows_temp = Candidate::new(
        Group::Safe,
        CandidateKind::WindowsTemp,
        Some(places.windows_temp.clone()),
        0,
    );
    windows_temp.needs_admin = true;
    if std::fs::read_dir(&places.windows_temp).is_ok() {
        windows_temp.bytes = older_than(&places.windows_temp, week_ago, &[]).0;
    }
    candidates.push(windows_temp);

    for (kind, relative) in [
        (CandidateKind::PnpmStore, r"pnpm\store"),
        (CandidateKind::NpmCache, "npm-cache"),
    ] {
        if let Some(mut candidate) = folder(
            Group::Safe,
            kind,
            places.local_app_data.join(relative),
            progress,
        ) {
            candidate.is_upper_bound = true;
            candidates.push(candidate);
        }
    }
    for relative in [r"NVIDIA\DXCache", r"NVIDIA\GLCache", "D3DSCache"] {
        candidates.extend(folder(
            Group::Safe,
            CandidateKind::ShaderCache,
            places.local_app_data.join(relative),
            progress,
        ));
    }
    candidates.extend(code_candidates(places, now, progress));
    if places.query_docker {
        candidates.extend(docker_candidates());
    }
    for path in vhdx_files(&places.local_app_data) {
        let bytes = std::fs::metadata(&path).map_or(0, |metadata| metadata.len());
        let mut candidate = Candidate::new(Group::Review, CandidateKind::Vhdx, Some(path), bytes);
        // Compacting frees only the empty space inside the disk image: how much is known only after it runs
        candidate.is_upper_bound = true;
        candidates.push(candidate);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn places(root: &Path) -> Places {
        Places::under(root)
    }

    #[test]
    fn finds_the_whitelist_and_keeps_claude_sessions_out() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        write(&places.temp.join("old.tmp"), 40);
        write(&places.temp.join("claude/session.json"), 1000);
        write(&places.local_app_data.join("pnpm/store/v10/file"), 500);
        write(&places.local_app_data.join("NVIDIA/DXCache/shader.bin"), 30);
        write(&root.path().join("home/Desktop/Code/app/package.json"), 2);
        write(
            &root
                .path()
                .join("home/Desktop/Code/app/.next/cache/page.bin"),
            70,
        );
        // Everything counts as old when "now" is far in the future
        let now = SystemTime::now() + Duration::from_secs(400 * 24 * 60 * 60);
        let candidates = find_candidates(&places, now, &Progress::default());

        let bytes_of = |kind: CandidateKind| {
            candidates
                .iter()
                .filter(|candidate| candidate.kind == kind)
                .map(|candidate| candidate.bytes)
                .sum::<u64>()
        };
        assert_eq!(bytes_of(CandidateKind::UserTemp), 40);
        assert_eq!(bytes_of(CandidateKind::PnpmStore), 500);
        assert_eq!(bytes_of(CandidateKind::ShaderCache), 30);
        assert_eq!(bytes_of(CandidateKind::NextCache), 70);
        let windows_temp = candidates
            .iter()
            .find(|candidate| candidate.kind == CandidateKind::WindowsTemp);
        assert!(windows_temp.is_some_and(|candidate| candidate.needs_admin));
    }

    #[test]
    fn node_modules_is_offered_only_in_a_project_untouched_for_a_month() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        write(&root.path().join("home/Desktop/Code/old/package.json"), 2);
        write(
            &root
                .path()
                .join("home/Desktop/Code/old/node_modules/lib/index.js"),
            900,
        );
        let in_two_months = SystemTime::now() + Duration::from_secs(60 * 24 * 60 * 60);
        let found = find_candidates(&places, in_two_months, &Progress::default());
        let stale: Vec<_> = found
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::StaleNodeModules)
            .collect();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].group, Group::Review);
        assert_eq!(stale[0].bytes, 900);

        let today = find_candidates(&places, SystemTime::now(), &Progress::default());
        assert!(
            today
                .iter()
                .all(|candidate| candidate.kind != CandidateKind::StaleNodeModules)
        );
    }

    #[test]
    fn nothing_outside_the_code_roots_is_a_project() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        write(&root.path().join("elsewhere/app/package.json"), 2);
        write(&root.path().join("elsewhere/app/.turbo/log"), 50);
        let candidates = find_candidates(&places, SystemTime::now(), &Progress::default());
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.kind != CandidateKind::Turbo)
        );
    }
}
