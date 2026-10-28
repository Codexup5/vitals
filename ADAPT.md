# Adapt Vitals to this PC

A prompt for Claude Code. Open a clone of this repository in Claude Code and say: **"Read ADAPT.md and adapt Vitals
to this PC."** Everything below is addressed to Claude.

---

You adapt Vitals, a local health panel for Windows, to the PC you run on. Most of it needs no change: it finds the
drives, the cards, the runners and the language of Windows by itself. Your work is to learn the PC, ask the user what
only they know, change the few values below, and build the panel. Answer the user in their language. Read `CLAUDE.md`
and `.claude/rules/` first: they are the canon of the code, and this file does not override them.

## 1. Learn the PC

Read only; nothing here changes the PC.

| What                     | How                                                                                      |
| ------------------------ | ---------------------------------------------------------------------------------------- |
| Windows and its language | `Get-ComputerInfo -Property OsName, OsVersion, OsLanguage`                               |
| Drives                   | `Get-Volume`                                                                             |
| Graphics cards           | `Get-CimInstance Win32_VideoController \| Select-Object Name`; `nvidia-smi -L` if NVIDIA |
| Self-hosted runners      | `Get-ChildItem C:\ -Directory -Filter 'actions-runner*'`                                 |
| Docker, WSL, GitHub CLI  | `docker version`, `wsl -l -v`, `gh auth status`                                          |
| winget                   | `winget --version`                                                                       |
| The tools of the build   | `rustc -V`, `rustup target list --installed`, `trunk -V`, `cargo tauri -V`               |

Then ask the user, one question at a time:

1. Where do your code projects live? The cleanup looks for project caches and old `node_modules` only there.
2. Below how many free gigabytes on C: should Vitals warn?
3. Do you want Telegram alerts? If yes, they make their own bot with @BotFather; the token goes into the window, never
   into the code.

## 2. What may change

Each value has tests next to it: change the test that pins the old value, and add a case for the new one.

| What                                  | Where                                                              | As shipped                   |
| ------------------------------------- | ------------------------------------------------------------------ | ---------------------------- |
| Code roots of the cleanup             | `src-tauri/src/commands/disks.rs`, `code_roots`                    | `%USERPROFILE%\Desktop\Code` |
| Low disk alert                        | `src-tauri/src/alerts/mod.rs`, `DISK_MIN_GB`                       | 50 GB                        |
| A runner offline before the alert     | `src-tauri/src/alerts/mod.rs`, `RUNNER_GRACE_MINUTES`              | 30 minutes                   |
| Runner folders                        | `src-tauri/src/runners/source.rs`, `RUNNER_FOLDER_PREFIX`          | `C:\actions-runner*`         |
| The internet end of the latency chart | `src-tauri/src/network/mod.rs`, `INTERNET_HOST`                    | `1.1.1.1`, pings only        |
| Categories of the home folder treemap | `src-tauri/src/scan/category.rs`, `RULES`                          | AI models, caches, SDKs, …   |
| A warm and a hot graphics card        | `crates/vitals-ui/src/views/sensors.rs`, `GPU_WARM_C`, `GPU_HOT_C` | 80 °C, 90 °C                 |

A new text goes into both languages of `crates/vitals-ui/src/texts.rs`; a missing one does not compile.

What needs no code at all:

- **Language and theme**: by Windows, and switched in the window.
- **Telegram**: Settings → Telegram: the bot token, the chat, a test message. Sending stays off until the user turns it
  on.
- **Claude Code**: Settings → Claude Code → "Connect to Claude Code" adds the MCP server `pc-health`; the status line
  is `vitals.exe --status` in the user's own status line script.
- **Runners and Docker**: the screen hides itself when the PC has neither.
- **Sensors**: NVIDIA cards are read through the `nvidia-smi` of their driver. The CPU temperature, the fans and other
  makers' cards need LibreHardwareMonitor, which the user installs themselves (its PawnIO driver, never WinRing0).

## 3. Never

- **Loosen the cleanup.** No new allowed root, nothing of the "never" list (the Recycle Bin, Downloads, Documents,
  the Desktop outside caches, browser profiles, `%TEMP%\claude`), no following of links or junctions. The invariant
  tests in `src-tauri/src/cleanup` stay as they are and green. A narrower list is fine.
- Install drivers, reboot the PC, touch HVCI, Defender, the pagefile or Microsoft services; add a registry cleaner, a
  "booster" or a debloat.
- Send anything new out of the PC: no telemetry, no new host. Personal data lives only in `%APPDATA%\Vitals`.
- Silence a gate by editing its configuration.
- Push this PC's facts (paths, names, tokens) to the upstream repository: adapt on a branch of the user's own fork.

## 4. Build and install

1. The gates of `CLAUDE.md`, all green.
2. `cargo tauri build`: the exe is `target\release\vitals.exe`, the installer
   `target\release\bundle\nsis\Vitals_<version>_x64-setup.exe`. Quit a running panel from the tray first: Windows does
   not replace a running exe.
3. The user runs the installer, or copies the exe where they keep programs. Windows SmartScreen says "unknown
   publisher": the exe is not signed. "More info" → "Run anyway".
4. In the window: Settings → Telegram and Claude Code, if the user wants them.

## 5. Report

What you found on the PC, what you changed (`file:line`), the gates, and what the user should look at in the window.
Name what you did not check.
