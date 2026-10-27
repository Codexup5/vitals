//! Deleting what a preview listed, under the invariants of the brief, §3: every path stays inside its root after
//! canonicalisation, a link is deleted as itself, a file changed since the preview is skipped, a busy one too.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::guard;
use super::snapshot::{Entry, entry_of};

/// What a target deletes: the entries of a preview under their root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// The canonical root the guard allowed
    pub root: PathBuf,
    /// The root itself goes too once empty (a `node_modules`); a cache or temp folder stays
    pub removes_root: bool,
    pub entries: Vec<Entry>,
}

pub use vitals_ipc::CleanupTally as Tally;

/// `ERROR_SHARING_VIOLATION`, `ERROR_LOCK_VIOLATION`, `ERROR_ACCESS_DENIED`: a program holds the file, or it is
/// read-only or protected; the cleanup leaves it and goes on
fn is_busy(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32 | 33 | 5)) || error.kind() == ErrorKind::PermissionDenied
}

/// The entry is inside the root by its path and by where its folder really is: a folder swapped for a junction
/// after the preview points elsewhere and is refused.
fn is_inside_root(entry: &Path, root: &Path, forbidden: &[PathBuf]) -> bool {
    if !guard::is_plain(entry)
        || !guard::is_within(entry, root)
        || guard::is_forbidden(entry, forbidden)
    {
        return false;
    }
    let Some(parent) = entry.parent() else {
        return false;
    };
    std::fs::canonicalize(parent)
        .is_ok_and(|real| guard::is_within(&real, root) && !guard::is_forbidden(&real, forbidden))
}

/// A junction reads as a link, not a directory, through `Metadata::is_dir`; its attributes still say directory
#[cfg(windows)]
fn is_directory(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0
}

#[cfg(not(windows))]
fn is_directory(metadata: &std::fs::Metadata) -> bool {
    metadata.is_dir()
}

fn delete_entry(entry: &Entry, root: &Path, forbidden: &[PathBuf], tally: &mut Tally) {
    if !is_inside_root(&entry.path, root, forbidden) {
        tally.refused += 1;
        return;
    }
    let Ok(metadata) = std::fs::symlink_metadata(&entry.path) else {
        tally.changed += 1;
        return;
    };
    if entry_of(&entry.path, &metadata) != *entry {
        tally.changed += 1;
        return;
    }
    // A junction or a directory symlink is removed as a directory entry: its target is never entered
    let removed = if entry.is_link && is_directory(&metadata) {
        std::fs::remove_dir(&entry.path)
    } else {
        std::fs::remove_file(&entry.path)
    };
    match removed {
        Ok(()) => {
            tally.deleted += 1;
            tally.freed += entry.size;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => tally.changed += 1,
        Err(error) if is_busy(&error) => tally.busy += 1,
        Err(error) => {
            tracing::warn!("could not delete {}: {error}", entry.path.display());
            tally.busy += 1;
        }
    }
}

/// Removes the folders the deletion emptied, deepest first, never through a link and never above the root.
fn remove_empty_folders(root: &Path, removes_root: bool) {
    let mut folders = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(listing) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in listing.flatten() {
            if item
                .metadata()
                .is_ok_and(|metadata| metadata.is_dir() && !crate::scan::is_link(&metadata))
            {
                stack.push(item.path());
                folders.push(item.path());
            }
        }
    }
    folders.sort_by_key(|folder| std::cmp::Reverse(folder.components().count()));
    for folder in folders {
        // Fails on a folder that still holds something: that is the point
        let _ = std::fs::remove_dir(folder);
    }
    if removes_root {
        let _ = std::fs::remove_dir(root);
    }
}

