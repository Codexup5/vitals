//! What the disks screen shows: the counters, the volumes, the drives, the treemap of the home and the cleanup
//! preview, computed from the answers of the core.

use vitals_ipc::{
    Candidate, Category, DiskHealth, DisksOverview, Group, PhysicalDisk, ScanStatus, Tile, Volume,
};

use crate::format;
use crate::i18n::Locale;
use crate::ipc::IpcError;

pub const TREEMAP_WIDTH: f64 = 1000.0;
pub const TREEMAP_HEIGHT: f64 = 520.0;
const LABEL_MIN_WIDTH: f64 = 110.0;
const LABEL_MIN_HEIGHT: f64 = 44.0;
/// Below a megabyte a row is noise: an empty `.turbo` of every package of a monorepo
const WORTH_SHOWING: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The worst aspect ratio of a row laid along a side of this length (Bruls, Huizing, van Wijk).
fn worst_ratio(areas: &[f64], side: f64) -> f64 {
    let sum: f64 = areas.iter().sum();
    let largest = areas.iter().copied().fold(f64::MIN, f64::max);
    let smallest = areas.iter().copied().fold(f64::MAX, f64::min);
    f64::max(
        side * side * largest / (sum * sum),
        sum * sum / (side * side * smallest),
    )
}

fn place_row(row: &[(usize, f64)], rect: Rect, placed: &mut Vec<(usize, Rect)>) -> Rect {
    let sum: f64 = row.iter().map(|(_, area)| area).sum();
    if rect.width >= rect.height {
        let column = sum / rect.height;
        let mut y = rect.y;
        for (index, area) in row {
            let height = area / column;
            placed.push((
                *index,
                Rect {
                    x: rect.x,
                    y,
                    width: column,
                    height,
                },
            ));
            y += height;
        }
        Rect {
            x: rect.x + column,
            width: rect.width - column,
            ..rect
        }
    } else {
        let line = sum / rect.width;
        let mut x = rect.x;
        for (index, area) in row {
            let width = area / line;
            placed.push((
                *index,
                Rect {
                    x,
                    y: rect.y,
                    width,
                    height: line,
                },
            ));
            x += width;
        }
        Rect {
            y: rect.y + line,
            height: rect.height - line,
            ..rect
        }
    }
}

