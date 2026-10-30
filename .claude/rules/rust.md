---
paths:
  - 'src-tauri/**'
---

# Rust canon (src-tauri)

The core of the panel. The window is Rust too since 2026-10-27; its canon is `ui.md`. Every rule below names its machine
check; a rule without one is checked by review.

## 0. The criterion

Architecture is the cost of the next change. A module of the panel (events, disks,
updates…) is one Rust module whose domain logic is testable without Windows, and a thin layer that talks to Windows.

## 1. Layout

| Where                         | What lives there                                                                                   |
| ----------------------------- | -------------------------------------------------------------------------------------------------- |
| `src/main.rs`                 | `vitals_lib::run()` only                                                                           |
| `src/lib.rs`                  | the Tauri builder: plugins, setup, window events, the command list                                 |
| `src/<domain>/mod.rs`         | the domain: types, pure functions, its error enum — no `tauri::` import, so `cargo test` covers it |
| `src/<domain>/windows.rs`     | the Windows side of the domain (EventLog, registry, WMI): the only place with `unsafe`             |
| `src/commands/<domain>.rs`    | `#[tauri::command]` functions: argument check, a call into the domain, the error mapped for the UI |
| `src/tray.rs`, `app_paths.rs` | the tray icon and menu; `%APPDATA%\Vitals`, the only place the panel writes personal data          |

A command is thin: no logic a test would need lives in it. A command added means a permission in
`capabilities/default.json`, nothing broader than the command needs.

## 2. Errors

- One `thiserror` enum per domain; `?` up to the command, which turns it into a serializable error the UI shows by kind
  (`ui.md` §2 on the window side); the wire shape of the error is `vitals_ipc::CommandErrorBody`. `anyhow` is not used: the UI needs kinds, not text.
- No `unwrap`, `expect`, `panic!`, `todo!`, `dbg!` outside `#[cfg(test)]` (clippy: `unwrap_used`, `expect_used`,
  `panic`, `todo`, `dbg_macro` are deny). A fallible start-up path returns an error from `setup`, not a panic.
- Nothing prints to stdout (`print_stdout` deny); a diagnostic goes to stderr until the log file of stage 1.

## 3. Safety of actions

- `unsafe` only in a `windows.rs` wrapper, each block with a `// SAFETY:` comment (`unsafe_code` deny at the crate;
  the wrapper module allows it by an `#[allow(unsafe_code)]` with a reason; clippy `undocumented_unsafe_blocks` deny).
- The panel runs as a normal user. An admin action is `vitals.exe --elevated <action>` under UAC for that action only;
  a refusal means the action does not run (the brief, §1 p.10).
- Deleting files follows the brief §3; its invariants are `cargo test` cases before any "Clean" button ships: the
  canonical path stays inside an allowed root, links and junctions are never followed, preview and run share one
  snapshot, a busy file is skipped, every clean is logged to `cleanup-log.jsonl`.
- The optimizer changes only what the catalog compiled into the exe names; the old value is in the journal before
  the write, and every change has a revert. Built-in apps are removed for the current user only, never from the image
  and never one of the protected list; OneDrive only after its folder check. `--elevated` never writes HKCU: the user
  hive is written by the normal process.
- The panel never reboots the PC, never installs drivers, never touches HVCI, Defender, the pagefile or Microsoft
  services (the brief, §7).

## 4. Style and gates

| Rule                                                   | Checked by                                                                  |
| ------------------------------------------------------ | --------------------------------------------------------------------------- |
| rustfmt formatting                                     | `cargo fmt --all -- --check`                                                |
| clippy `pedantic` plus the deny list of the workspace  | `cargo clippy --workspace --all-targets --locked -- -D warnings`            |
| Domain logic has tests next to it (`#[cfg(test)] mod`) | `cargo test --workspace --locked`, review                                   |
| A lint is not silenced by a config edit                | review: an `#[allow]` names its reason, a `Cargo.toml` lint change is asked |

CI runs every gate of `ui.md` §5. Names: modules and functions snake_case, types PascalCase, a function is verb + noun;
empty words (`data`, `info`, `utils`, `manager`) are not names. A comment only for a non-obvious algorithm or a
workaround of an external limitation.
