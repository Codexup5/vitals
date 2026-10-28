//! Connecting the MCP server "pc-health" to Claude Code, on the user's click (the brief, §1 p.14: off by default).

use std::path::{Path, PathBuf};
use std::time::Duration;

use vitals_ipc::{ClaudeStatus, ConnectOutcome};

const SERVER: &str = "pc-health";
const CLI_DEADLINE: Duration = Duration::from_secs(60);

/// `claude.exe` on the PATH, else where its installer puts it.
fn claude_cli() -> Option<PathBuf> {
    std::env::var_os("PATH")
        .and_then(|path| crate::runners::source::find_on_path("claude.exe", &path))
        .or_else(|| {
            let profile = std::env::var_os("USERPROFILE")?;
            let path = Path::new(&profile).join(r".local\bin\claude.exe");
            path.is_file().then_some(path)
        })
}

/// The user scope of Claude Code keeps its servers in `~/.claude.json`, `mcpServers`.
fn is_connected_in(config: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(config)
        .ok()
        .and_then(|config| config.get("mcpServers")?.get(SERVER).cloned())
        .is_some()
}

fn claude_config() -> Option<String> {
    let profile = std::env::var_os("USERPROFILE")?;
    std::fs::read_to_string(Path::new(&profile).join(".claude.json")).ok()
}

fn exe() -> String {
    std::env::current_exe().map_or_else(
        |_| "vitals.exe".to_owned(),
        |path| path.display().to_string(),
    )
}

#[tauri::command(async)]
pub fn claude_status() -> ClaudeStatus {
    ClaudeStatus {
        has_cli: claude_cli().is_some(),
        is_connected: claude_config().is_some_and(|config| is_connected_in(&config)),
        status_command: format!("\"{}\" {}", exe(), crate::STATUS_FLAG),
    }
}

/// `claude mcp add --scope user pc-health -- <this exe> --mcp`; an older entry, of another path, goes first.
#[tauri::command(async)]
pub fn claude_connect() -> ConnectOutcome {
    let Some(cli) = claude_cli() else {
        return ConnectOutcome::NoCli;
    };
    let cli = cli.display().to_string();
    let _ = crate::process::output(
        &cli,
        &["mcp", "remove", "--scope", "user", SERVER],
        CLI_DEADLINE,
    );
    let exe = exe();
    let added = crate::process::output(
        &cli,
        &[
            "mcp",
            "add",
            "--scope",
            "user",
            SERVER,
            "--",
            &exe,
            crate::mcp::FLAG,
        ],
        CLI_DEADLINE,
    );
    match added {
        Ok(done) if done.code == 0 => {
            tracing::info!("pc-health is connected to Claude Code");
            ConnectOutcome::Done
        }
        Ok(done) => {
            tracing::warn!("claude mcp add exited with {}", done.code);
            ConnectOutcome::Failed
        }
        Err(error) => {
            tracing::warn!("claude did not run: {error}");
            ConnectOutcome::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_is_found_in_the_user_config() {
        assert!(is_connected_in(
            r#"{"mcpServers":{"pc-health":{"type":"stdio","command":"vitals.exe","args":["--mcp"]}}}"#
        ));
        assert!(!is_connected_in(r#"{"mcpServers":{"playwright":{}}}"#));
        assert!(!is_connected_in(r#"{"projects":{}}"#));
        assert!(!is_connected_in("not json"));
    }
}