/// Squarified treemap: each value gets a rectangle whose area is its share of the whole, as square as the row
/// allows. Returns the index of each placed value with its rectangle; zero values get none.
#[allow(
    clippy::cast_precision_loss,
    reason = "sizes are shown as areas, not counted"
)]
pub fn squarify(values: &[u64], bounds: Rect) -> Vec<(usize, Rect)> {
    let total: u64 = values.iter().sum();
    if total == 0 {
        return Vec::new();
    }
    let scale = bounds.width * bounds.height / total as f64;
    let mut weighted: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter(|(_, value)| **value > 0)
        .map(|(index, value)| (index, *value as f64 * scale))
        .collect();
    weighted.sort_by(|left, right| right.1.total_cmp(&left.1));

    let mut placed = Vec::with_capacity(weighted.len());
    let mut rect = bounds;
    let mut row: Vec<(usize, f64)> = Vec::new();
    for entry in weighted {
        let side = rect.width.min(rect.height);
        let areas: Vec<f64> = row.iter().map(|(_, area)| *area).collect();
        let mut grown = areas.clone();
        grown.push(entry.1);
        if row.is_empty() || worst_ratio(&grown, side) <= worst_ratio(&areas, side) {
            row.push(entry);
        } else {
            rect = place_row(&row, rect, &mut placed);
            row = vec![entry];
        }
    }
    if !row.is_empty() {
        place_row(&row, rect, &mut placed);
    }
    placed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreemapMode {
    #[default]
    Category,
    Cleanup,
}

/// The fill of a tile: its category, or what cleaning it could do
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileFill {
    Category(Category),
    Safe,
    Review,
    Nothing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapTile {
    pub key: String,
    pub rect: Rect,
    pub fill: TileFill,
    pub title: String,
    pub label: Option<String>,
    pub size: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendEntry {
    pub fill: TileFill,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Treemap {
    pub tiles: Vec<TreemapTile>,
    pub legend: Vec<LegendEntry>,
}

fn is_inside(path: &str, folder: &str) -> bool {
    let child = path.to_lowercase();
    let parent = folder.to_lowercase();
    child == parent || child.starts_with(&format!("{parent}\\"))
}

/// A tile is "safe" when a whitelisted candidate lies inside it (or holds it), "review" for a review-only one.
pub fn cleanup_status(tile: &Tile, candidates: &[Candidate]) -> TileFill {
    if tile.is_rest {
        return TileFill::Nothing;
    }
    let tile_path = tile.path.to_string_lossy();
    let related: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| candidate.bytes > 0)
        .filter(|candidate| {
            candidate.path.as_ref().is_some_and(|path| {
                let path = path.to_string_lossy();
                is_inside(&path, &tile_path) || is_inside(&tile_path, &path)
            })
        })
        .collect();
    if related
        .iter()
        .any(|candidate| candidate.group == Group::Safe)
    {
        TileFill::Safe
    } else if related.is_empty() {
        TileFill::Nothing
    } else {
        TileFill::Review
    }
}

pub fn treemap(
    tiles: &[Tile],
    candidates: &[Candidate],
    mode: TreemapMode,
    locale: Locale,
) -> Treemap {
    let texts = &locale.texts().disks;
    let categories = &locale.texts().disk;
    let sizes: Vec<u64> = tiles.iter().map(|tile| tile.bytes).collect();
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: TREEMAP_WIDTH,
        height: TREEMAP_HEIGHT,
    };
    let placed = squarify(&sizes, bounds);

    let tiles = placed
        .into_iter()
        .filter_map(|(index, rect)| tiles.get(index).map(|tile| (tile, rect)))
        .map(|(tile, rect)| {
            let size = format::bytes(tile.bytes, locale);
            let category = categories.category(tile.category);
            let name = if tile.is_rest {
                let base = if tile.label.is_empty() {
                    category
                } else {
                    &tile.label
                };
                format!("{base} · {}", texts.rest)
            } else {
                tile.label.clone()
            };
            // The tile shows the folder name; the hover title carries the whole path from the home
            let label = if tile.is_rest {
                name.clone()
            } else {
                name.rsplit('\\').next().unwrap_or(&name).to_owned()
            };
            let is_labelled = rect.width >= LABEL_MIN_WIDTH && rect.height >= LABEL_MIN_HEIGHT;
            TreemapTile {
                key: format!(
                    "{}|{}|{:?}|{}",
                    tile.path.display(),
                    tile.label,
                    tile.category,
                    tile.is_rest
                ),
                rect,
                fill: match mode {
                    TreemapMode::Category => TileFill::Category(tile.category),
                    TreemapMode::Cleanup => cleanup_status(tile, candidates),
                },
                title: format!("{name} — {size} — {category}"),
                label: is_labelled.then_some(label),
                size,
            }
        })
        .collect();

    let legend = match mode {
        TreemapMode::Category => Category::ALL
            .into_iter()
            .map(|category| LegendEntry {
                fill: TileFill::Category(category),
                label: categories.category(category),
            })
            .collect(),
        TreemapMode::Cleanup => vec![
            LegendEntry {
                fill: TileFill::Safe,
                label: texts.tile_safe,
            },
            LegendEntry {
                fill: TileFill::Review,
                label: texts.tile_review,
            },
            LegendEntry {
                fill: TileFill::Nothing,
                label: texts.tile_none,
            },
        ],
    };
    Treemap { tiles, legend }
}

pub fn sum_of(candidates: &[Candidate], group: Group) -> u64 {
    candidates
        .iter()
        .filter(|candidate| candidate.group == group)
        .map(|candidate| candidate.bytes)
        .sum()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationRow {
    pub id: String,
    pub title: &'static str,
    pub hint: &'static str,
    pub path: Option<String>,
    pub size: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationGroup {
    pub group: Group,
    pub title: &'static str,
    pub hint: &'static str,
    pub total: String,
    pub rows: Vec<RecommendationRow>,
}

fn recommendation_row(candidate: &Candidate, locale: Locale) -> RecommendationRow {
    let texts = &locale.texts().disks;
    let (title, hint) = locale.texts().disk.candidate(candidate.kind);
    let size = format::bytes(candidate.bytes, locale);
    let notes = candidate
        .last_modified
        .and_then(format::local_time)
        .map(|time| (texts.changed)(&format::date(time.date(), locale)))
        .into_iter()
        .collect();
    RecommendationRow {
        id: candidate.id.clone(),
        title,
        hint,
        path: candidate
            .path
            .as_ref()
            .map(|path| path.display().to_string()),
        size: if candidate.is_upper_bound {
            (texts.up_to)(&size)
        } else {
            size
        },
        notes,
    }
}

/// Biggest first inside each group; a candidate with nothing in it is not worth a row.
pub fn recommendation_groups(candidates: &[Candidate], locale: Locale) -> Vec<RecommendationGroup> {
    let texts = &locale.texts().disks;
    [Group::Safe, Group::Review]
        .into_iter()
        .map(|group| {
            let mut chosen: Vec<&Candidate> = candidates
                .iter()
                .filter(|candidate| {
                    candidate.group == group
                        && (candidate.bytes >= WORTH_SHOWING || candidate.needs_admin)
                })
                .collect();
            chosen.sort_by_key(|candidate| std::cmp::Reverse(candidate.bytes));
            let (title, hint) = match group {
                Group::Safe => (texts.safe, texts.safe_hint),
                Group::Review => (texts.review, texts.review_hint),
            };
            RecommendationGroup {
                group,
                title,
                hint,
                total: (texts.total)(&format::bytes(sum_of(candidates, group), locale)),
                rows: chosen
                    .into_iter()
                    .map(|candidate| recommendation_row(candidate, locale))
                    .collect(),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kpi {
    pub key: &'static str,
    pub label: String,
    pub value: String,
    pub hint: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeRow {
    pub letter: String,
    pub name: String,
    pub used: String,
    pub free: String,
    pub used_percent: u8,
}

/// The tone of the health tag: the word always comes with it, the tone only repeats it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Ok,
    Warn,
    Danger,
}

impl Tone {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Accent => "accent",
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Danger => "danger",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveRow {
    pub key: String,
    pub name: String,
    pub health_tone: Tone,
    pub health: &'static str,
    pub temperature: String,
    pub wear: String,
    pub hours: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisksView {
    Loading,
    Error(&'static str),
    Ready {
        kpis: Vec<Kpi>,
        volumes: Vec<VolumeRow>,
        drives: Vec<DriveRow>,
        scan_label: String,
        has_scan: bool,
        recommendations: Vec<RecommendationGroup>,
    },
}

pub fn scan_label(
    overview: Option<&DisksOverview>,
    status: Option<&ScanStatus>,
    locale: Locale,
) -> String {
    let texts = &locale.texts().disks;
    if let Some(status) = status.filter(|status| status.is_running) {
        return (texts.scan_running)(
            &format::count(status.files, locale),
            &format::bytes(status.bytes, locale),
        );
    }
    let Some(scan) = overview.and_then(|overview| overview.scan.as_ref()) else {
        return texts.scan_never.to_owned();
    };
    let finished = format::local_time(scan.finished_at).map_or_else(
        || locale.texts().common.missing.to_owned(),
        |time| format::date_time(time, locale),
    );
    let last = (texts.scan_last)(&finished);
    if scan.links > 0 {
        format!(
            "{last} · {}",
            (texts.scan_links)(&format::count(scan.links, locale))
        )
    } else {
        last
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a share of a volume rounds to 0..=100"
)]
fn percent(part: u64, whole: u64) -> u8 {
    if whole == 0 {
        0
    } else {
        (part as f64 / whole as f64 * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }
}

fn volume_row(volume: &Volume, locale: Locale) -> VolumeRow {
    let texts = &locale.texts().disks;
    let used = volume.total_bytes.saturating_sub(volume.free_bytes);
    VolumeRow {
        letter: volume.letter.clone(),
        name: if volume.label.is_empty() {
            format!("{}:", volume.letter)
        } else {
            format!("{}: {}", volume.letter, volume.label)
        },
        used: (texts.volume_used)(
            &format::bytes(used, locale),
            &format::bytes(volume.total_bytes, locale),
        ),
        free: (texts.volume_free)(&format::bytes(volume.free_bytes, locale)),
        used_percent: percent(used, volume.total_bytes),
    }
}

fn drive_row(index: usize, drive: &PhysicalDisk, locale: Locale) -> DriveRow {
    let texts = &locale.texts().disks;
    let missing = locale.texts().common.missing;
    let (health_tone, health) = match drive.health {
        DiskHealth::Healthy => (Tone::Ok, texts.healthy),
        DiskHealth::Warning => (Tone::Warn, texts.warning),
        DiskHealth::Unhealthy => (Tone::Danger, texts.unhealthy),
        DiskHealth::Unknown => (Tone::Accent, texts.unknown),
    };
    DriveRow {
        key: format!("{}-{index}", drive.name),
        name: format!(
            "{} · {} · {}",
            drive.name,
            drive.media,
            format::bytes(drive.size_bytes, locale)
        ),
        health_tone,
        health,
        temperature: drive
            .temperature_c
            .map_or_else(|| missing.to_owned(), |value| format!("{value} °C")),
        wear: drive
            .wear_percent
            .map_or_else(|| missing.to_owned(), |value| format!("{value} %")),
        hours: drive.power_on_hours.map_or_else(
            || missing.to_owned(),
            |value| (texts.hours_value)(&format::count(value, locale)),
        ),
    }
}

pub fn disks_view(
    answer: Option<&Result<DisksOverview, IpcError>>,
    status: Option<&ScanStatus>,
    system_drive: &str,
    locale: Locale,
) -> DisksView {
    let overview = match answer {
        None => return DisksView::Loading,
        Some(Err(error)) => return DisksView::Error(error.text(locale)),
        Some(Ok(overview)) => overview,
    };
    let texts = &locale.texts().disks;
    let missing = locale.texts().common.missing;
    let system = overview
        .volumes
        .iter()
        .find(|volume| volume.letter == system_drive);
    let candidates = overview
        .scan
        .as_ref()
        .map_or(&[][..], |scan| &scan.candidates[..]);
    let safe = sum_of(candidates, Group::Safe);
    let review = sum_of(candidates, Group::Review);
    let bytes = |value: Option<u64>| {
        value.map_or_else(|| missing.to_owned(), |value| format::bytes(value, locale))
    };
    let scanned = overview.scan.as_ref().map(|scan| scan.total_bytes);
    let not_scanned = system
        .zip(scanned)
        .map(|(volume, scanned)| (volume.total_bytes - volume.free_bytes).saturating_sub(scanned));
    let has_scan = overview.scan.is_some();

    DisksView::Ready {
        kpis: vec![
            Kpi {
                key: "free",
                label: (texts.free_now)(&format!("{system_drive}:")),
                value: bytes(system.map(|volume| volume.free_bytes)),
                hint: None,
            },
            Kpi {
                key: "after",
                label: texts.free_after.to_owned(),
                value: bytes(
                    system
                        .filter(|_| has_scan)
                        .map(|volume| volume.free_bytes + safe),
                ),
                hint: None,
            },
            Kpi {
                key: "safe",
                label: texts.safe.to_owned(),
                value: bytes(has_scan.then_some(safe)),
                hint: None,
            },
            Kpi {
                key: "review",
                label: texts.review.to_owned(),
                value: bytes(has_scan.then_some(review)),
                hint: None,
            },
            Kpi {
                key: "notScanned",
                label: texts.not_scanned.to_owned(),
                value: bytes(not_scanned),
                hint: Some(texts.not_scanned_hint),
            },
        ],
        volumes: overview
            .volumes
            .iter()
            .map(|volume| volume_row(volume, locale))
            .collect(),
        drives: overview
            .physical_disks
            .iter()
            .enumerate()
            .map(|(index, drive)| drive_row(index, drive, locale))
            .collect(),
        scan_label: scan_label(Some(overview), status, locale),
        has_scan,
        recommendations: recommendation_groups(candidates, locale),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use vitals_ipc::{CandidateKind, ScanResult};

    use super::*;

    const BOUNDS: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 500.0,
    };

    fn overlaps(left: &Rect, right: &Rect) -> bool {
        let epsilon = 1e-6;
        left.x + epsilon < right.x + right.width
            && right.x + epsilon < left.x + left.width
            && left.y + epsilon < right.y + right.height
            && right.y + epsilon < left.y + left.height
    }

    #[test]
    fn gives_each_value_its_share_of_the_area_and_stays_inside_the_bounds() {
        let values = [600, 300, 100];
        let placed = squarify(&values, BOUNDS);
        assert_eq!(placed.len(), 3);
        for (index, rect) in &placed {
            let share = f64::from(u32::try_from(values[*index]).unwrap()) / 1000.0;
            assert!((rect.width * rect.height - share * 500_000.0).abs() < 1e-6);
            assert!(rect.x >= 0.0 && rect.y >= 0.0);
            assert!(rect.x + rect.width <= 1000.0 + 1e-6 && rect.y + rect.height <= 500.0 + 1e-6);
        }
    }

    #[test]
    fn never_overlaps_two_tiles() {
        let values: Vec<u64> = (1..=40).map(|value| value * 37 % 101 + 1).collect();
        let placed = squarify(&values, BOUNDS);
        for (index, (_, left)) in placed.iter().enumerate() {
            for (_, right) in &placed[index + 1..] {
                assert!(!overlaps(left, right));
            }
        }
    }

    #[test]
    fn keeps_tiles_near_square_unlike_plain_slices() {
        let placed = squarify(
            &[1, 1, 1, 1],
            Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        );
        for (_, rect) in placed {
            assert!((rect.width / rect.height - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn drops_empty_values_and_an_empty_list() {
        assert_eq!(squarify(&[], BOUNDS).len(), 0);
        assert_eq!(squarify(&[0, 0], BOUNDS).len(), 0);
        assert_eq!(squarify(&[0, 5], BOUNDS).len(), 1);
    }

    fn tile(path: &str, bytes: u64, category: Category) -> Tile {
        Tile {
            path: PathBuf::from(path),
            label: path.trim_start_matches(r"C:\Users\someone\").to_owned(),
            bytes,
            category,
            is_rest: false,
        }
    }

    fn candidate(path: &str, group: Group, bytes: u64) -> Candidate {
        Candidate::new(
            group,
            CandidateKind::PnpmStore,
            Some(PathBuf::from(path)),
            bytes,
        )
    }

    #[test]
    fn marks_a_tile_by_the_strongest_candidate_inside_it_or_around_it() {
        let candidates = [
            candidate(
                r"C:\Users\someone\AppData\Local\pnpm\store",
                Group::Safe,
                10,
            ),
            candidate(
                r"C:\Users\someone\Desktop\Code\old\node_modules",
                Group::Review,
                10,
            ),
        ];
        let pnpm = tile(
            r"C:\Users\someone\AppData\Local\pnpm",
            100,
            Category::PackageCaches,
        );
        let project = tile(
            r"C:\Users\someone\Desktop\Code\old",
            100,
            Category::CodeProjects,
        );
        let documents = tile(r"C:\Users\someone\Documents", 10, Category::DocumentsMedia);
        assert_eq!(cleanup_status(&pnpm, &candidates), TileFill::Safe);
        assert_eq!(cleanup_status(&project, &candidates), TileFill::Review);
        assert_eq!(cleanup_status(&documents, &candidates), TileFill::Nothing);
    }

    #[test]
    fn does_not_take_a_sibling_with_a_common_prefix_for_a_parent() {
        let tools = tile(
            r"C:\Users\someone\AppData\Local\pnpm-tools",
            10,
            Category::DevTools,
        );
        let candidates = [candidate(
            r"C:\Users\someone\AppData\Local\pnpm",
            Group::Safe,
            10,
        )];
        assert_eq!(cleanup_status(&tools, &candidates), TileFill::Nothing);
    }

    #[test]
    fn ignores_an_empty_candidate() {
        let pnpm = tile(
            r"C:\Users\someone\AppData\Local\pnpm",
            100,
            Category::PackageCaches,
        );
        let candidates = [candidate(
            r"C:\Users\someone\AppData\Local\pnpm",
            Group::Safe,
            0,
        )];
        assert_eq!(cleanup_status(&pnpm, &candidates), TileFill::Nothing);
    }

    #[test]
    fn labels_a_big_tile_with_its_folder_name_and_keeps_the_path_in_the_title() {
        let tiles = [
            tile(
                r"C:\Users\someone\AppData\Local\pnpm",
                1000,
                Category::PackageCaches,
            ),
            tile(r"C:\Users\someone\Tiny", 1, Category::Other),
        ];
        let map = treemap(&tiles, &[], TreemapMode::Category, Locale::En);
        let big = &map.tiles[0];
        assert_eq!(big.label.as_deref(), Some("pnpm"));
        assert!(big.title.contains(r"AppData\Local\pnpm"));
        assert_eq!(map.tiles[1].label, None);
        assert_eq!(map.legend.len(), 8);
    }

    #[test]
    fn recommendations_hide_noise_and_put_the_biggest_first() {
        let candidates = [
            candidate(r"C:\a", Group::Safe, 5 * 1024 * 1024),
            candidate(r"C:\b", Group::Safe, 50 * 1024 * 1024),
            candidate(r"C:\c", Group::Safe, 10),
        ];
        let groups = recommendation_groups(&candidates, Locale::En);
        let safe = &groups[0];
        assert_eq!(safe.rows.len(), 2);
        assert_eq!(safe.rows[0].path.as_deref(), Some(r"C:\b"));
        assert_eq!(groups[1].rows.len(), 0);
    }

    #[test]
    fn the_counters_show_what_the_safe_cleanup_would_free() {
        let gib = 1024 * 1024 * 1024;
        let overview = DisksOverview {
            volumes: vec![Volume {
                letter: "C".to_owned(),
                label: String::new(),
                total_bytes: 1000 * gib,
                free_bytes: 300 * gib,
            }],
            physical_disks: vec![],
            scan: Some(ScanResult {
                finished_at: 1_793_097_600,
                home: PathBuf::from(r"C:\Users\someone"),
                total_bytes: 200 * gib,
                files: 10,
                links: 0,
                unreadable: 0,
                tiles: vec![],
                candidates: vec![candidate(r"C:\a", Group::Safe, 9 * gib)],
            }),
        };
        let DisksView::Ready { kpis, volumes, .. } =
            disks_view(Some(&Ok(overview)), None, "C", Locale::En)
        else {
            panic!("not ready");
        };
        let value = |key| {
            kpis.iter()
                .find(|kpi| kpi.key == key)
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(value("after"), "309.0 GB");
        assert_eq!(value("notScanned"), "500.0 GB");
        assert_eq!(volumes[0].used_percent, 70);
    }
}