/// Deletes the entries of a target; `forbidden` is checked again here, whatever the preview did.
pub fn run(target: &Target, forbidden: &[PathBuf]) -> Tally {
    let mut tally = Tally::default();
    if guard::is_forbidden(&target.root, forbidden) {
        tally.refused = u64::try_from(target.entries.len()).unwrap_or(u64::MAX);
        return tally;
    }
    for entry in &target.entries {
        delete_entry(entry, &target.root, forbidden, &mut tally);
    }
    remove_empty_folders(&target.root, target.removes_root);
    tally
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use super::super::snapshot::{Filter, take};
    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn target(root: &Path, removes_root: bool) -> Target {
        let root = fs::canonicalize(root).unwrap();
        Target {
            entries: take(&root, Filter::All),
            root,
            removes_root,
        }
    }

    #[test]
    fn deletes_the_snapshot_and_the_folders_it_emptied() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join(".turbo");
        write(&cache.join("a/log.txt"), 100);
        write(&cache.join("b/c/run.json"), 50);
        let tally = run(&target(&cache, false), &[]);
        assert_eq!((tally.deleted, tally.freed), (2, 150));
        assert!(cache.is_dir(), "a cache folder itself stays");
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn a_file_that_appeared_after_the_preview_stays() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("old.tmp"), 10);
        let plan = target(dir.path(), false);
        write(&dir.path().join("new.tmp"), 10);
        let tally = run(&plan, &[]);
        assert_eq!(tally.deleted, 1);
        assert!(dir.path().join("new.tmp").exists());
    }

    #[test]
    fn a_file_changed_since_the_preview_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("log.txt");
        write(&file, 10);
        let plan = target(dir.path(), false);
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&file, vec![1u8; 30]).unwrap();
        let tally = run(&plan, &[]);
        assert_eq!((tally.deleted, tally.changed), (0, 1));
        assert!(file.exists());
    }

    #[test]
    fn an_entry_outside_its_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        let outside = dir.path().join("documents/report.docx");
        write(&root.join("x.bin"), 1);
        write(&outside, 5);
        let mut plan = target(&root, false);
        let metadata = fs::symlink_metadata(&outside).unwrap();
        plan.entries
            .push(entry_of(&fs::canonicalize(&outside).unwrap(), &metadata));
        let mut climbing = entry_of(&outside, &metadata);
        climbing.path = plan.root.join("..").join("documents/report.docx");
        plan.entries.push(climbing);
        let tally = run(&plan, &[]);
        assert_eq!((tally.deleted, tally.refused), (1, 2));
        assert!(outside.exists());
    }

    #[test]
    fn the_never_list_is_checked_again_at_the_run() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("claude/session.json"), 5);
        let plan = target(dir.path(), false);
        let never = vec![fs::canonicalize(dir.path().join("claude")).unwrap()];
        let tally = run(&plan, &never);
        assert_eq!((tally.deleted, tally.refused), (0, 1));
        assert!(dir.path().join("claude/session.json").exists());
    }

    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed: {status:?}");
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_is_deleted_as_itself_and_its_target_survives() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        write(&store.join("big.bin"), 10_000);
        let modules = dir.path().join("project").join("node_modules");
        write(&modules.join("own/index.js"), 20);
        junction(&modules.join("react"), &store);
        let plan = target(&modules, true);
        assert_eq!(plan.entries.iter().filter(|entry| entry.is_link).count(), 1);
        let tally = run(&plan, &[]);
        assert_eq!((tally.deleted, tally.freed), (2, 20));
        assert!(!modules.exists(), "node_modules goes once empty");
        assert_eq!(fs::read(store.join("big.bin")).unwrap().len(), 10_000);
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_swapped_for_a_junction_after_the_preview_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        write(&root.join("sub/file.bin"), 7);
        let elsewhere = dir.path().join("elsewhere");
        write(&elsewhere.join("file.bin"), 7);
        let plan = target(&root, false);
        fs::remove_dir_all(root.join("sub")).unwrap();
        junction(&root.join("sub"), &elsewhere);
        let tally = run(&plan, &[]);
        assert_eq!(tally.deleted, 0);
        assert_eq!(tally.refused, 1);
        assert!(elsewhere.join("file.bin").exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_busy_file_is_skipped_and_the_rest_goes() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("busy.log"), 10);
        write(&dir.path().join("free.log"), 10);
        let plan = target(dir.path(), false);
        // No sharing at all: Windows refuses to delete the file while it is open
        let _held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(dir.path().join("busy.log"))
            .unwrap();
        let tally = run(&plan, &[]);
        assert_eq!((tally.deleted, tally.busy), (1, 1));
        assert!(dir.path().join("busy.log").exists());
    }

    #[test]
    fn an_old_filter_keeps_fresh_files() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("fresh.tmp"), 3);
        let root = fs::canonicalize(dir.path()).unwrap();
        let week_ago = SystemTime::now() - Duration::from_secs(7 * 24 * 60 * 60);
        let plan = Target {
            entries: take(
                &root,
                Filter::OlderThan {
                    cutoff: week_ago,
                    skip_names: &[],
                },
            ),
            root,
            removes_root: false,
        };
        assert_eq!(run(&plan, &[]).deleted, 0);
        assert!(dir.path().join("fresh.tmp").exists());
    }
}
