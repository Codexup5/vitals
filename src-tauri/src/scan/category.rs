//! Which category a folder of the home belongs to, for the treemap legend of the brief, §4.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    DocumentsMedia,
    DevTools,
    CodeProjects,
    Sdks,
    AiModels,
    PackageCaches,
    System,
    Other,
}

/// Relative to the home, lower case, `/`-separated prefixes; the first match wins, so the specific go first.
const RULES: &[(&str, Category)] = &[
    (".ollama", Category::AiModels),
    ("appdata/local/ollama", Category::AiModels),
    (".lmstudio", Category::AiModels),
    (".cache/lm-studio", Category::AiModels),
    (".cache/huggingface", Category::AiModels),
    ("appdata/local/pnpm", Category::PackageCaches),
    ("appdata/local/npm-cache", Category::PackageCaches),
    ("appdata/roaming/npm-cache", Category::PackageCaches),
    (".npm", Category::PackageCaches),
    (".cargo/registry", Category::PackageCaches),
    (".cargo/git", Category::PackageCaches),
    (".nuget/packages", Category::PackageCaches),
    ("appdata/local/yarn", Category::PackageCaches),
    ("appdata/local/pip", Category::PackageCaches),
    (".gradle/caches", Category::PackageCaches),
    (".rustup", Category::Sdks),
    (".dotnet", Category::Sdks),
    ("appdata/local/android", Category::Sdks),
    ("appdata/roaming/fnm", Category::Sdks),
    ("appdata/local/fnm_multishells", Category::Sdks),
    (".cargo", Category::DevTools),
    (".vscode", Category::DevTools),
    (".docker", Category::DevTools),
    ("appdata/local/docker", Category::DevTools),
    ("appdata/local/programs", Category::DevTools),
    ("appdata/local/jetbrains", Category::DevTools),
    ("documents", Category::DocumentsMedia),
    ("pictures", Category::DocumentsMedia),
    ("videos", Category::DocumentsMedia),
    ("music", Category::DocumentsMedia),
    ("downloads", Category::DocumentsMedia),
    ("desktop", Category::DocumentsMedia),
    ("onedrive", Category::DocumentsMedia),
    ("appdata/local/temp", Category::System),
    ("appdata/local/microsoft", Category::System),
    ("appdata/local/packages", Category::System),
];

fn normalized(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_under(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `path` is absolute; `code_roots` win over every rule (a project on the Desktop is code, not a document).
pub fn category_of(path: &Path, home: &Path, code_roots: &[&Path]) -> Category {
    let target = normalized(path);
    if code_roots
        .iter()
        .any(|root| is_under(&target, &normalized(root)))
    {
        return Category::CodeProjects;
    }
    let Ok(relative) = path.strip_prefix(home) else {
        return Category::Other;
    };
    let relative = normalized(relative);
    RULES
        .iter()
        .find(|(prefix, _)| is_under(&relative, prefix))
        .map_or(Category::Other, |(_, category)| *category)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn categories_of_known_folders() {
        let home = PathBuf::from(r"C:\Users\someone");
        let code = home.join(r"Desktop\Code");
        let roots = [code.as_path()];
        let of = |relative: &str| category_of(&home.join(relative), &home, &roots);
        assert_eq!(of(r"Desktop\Code\kit"), Category::CodeProjects);
        assert_eq!(of(r"Desktop\notes.txt"), Category::DocumentsMedia);
        assert_eq!(of(r"AppData\Local\pnpm\store"), Category::PackageCaches);
        assert_eq!(of(r".cargo\registry"), Category::PackageCaches);
        assert_eq!(of(r".cargo\bin"), Category::DevTools);
        assert_eq!(of(r".rustup\toolchains"), Category::Sdks);
        assert_eq!(of(r".ollama\models"), Category::AiModels);
        assert_eq!(of(r"AppData\Local\Temp"), Category::System);
        assert_eq!(of(r"Saved Games"), Category::Other);
        // A prefix is a whole folder name: .npmrc is not under .npm
        assert_eq!(of(r".npmrc"), Category::Other);
    }
}
