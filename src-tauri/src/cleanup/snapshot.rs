//! The list of what a cleanup will delete, taken at the preview. The run deletes exactly this list: a file that
//! appeared after the preview is not on it, and one that changed since is skipped (the brief, §3).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::scan::is_link;

/// One file or link as the preview saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
    /// Nanoseconds since the Unix epoch of the last write
    pub modified: Option<u128>,
    /// A symlink or a junction: deleted as itself, its target never entered
    pub is_link: bool,
}

/// Which files of a root the cleanup takes.
#[derive(Debug, Clone, Copy)]
pub enum Filter<'a> {
    /// Everything under the root
    All,
    /// Files written before the cutoff, outside folders of these names (directly under the root)
    OlderThan {
        cutoff: SystemTime,
        skip_names: &'a [&'a str],
    },
}

pub fn nanos(time: SystemTime) -> Option<u128> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

pub fn entry_of(path: &Path, metadata: &std::fs::Metadata) -> Entry {
    Entry {
        path: path.to_path_buf(),
        size: if is_link(metadata) { 0 } else { metadata.len() },
        modified: metadata.modified().ok().and_then(nanos),
        is_link: is_link(metadata),
    }
}

/// The files and links under `root` the filter takes; links are listed, never entered.
pub fn take(root: &Path, filter: Filter<'_>) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0_usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(listing) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in listing.flatten() {
            // DirEntry metadata on Windows comes from the listing: it describes the entry, never a link target
            let Ok(metadata) = item.metadata() else {
                continue;
            };
            let path = item.path();
            let is_old = |metadata: &std::fs::Metadata| match filter {
                Filter::All => true,
                Filter::OlderThan { cutoff, .. } => {
                    metadata.modified().is_ok_and(|time| time < cutoff)
                }
            };
            if is_link(&metadata) {
                if is_old(&metadata) {
                    entries.push(entry_of(&path, &metadata));
                }
            } else if metadata.is_dir() {
                let is_skipped = match filter {
                    Filter::OlderThan { skip_names, .. } if depth == 0 => skip_names
                        .iter()
                        .any(|name| item.file_name().eq_ignore_ascii_case(name)),
                    _ => false,
                };
                if !is_skipped {
                    stack.push((path, depth + 1));
                }
            } else if is_old(&metadata) {
                entries.push(entry_of(&path, &metadata));
            }
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn takes_old_files_and_leaves_the_skipped_folder() {
        let root = tempfile::tempdir().unwrap();
        write(&root.path().join("old.tmp"), 10);
        write(&root.path().join("deep/older.tmp"), 20);
        write(&root.path().join("claude/session.json"), 30);
        let future = SystemTime::now() + Duration::from_secs(60);
        let entries = take(
            root.path(),
            Filter::OlderThan {
                cutoff: future,
                skip_names: &["claude"],
            },
        );
        let sizes: Vec<u64> = entries.iter().map(|entry| entry.size).collect();
        assert_eq!(sizes, [20, 10]);
        let none = take(
            root.path(),
            Filter::OlderThan {
                cutoff: UNIX_EPOCH,
                skip_names: &[],
            },
        );
        assert_eq!(none.len(), 0);
    }
}
