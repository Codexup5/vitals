//! `cargo xtask <task>`: the project tasks that are not a plain cargo command.
//!
//! - `styles` compiles the SCSS of the window into `crates/vitals-ui/assets/app.css` (Trunk runs it before a build)
//! - `tokens` builds `styles/_tokens.scss` from the DTCG data
//! - `check` fails on a stale `_tokens.scss`, a design rule or a style rule: the design gates of the kit, in Rust
//! - `commits <range>` checks the commit headers of a range, `origin/main..HEAD` by default
//! - `header <text>` checks one header, the title of a pull request

mod commits;
mod styles;
mod tokens;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn ui() -> PathBuf {
    root().join("crates").join("vitals-ui")
}

fn report(findings: &[String]) -> ExitCode {
    for finding in findings {
        eprintln!("{finding}");
    }
    if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn styles() -> Result<Vec<String>, String> {
    let css = styles::compile(&ui().join("styles").join("main.scss"))?;
    let assets = ui().join("assets");
    fs::write(assets.join("app.css"), css).map_err(|error| error.to_string())?;
    Ok(Vec::new())
}

fn tokens() -> Result<Vec<String>, String> {
    let built = tokens::build(&ui().join("tokens"))?;
    fs::write(ui().join("styles").join("_tokens.scss"), built)
        .map_err(|error| error.to_string())?;
    Ok(Vec::new())
}

fn check() -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    let built = tokens::build(&ui().join("tokens"))?;
    let current = fs::read_to_string(ui().join("styles").join("_tokens.scss")).unwrap_or_default();
    if current.replace("\r\n", "\n") != built {
        findings.push(
            "styles/_tokens.scss is stale: run `cargo xtask tokens` and commit the result"
                .to_owned(),
        );
    }
    findings.extend(tokens::check(&ui().join("tokens"))?);
    let styles_dir = ui().join("styles");
    let mut entries: Vec<PathBuf> = fs::read_dir(&styles_dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "scss")
        })
        .filter(|path| path.file_name().is_some_and(|name| name != "_tokens.scss"))
        .collect();
    entries.sort();
    for path in entries {
        let allows_important = path.file_name().is_some_and(|name| name == "_base.scss");
        findings.extend(styles::lint(&path, allows_important)?);
    }
    // The stylesheet must compile, or the window ships without styles
    styles::compile(&styles_dir.join("main.scss"))?;
    Ok(findings)
}

fn commit_headers(range: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["log", "--format=%s", range])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|header| commits::problem(header).map(|problem| format!("{header}: {problem}")))
        .collect())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("styles") => styles(),
        Some("tokens") => tokens(),
        Some("check") => check(),
        Some("commits") => commit_headers(args.get(1).map_or("origin/main..HEAD", String::as_str)),
        Some("header") => {
            let header = args.get(1).map_or("", String::as_str);
            Ok(commits::problem(header)
                .map(|problem| format!("{header}: {problem}"))
                .into_iter()
                .collect())
        }
        _ => Err(
            "usage: cargo xtask styles | tokens | check | commits [range] | header <text>"
                .to_owned(),
        ),
    };
    match outcome {
        Ok(findings) => report(&findings),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
