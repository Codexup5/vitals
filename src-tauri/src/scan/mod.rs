//! Walks a folder tree for its sizes. Links and junctions are counted as themselves and never followed:
//! a pnpm `node_modules` is a junction onto the shared store, and following it counted 16 GB instead of ~1 GB.

pub mod category;
pub mod job;

use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

use serde::Serialize;

/// Live numbers of a running scan, read by the UI while the walk goes on.
#[derive(Debug, Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    pub cancelled: AtomicBool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub path: PathBuf,
    pub bytes: u64,
    pub files: u64,
    /// Links and junctions met and not followed
    pub links: u64,
    /// Folders the walk could not open (access denied, gone meanwhile)
    pub unreadable: u64,
    pub newest: Option<i64>,
    pub children: Vec<Node>,
}

#[cfg(windows)]
pub fn is_link(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.file_type().is_symlink()
}

#[cfg(not(windows))]
pub fn is_link(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn seconds(time: SystemTime) -> Option<i64> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
}

/// Options of one walk.
#[derive(Debug, Clone, Copy)]
pub struct WalkOptions<'a> {
    /// Children kept in the result down to this depth (0 keeps none)
    pub keep_depth: usize,
    /// Folder names not entered at all (counted as nothing)
    pub skip_names: &'a [&'a str],
}

/// The size of `dir`: files counted by their own length, links never entered, unreadable folders counted and skipped.
pub fn walk(dir: &Path, options: WalkOptions<'_>, progress: &Progress) -> Node {
    walk_at(dir, 0, options, progress)
}

fn walk_at(dir: &Path, depth: usize, options: WalkOptions<'_>, progress: &Progress) -> Node {
    let mut node = Node {
        path: dir.to_path_buf(),
        bytes: 0,
        files: 0,
        links: 0,
        unreadable: 0,
        newest: None,
        children: Vec::new(),
    };
    if progress.cancelled.load(Ordering::Relaxed) {
        return node;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        node.unreadable = 1;
        return node;
    };
    for entry in entries.flatten() {
        // DirEntry metadata on Windows comes from the directory listing: it describes the entry, never a link target
        let Ok(metadata) = entry.metadata() else {
            node.unreadable += 1;
            continue;
        };
        if is_link(&metadata) {
            node.links += 1;
            continue;
        }
        if metadata.is_dir() {
            let name = entry.file_name();
            if options
                .skip_names
                .iter()
                .any(|skip| name.eq_ignore_ascii_case(skip))
            {
                continue;
            }
            let child = walk_at(&entry.path(), depth + 1, options, progress);
            node.bytes += child.bytes;
            node.files += child.files;
            node.links += child.links;
            node.unreadable += child.unreadable;
            node.newest = node.newest.max(child.newest);
            if depth < options.keep_depth {
                node.children.push(child);
            }
        } else {
            node.bytes += metadata.len();
            node.files += 1;
            node.newest = node.newest.max(metadata.modified().ok().and_then(seconds));
            progress.files.fetch_add(1, Ordering::Relaxed);
            progress.bytes.fetch_add(metadata.len(), Ordering::Relaxed);
        }
    }
    node.children
        .sort_by_key(|child| std::cmp::Reverse(child.bytes));
    node
}

/// Files directly or deeply under `dir` last written before `cutoff`, with their total size; links never entered.
pub fn older_than(dir: &Path, cutoff: SystemTime, skip_names: &[&str]) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if is_link(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if !skip_names
                    .iter()
                    .any(|skip| entry.file_name().eq_ignore_ascii_case(skip))
                {
                    stack.push(entry.path());
                }
            } else if metadata.modified().is_ok_and(|time| time < cutoff) {
                bytes += metadata.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn options() -> WalkOptions<'static> {
        WalkOptions {
            keep_depth: 1,
            skip_names: &[],
        }
    }

    #[test]
    fn sums_files_and_keeps_children_by_size() {
        let root = tempfile::tempdir().unwrap();
        write(&root.path().join("a/one.bin"), 100);
        write(&root.path().join("a/deep/two.bin"), 50);
        write(&root.path().join("b/three.bin"), 300);
        let node = walk(root.path(), options(), &Progress::default());
        assert_eq!(node.bytes, 450);
        assert_eq!(node.files, 3);
        assert_eq!(
            node.children
                .iter()
                .map(|child| child.bytes)
                .collect::<Vec<_>>(),
            [300, 150]
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_is_counted_as_itself_and_never_entered() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("store");
        write(&store.join("big.bin"), 10_000);
        let project = root.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let link = project.join("node_modules");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&store)
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed: {status:?}");

        let node = walk(&project, options(), &Progress::default());
        assert_eq!(node.bytes, 0);
        assert_eq!(node.links, 1);
        // The store itself is still counted once, where it really lives
        assert_eq!(
            walk(root.path(), options(), &Progress::default()).bytes,
            10_000
        );
    }

    #[test]
    fn skips_named_folders_and_stops_when_cancelled() {
        let root = tempfile::tempdir().unwrap();
        write(&root.path().join("claude/session.json"), 70);
        write(&root.path().join("old.tmp"), 30);
        let skipping = WalkOptions {
            keep_depth: 0,
            skip_names: &["CLAUDE"],
        };
        assert_eq!(walk(root.path(), skipping, &Progress::default()).bytes, 30);
        let cancelled = Progress::default();
        cancelled.cancelled.store(true, Ordering::Relaxed);
        assert_eq!(walk(root.path(), options(), &cancelled).bytes, 0);
    }

    #[test]
    fn older_than_counts_only_old_files_outside_skipped_folders() {
        let root = tempfile::tempdir().unwrap();
        write(&root.path().join("fresh.tmp"), 10);
        write(&root.path().join("claude/old.json"), 20);
        let future = SystemTime::now() + std::time::Duration::from_secs(60);
        assert_eq!(older_than(root.path(), future, &["claude"]), (10, 1));
        assert_eq!(older_than(root.path(), SystemTime::UNIX_EPOCH, &[]), (0, 0));
    }
}
