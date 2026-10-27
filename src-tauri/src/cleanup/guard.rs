//! Where a cleanup may delete: each kind of candidate has its allowed root, and some places are never touched
//! whatever a candidate says (the brief, §3: the Recycle Bin, Downloads, Documents, `%TEMP%\claude`, browser profiles).

use std::path::{Component, Path, PathBuf};

use super::{CandidateKind, Places};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("this kind of candidate is not cleaned by deleting files")]
    NotFiles,
    #[error("the folder is not where this kind of candidate lives")]
    NotAllowed,
    #[error("the folder is one the panel never touches")]
    Forbidden,
    #[error("the folder is gone")]
    Missing,
}

/// The components of a path, lower case: Windows paths compare without case.
fn parts(path: &Path) -> Vec<String> {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// `child` is `parent` or lies under it, component by component.
pub fn is_within(child: &Path, parent: &Path) -> bool {
    let child = parts(child);
    let parent = parts(parent);
    child.len() >= parent.len() && child[..parent.len()] == parent[..]
}

/// A path with a `..` or a `.` in it could walk out of its root after the check: such a path is never trusted.
pub fn is_plain(path: &Path) -> bool {
    path.components()
        .all(|component| !matches!(component, Component::ParentDir | Component::CurDir))
}

/// `canonicalize` when the path exists, the path itself otherwise: a forbidden place that does not exist yet stays
/// forbidden.
fn resolved(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The places no cleanup ever enters.
pub fn forbidden(places: &Places) -> Vec<PathBuf> {
    let mut never = vec![
        places.temp.join("claude"),
        places.home.join("Downloads"),
        places.home.join("Documents"),
        places.local_app_data.join(r"Google\Chrome\User Data"),
        places.local_app_data.join(r"Microsoft\Edge\User Data"),
        places
            .local_app_data
            .join(r"BraveSoftware\Brave-Browser\User Data"),
        places
            .local_app_data
            .join(r"Yandex\YandexBrowser\User Data"),
        places.roaming_app_data.join(r"Mozilla\Firefox\Profiles"),
        places.roaming_app_data.join(r"Opera Software"),
    ];
    never.extend(never.clone().iter().map(|path| resolved(path)));
    never
}

/// The Recycle Bin is never touched, on any drive.
fn is_recycle_bin(path: &Path) -> bool {
    parts(path).iter().any(|part| part == "$recycle.bin")
}

pub fn is_forbidden(path: &Path, forbidden: &[PathBuf]) -> bool {
    is_recycle_bin(path) || forbidden.iter().any(|never| is_within(path, never))
}

fn shader_caches(places: &Places) -> [PathBuf; 3] {
    [
        places.local_app_data.join(r"NVIDIA\DXCache"),
        places.local_app_data.join(r"NVIDIA\GLCache"),
        places.local_app_data.join("D3DSCache"),
    ]
}

fn ends_with(path: &Path, tail: &[&str]) -> bool {
    let parts = parts(path);
    parts.len() >= tail.len()
        && parts[parts.len() - tail.len()..]
            .iter()
            .zip(tail)
            .all(|(part, name)| part == name)
}

fn in_code_root(path: &Path, places: &Places) -> bool {
    places
        .code_roots
        .iter()
        .any(|root| is_within(path, &resolved(root)) && !parts(path).eq(&parts(&resolved(root))))
}

/// The canonical root a candidate may be cleaned in, or why it may not.
pub fn allowed_root(kind: CandidateKind, path: &Path, places: &Places) -> Result<PathBuf, Refusal> {
    if !is_plain(path) {
        return Err(Refusal::NotAllowed);
    }
    let root = std::fs::canonicalize(path).map_err(|_| Refusal::Missing)?;
    let same = |place: &Path| parts(&root) == parts(&resolved(place));
    let is_allowed = match kind {
        CandidateKind::UserTemp => same(&places.temp),
        CandidateKind::WindowsTemp => same(&places.windows_temp),
        CandidateKind::ShaderCache => shader_caches(places).iter().any(|cache| same(cache)),
        CandidateKind::NextCache => {
            in_code_root(&root, places) && ends_with(&root, &[".next", "cache"])
        }
        CandidateKind::Turbo => in_code_root(&root, places) && ends_with(&root, &[".turbo"]),
        CandidateKind::StaleNodeModules => {
            in_code_root(&root, places) && ends_with(&root, &["node_modules"])
        }
        CandidateKind::PnpmStore
        | CandidateKind::NpmCache
        | CandidateKind::DockerDangling
        | CandidateKind::DockerBuildCache
        | CandidateKind::DockerUnusedImages
        | CandidateKind::Vhdx => return Err(Refusal::NotFiles),
    };
    if !is_allowed {
        return Err(Refusal::NotAllowed);
    }
    if is_forbidden(&root, &forbidden(places)) {
        return Err(Refusal::Forbidden);
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn places(root: &Path) -> Places {
        Places::under(root)
    }

    #[test]
    fn within_compares_whole_components_without_case() {
        assert!(is_within(
            Path::new(r"C:\Users\A\Temp\x"),
            Path::new(r"c:\users\a\temp")
        ));
        assert!(!is_within(
            Path::new(r"C:\Users\A\Temp2\x"),
            Path::new(r"C:\Users\A\Temp")
        ));
        assert!(!is_plain(Path::new(r"C:\Temp\..\Windows")));
    }

    #[test]
    fn each_kind_has_its_own_root() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        let project = places.code_roots[0].join("app");
        for dir in [
            places.temp.clone(),
            project.join(".next/cache"),
            project.join(".turbo"),
            project.join("node_modules"),
            places.local_app_data.join("NVIDIA/DXCache"),
            places.home.join("Downloads"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        assert!(allowed_root(CandidateKind::UserTemp, &places.temp, &places).is_ok());
        assert!(
            allowed_root(
                CandidateKind::NextCache,
                &project.join(".next/cache"),
                &places
            )
            .is_ok()
        );
        assert!(
            allowed_root(
                CandidateKind::StaleNodeModules,
                &project.join("node_modules"),
                &places
            )
            .is_ok()
        );
        assert!(
            allowed_root(
                CandidateKind::ShaderCache,
                &places.local_app_data.join("NVIDIA/DXCache"),
                &places
            )
            .is_ok()
        );
        // A kind's root elsewhere is refused: the temp folder is not a Next.js cache, Downloads not a node_modules
        assert_eq!(
            allowed_root(CandidateKind::NextCache, &places.temp, &places),
            Err(Refusal::NotAllowed)
        );
        assert_eq!(
            allowed_root(
                CandidateKind::StaleNodeModules,
                &places.home.join("Downloads"),
                &places
            ),
            Err(Refusal::NotAllowed)
        );
        assert_eq!(
            allowed_root(
                CandidateKind::Turbo,
                &project.join(".turbo/../.turbo"),
                &places
            ),
            Err(Refusal::NotAllowed)
        );
        assert_eq!(
            allowed_root(CandidateKind::PnpmStore, &places.temp, &places),
            Err(Refusal::NotFiles)
        );
    }

    #[test]
    fn the_never_list_wins_over_any_root() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        let never = forbidden(&places);
        assert!(is_forbidden(
            &places.temp.join(r"claude\session.json"),
            &never
        ));
        assert!(is_forbidden(
            &places.home.join(r"Downloads\setup.exe"),
            &never
        ));
        assert!(is_forbidden(
            Path::new(r"D:\$Recycle.Bin\S-1-5\file"),
            &never
        ));
        assert!(!is_forbidden(&places.temp.join("old.tmp"), &never));
    }
}
