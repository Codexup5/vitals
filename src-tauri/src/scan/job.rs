//! One scan of the home at a time, in the background: the treemap tiles and the cleanup preview it produces.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::category::{Category, category_of};
use super::{Node, Progress, WalkOptions, walk};
use crate::cleanup::{Candidate, Places, find_candidates};

/// home → `AppData` → `Local` → a program folder is the deepest level a tile comes from
const KEEP_DEPTH: usize = 3;
const MAX_TILES: usize = 80;
/// Folders shown by their content rather than as one tile: the program data folders
const EXPANDED: &[&str] = &[
    "appdata",
    "appdata/local",
    "appdata/roaming",
    "appdata/locallow",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub path: PathBuf,
    /// The path from the home, `AppData\Local\pnpm`
    pub label: String,
    pub bytes: u64,
    pub category: Category,
    /// Several small folders of one category shown as one tile
    pub is_rest: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub finished_at: i64,
    pub home: PathBuf,
    pub total_bytes: u64,
    pub files: u64,
    pub links: u64,
    pub unreadable: u64,
    pub tiles: Vec<Tile>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Default)]
pub struct ScanJob {
    pub progress: Arc<Progress>,
    pub is_running: AtomicBool,
}

fn relative_key(path: &Path, home: &Path) -> String {
    path.strip_prefix(home)
        .map(|relative| relative.to_string_lossy().replace('\\', "/").to_lowercase())
        .unwrap_or_default()
}

fn label_of(path: &Path, home: &Path) -> String {
    path.strip_prefix(home).map_or_else(
        |_| path.display().to_string(),
        |relative| relative.display().to_string(),
    )
}

/// Leaves of the kept tree: a folder becomes a tile unless it is a program data folder or holds a code root.
fn collect_tiles(node: &Node, home: &Path, code_roots: &[&Path], tiles: &mut Vec<Tile>) {
    let key = relative_key(&node.path, home);
    let holds_code_root = code_roots
        .iter()
        .any(|root| root.starts_with(&node.path) && root != &node.path.as_path());
    let is_code_root = code_roots.iter().any(|root| root == &node.path.as_path());
    let is_expanded =
        node.path == home || EXPANDED.contains(&key.as_str()) || holds_code_root || is_code_root;
    if !is_expanded || node.children.is_empty() {
        if node.bytes > 0 {
            tiles.push(Tile {
                path: node.path.clone(),
                label: label_of(&node.path, home),
                bytes: node.bytes,
                category: category_of(&node.path, home, code_roots),
                is_rest: false,
            });
        }
        return;
    }
    for child in &node.children {
        collect_tiles(child, home, code_roots, tiles);
    }
    let in_children: u64 = node.children.iter().map(|child| child.bytes).sum();
    let own_files = node.bytes.saturating_sub(in_children);
    if own_files > 0 {
        tiles.push(Tile {
            path: node.path.clone(),
            label: label_of(&node.path, home),
            bytes: own_files,
            category: category_of(&node.path, home, code_roots),
            is_rest: true,
        });
    }
}

/// The biggest tiles as they are; the rest folded into one tile per category, so the treemap stays readable.
pub fn tiles_of(root: &Node, home: &Path, code_roots: &[&Path]) -> Vec<Tile> {
    let mut tiles = Vec::new();
    collect_tiles(root, home, code_roots, &mut tiles);
    tiles.sort_by_key(|tile| std::cmp::Reverse(tile.bytes));
    if tiles.len() <= MAX_TILES {
        return tiles;
    }
    let rest = tiles.split_off(MAX_TILES - 1);
    let mut folded: Vec<Tile> = Vec::new();
    for tile in rest {
        match folded
            .iter_mut()
            .find(|rest_tile| rest_tile.category == tile.category)
        {
            Some(rest_tile) => rest_tile.bytes += tile.bytes,
            None => folded.push(Tile {
                path: home.to_path_buf(),
                label: String::new(),
                is_rest: true,
                ..tile
            }),
        }
    }
    tiles.extend(folded);
    tiles.sort_by_key(|tile| std::cmp::Reverse(tile.bytes));
    tiles
}

impl ScanJob {
    /// Starts a scan unless one runs; `on_done` receives the result on the scan thread.
    pub fn start(
        self: &Arc<Self>,
        home: PathBuf,
        places: Places,
        on_done: impl FnOnce(ScanResult) + Send + 'static,
    ) -> bool {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.progress.files.store(0, Ordering::Relaxed);
        self.progress.bytes.store(0, Ordering::Relaxed);
        self.progress.cancelled.store(false, Ordering::Relaxed);
        let job = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("scan".to_owned())
            .spawn(move || {
                let code_roots: Vec<&Path> =
                    places.code_roots.iter().map(PathBuf::as_path).collect();
                let root = walk(
                    &home,
                    WalkOptions {
                        keep_depth: KEEP_DEPTH,
                        skip_names: &[],
                    },
                    &job.progress,
                );
                let tiles = tiles_of(&root, &home, &code_roots);
                let candidates = find_candidates(&places, SystemTime::now(), &job.progress);
                let result = ScanResult {
                    finished_at: chrono::Utc::now().timestamp(),
                    home: home.clone(),
                    total_bytes: root.bytes,
                    files: root.files,
                    links: root.links,
                    unreadable: root.unreadable,
                    tiles,
                    candidates,
                };
                tracing::info!(
                    "scanned {}: {} files, {} bytes",
                    home.display(),
                    result.files,
                    result.total_bytes
                );
                on_done(result);
                job.is_running.store(false, Ordering::SeqCst);
            });
        if let Err(error) = spawned {
            tracing::error!("the scan thread did not start: {error}");
            self.is_running.store(false, Ordering::SeqCst);
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, bytes: u64, children: Vec<Node>) -> Node {
        Node {
            path: PathBuf::from(path),
            bytes,
            files: 1,
            links: 0,
            unreadable: 0,
            newest: None,
            children,
        }
    }

    #[test]
    fn tiles_open_program_data_and_the_code_root_and_keep_loose_files() {
        let home = PathBuf::from(r"C:\Users\someone");
        let code = PathBuf::from(r"C:\Users\someone\Desktop\Code");
        let tree = node(
            r"C:\Users\someone",
            1000,
            vec![
                node(
                    r"C:\Users\someone\AppData",
                    500,
                    vec![node(
                        r"C:\Users\someone\AppData\Local",
                        500,
                        vec![node(r"C:\Users\someone\AppData\Local\pnpm", 500, vec![])],
                    )],
                ),
                node(
                    r"C:\Users\someone\Desktop",
                    400,
                    vec![node(
                        r"C:\Users\someone\Desktop\Code",
                        300,
                        vec![node(r"C:\Users\someone\Desktop\Code\kit", 300, vec![])],
                    )],
                ),
            ],
        );
        let tiles = tiles_of(&tree, &home, &[code.as_path()]);
        let labels: Vec<(&str, u64, bool)> = tiles
            .iter()
            .map(|tile| (tile.label.as_str(), tile.bytes, tile.is_rest))
            .collect();
        assert_eq!(
            labels,
            [
                (r"AppData\Local\pnpm", 500, false),
                (r"Desktop\Code\kit", 300, false),
                ("Desktop", 100, true),
                ("", 100, true)
            ]
        );
        assert_eq!(tiles[0].category, Category::PackageCaches);
        assert_eq!(tiles[1].category, Category::CodeProjects);
        // The tiles add up to the home: nothing counted twice, nothing lost
        assert_eq!(tiles.iter().map(|tile| tile.bytes).sum::<u64>(), 1000);
    }
}
