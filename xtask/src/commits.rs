//! Commit headers in the Conventional Commits form, as commitlint held them: a known type, an optional scope, a
//! subject, at most 72 characters.

const TYPES: &[&str] = &[
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];
const MAX_HEADER: usize = 72;

/// What is wrong with a header, or nothing.
pub fn problem(header: &str) -> Option<String> {
    if header.starts_with("Merge ") {
        return None;
    }
    if header.chars().count() > MAX_HEADER {
        return Some(format!("longer than {MAX_HEADER} characters"));
    }
    let Some((kind, subject)) = header.split_once(": ") else {
        return Some("no `type: subject`".to_owned());
    };
    let kind = kind.trim_end_matches('!');
    let kind = match kind.split_once('(') {
        Some((kind, scope)) if scope.ends_with(')') && scope.len() > 1 => kind,
        Some(_) => return Some("a broken scope".to_owned()),
        None => kind,
    };
    if !TYPES.contains(&kind) {
        return Some(format!("`{kind}` is not one of {}", TYPES.join(", ")));
    }
    if subject.trim().is_empty() {
        return Some("an empty subject".to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conventional_header_passes() {
        assert_eq!(problem("feat: disks screen with home treemap"), None);
        assert_eq!(problem("fix(ui)!: drop the old router"), None);
        assert_eq!(
            problem("Merge pull request #4 from Codexup5/stage-3a/disks"),
            None
        );
    }

    #[test]
    fn a_wrong_header_says_why() {
        assert!(problem("added stuff").is_some());
        assert!(problem("feature: a thing").is_some());
        assert!(problem("feat(: a thing").is_some());
        assert!(problem(&format!("feat: {}", "x".repeat(80))).is_some());
    }
}
