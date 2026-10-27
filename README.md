# Vitals

A local health panel for a Windows PC: crashes and errors, disks and safe cleanup, updates, runners and Docker, network,
startup and services, sensors. One small exe written in Rust (Tauri 2, the interface in Leptos compiled to
WebAssembly), no account, no telemetry: everything stays on your PC, in `%APPDATA%\Vitals`.

> Status: errors and crashes, disks with a cleanup preview. The other modules arrive one per release.

## Development

Needs Windows 11, Rust (stable, MSVC, with the `wasm32-unknown-unknown` target) and the Visual Studio Build Tools with
the C++ workload. No Node.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
cargo install tauri-cli --version "^2" --locked

cargo tauri dev     # the panel with hot reload
cargo tauri build   # the release exe and installer
```

The gates CI runs are listed in `CLAUDE.md`; the code rules are in `.claude/rules/`.

| Crate                | What it is                                                         |
| -------------------- | ------------------------------------------------------------------ |
| `src-tauri`          | the core: event log, SQLite, WMI, the home scan, the tray          |
| `crates/vitals-ipc`  | the commands and the types the core and the window exchange        |
| `crates/vitals-ui`   | the window: Leptos components, view models, texts, SCSS on tokens  |
| `xtask`              | styles, design tokens and their checks, commit headers             |

## License

MIT
