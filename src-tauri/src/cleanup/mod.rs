//! What could be cleaned and how much it holds — the preview of the brief, §3. Nothing here deletes anything:
//! the cleaning engine comes with stage 3b, behind the tests of every invariant of §3.

mod docker;
mod sources;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use sources::{Places, find_candidates};

/// "Safe" items are on the whitelist and may be checked all at once; "review" items are chosen one by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Safe,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateKind {
    /// `%TEMP%` files older than seven days, `%TEMP%\claude` excluded
    UserTemp,
    /// `C:\Windows\Temp` older than seven days: an admin action
    WindowsTemp,
    PnpmStore,
    NpmCache,
    DockerDangling,
    DockerBuildCache,
    NextCache,
    Turbo,
    ShaderCache,
    StaleNodeModules,
    DockerUnusedImages,
    Vhdx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub group: Group,
    pub kind: CandidateKind,
    pub path: Option<PathBuf>,
    pub bytes: u64,
    /// The size is what the folder holds; the command frees only part of it (pnpm prune, npm verify, hard links)
    pub is_upper_bound: bool,
    pub last_modified: Option<i64>,
    pub needs_admin: bool,
}

impl Candidate {
    fn new(group: Group, kind: CandidateKind, path: Option<PathBuf>, bytes: u64) -> Self {
        let id = match &path {
            Some(path) => format!("{kind:?}:{}", path.display()),
            None => format!("{kind:?}"),
        };
        Self {
            id,
            group,
            kind,
            path,
            bytes,
            is_upper_bound: false,
            last_modified: None,
            needs_admin: false,
        }
    }
}
