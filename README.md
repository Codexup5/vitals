# Vitals

A local health panel for a Windows PC: crashes and errors, disks and safe cleanup, updates, runners and Docker, network,
startup and services, sensors. One small exe written in Rust (Tauri 2, the interface in Leptos compiled to
WebAssembly), no account, no telemetry: everything stays on your PC, in `%APPDATA%\Vitals`.

> Status: v0.1, the first release: every module of v1 works, the CPU temperature and the fans too once you run
> LibreHardwareMonitor.

## Install

Download `Vitals_0.1.0_x64-setup.exe` from [Releases](https://github.com/Codexup5/vitals/releases) and run it: it
installs for your user, without admin rights. The exe is not code-signed yet, so Windows SmartScreen says the publisher
is unknown: "More info" → "Run anyway". Checked on Windows 11 only.

## What it shows

| Module               | What it does                                                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Errors and crashes   | Days without a crash; unexpected restarts, BSODs with their code, WHEA errors, GPU driver resets and app crashes by day; the event feed; the minidumps  |
| Disks and cleanup    | Volumes and drive health; a treemap of the home folder; a cleanup from a whitelist with a preview, "review first" items one by one, every clean logged  |
| Updates              | winget packages with the versions before and after; Windows Update and the GPU driver open their own tools; the BIOS version                            |
| Runners and Docker   | Self-hosted GitHub runners found by their `C:\actions-runner*` folders, Docker containers, a Gaming and a Coding mode. Hidden on a PC with neither      |
| Network              | Latency, loss and jitter to the router and to the internet every 5 minutes; the link speed; a speed test on its button; the devices on the network      |
| Startup and services | Programs that start at sign-in with their cost, publisher and signature; third-party services. Off with a one-click way back; Microsoft's are view only |
| Sensors              | CPU load and clock, memory, NVIDIA cards through `nvidia-smi`; the CPU temperature, power and fans from your LibreHardwareMonitor; a chart and history  |
| Settings             | The start with Windows, in the tray (off by default); Telegram alerts through your own bot (off by default); Claude Code                                |

The window is in Russian or English, by the language of Windows, with a dark (Catppuccin Mocha) and a light (Latte)
theme. Closed, it stays in the tray, and only the light collection runs, every 5 minutes.

## Safety

- It runs as a normal user. An admin action is the same exe under UAC for that one action; "No" means it does not run.
- It never reboots the PC, installs no drivers, never touches HVCI, Defender, the pagefile or Microsoft services.
- The cleanup deletes only inside allowed folders, never follows links or junctions, deletes exactly what its preview
  showed and skips what changed since. Never: the Recycle Bin, Downloads, Documents, browser profiles.

## What leaves the PC

Nothing about you, and only what a feature in use needs:

- a ping to `1.1.1.1` every 5 minutes, for the internet line of the latency chart;
- the speed test, only on its button (`speed.cloudflare.com`);
- Telegram, only with your bot set and sending turned on;
- GitHub's API through your `gh`, only when a runner folder is on the PC;
- winget's sources, when you check or install updates.

## Claude Code

- Settings → Claude Code → "Connect to Claude Code" adds the MCP server `pc-health`. Its tools only read:
  `health_summary`, `events`, `disk_usage`, `cleanup_preview`, `updates_list`, `network_status`.
- `vitals.exe --status` prints one line for your status line script: days without crashes, free space on C:, the
  runners, the GPU temperature.

## Adapt it to your PC

Open a clone of this repository in Claude Code and say: "Read ADAPT.md and adapt Vitals to this PC." It learns the PC,
asks where your code lives and when to warn about the disk, changes those few values and builds the panel.

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

The gates CI runs are listed in `CLAUDE.md`; the code rules are in `.claude/rules/`, the deliberate deviations in
`docs/deviations.md`.

| Crate               | What it is                                                                      |
| ------------------- | ------------------------------------------------------------------------------- |
| `src-tauri`         | the core: event log, SQLite, WMI, performance counters, the home scan, the tray |
| `crates/vitals-ipc` | the commands and the types the core and the window exchange                     |
| `crates/vitals-ui`  | the window: Leptos components, view models, texts, SCSS on tokens               |
| `xtask`             | styles, design tokens and their checks, commit headers                          |

## License

MIT
