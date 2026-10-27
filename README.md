# Vitals

A local health panel for a Windows PC: crashes and errors, disks and safe cleanup, updates, runners and Docker, network,
startup and services, sensors. One small exe (Tauri 2 + Rust, UI in WebView2), no account, no telemetry: everything stays
on your PC, in `%APPDATA%\Vitals`.

> Status: stage 0 of the work plan — the shell (tray icon, window, navigation over the seven modules, ru/en,
> Catppuccin Mocha and Latte). The modules arrive one per release.

## Development

Needs Windows 11, Node 24 with pnpm 11, Rust (stable, MSVC) and the Visual Studio Build Tools with the C++ workload.

```bash
pnpm install
pnpm tauri dev     # the panel with hot reload
pnpm verify        # every gate: types, ESLint, stylelint, design tokens, FSD layers, knip, tests, rustfmt, clippy, cargo test
pnpm tauri build   # the release exe and installer
```

The code rules are in `CLAUDE.md` and `.claude/rules/`; the front end follows the [kit](https://github.com/Codexup5/kit)
canon.

## License

MIT
