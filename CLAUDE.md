# Vitals

A local Windows PC health panel, all Rust: Tauri 2 for the core and the window, Leptos compiled to WebAssembly for the
interface. The decisions are in the brief, `C:\Users\olegs\Desktop\Code\vitals-brief.md` (kept outside the repository: it
holds the owner's machine facts). On 2026-10-27 the user moved the interface from React/TypeScript to Rust; this
overrides the brief's React line, and the kit's web-react canon no longer applies.

- **No back end, no telemetry.** The window calls the core through `invoke`; nothing leaves the PC except the user's own
  Telegram and the Vitals update check.
- **One contract.** The command names and the types they exchange live in `crates/vitals-ipc`, compiled by both sides.
- **Texts are ru and en**, every one in `crates/vitals-ui/src/texts.rs` (`RU` and `EN` of one struct: a missing text
  does not compile).
- **Theme and language** live in `crates/vitals-ui/src/preferences.rs` (Mocha by default, Latte on
  `data-theme='light'`).

```bash
cargo tauri dev                 # the panel with hot reload (Trunk serves the window on 127.0.0.1:3000)
cargo tauri build               # the release exe and the NSIS installer in target/release
cargo fmt --all -- --check      # the gates, as CI runs them
cargo xtask check               #   design tokens, a stale _tokens.scss, the style rules
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p vitals-ui --target wasm32-unknown-unknown --locked -- -D warnings
cargo test --workspace --locked
```

Tools: the `wasm32-unknown-unknown` target (`rustup target add`), Trunk 0.21 and the Tauri CLI 2 (`cargo install trunk
--version 0.21.14 --locked`, `cargo install tauri-cli --version ^2 --locked`). No Node.

| Where                     | What is there                                                              |
| ------------------------- | -------------------------------------------------------------------------- |
| `.claude/rules/rust.md`   | the core canon: layout, errors, `unsafe`, safety of actions, gates         |
| `.claude/rules/ui.md`     | the window canon: views and components, texts, state, styles, design rules |
| `src-tauri/capabilities/` | what the window may call: one permission per command                       |
| `docs/deviations.md`      | every deliberate deviation, with its reason                                |
| `docs/known-issues.md`    | recurring traps: symptom → cause → how to avoid it                         |

## Boundaries

- A conflict between the task and the canon: say it before the code. A gate is not bypassed by editing its config; a
  rule changes only by the user's decision.
- Wait for confirmation for: deletion, irreversible changes, canon edits.
- Done = every gate above green **and** the scenario checked in the running panel. Anything unchecked is called
  unchecked.
