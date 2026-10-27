---
paths:
  - 'crates/**'
  - 'xtask/**'
---

# The window canon (crates/vitals-ui, crates/vitals-ipc)

The window is Leptos compiled to WebAssembly, inside the Tauri window of the core. Since 2026-10-27 the whole panel is
Rust (the user's decision); this file replaces the kit's web-react `architecture.md`, `code.md` and `styles.md` and keeps
their ideas where they still hold. Every rule names its machine check; a rule without one is checked by review.

## 0. The criterion

Architecture is the cost of the next change. A new screen touches its view model, its component, its texts and its
styles, and it is clear which files those are.

## 1. Layout

| Where                         | What lives there                                                                                        |
| ----------------------------- | ------------------------------------------------------------------------------------------------------- |
| `crates/vitals-ipc`           | the contract: command names, their argument and answer types; the core and the window compile the same |
| `crates/vitals-ui/src/views`  | a screen as data: pure functions from the answers of the core to a view model, with `cargo test` cases  |
| `crates/vitals-ui/src/ui`     | Leptos components: they draw a view model and wire events, nothing more                                |
| `crates/vitals-ui/src/texts.rs` | every interface text, `RU` and `EN` values of one struct                                             |
| `crates/vitals-ui/src/format.rs` | numbers, sizes and dates per language                                                                |
| `crates/vitals-ui/src/ipc.rs` | the only call of `invoke`: a deadline, the answer read into its type, one `IpcError`                    |
| `crates/vitals-ui/styles`     | SCSS on tokens; `_tokens.scss` is built from `tokens/` by `cargo xtask tokens` and never edited         |
| `xtask`                       | styles, tokens, the design and style checks, commit headers                                              |

## 2. Code

- **A wire type lives once**, in `vitals-ipc`. A field renamed in the core is a compile error in the window (cargo).
- **A component draws, a view model decides.** Counting, choosing, formatting and the five states (loading, empty,
  error, partial, ready) are an enum from `views/`, tested natively (`cargo test`); a component matches on it.
- **No interface string in code** outside `texts.rs` and the tray of the core: the Cyrillic check of `kit.json`. A text
  with a value in it is a function in the catalog; a missing text in one language does not compile.
- **State:** what must survive a reload lives in the address (the errors screen: range, kinds, page); answers of the
  core live in a `LocalResource`; local UI (a treemap mode) in a signal. Preferences go through `PreferencesStore`.
- **Errors:** every failure of a call is an `IpcError` with a kind; the screen shows what happened and a retry
  (`Failure`), never a raw message. A failed block keeps the rest of the screen alive.
- **Clippy pedantic** with the deny list of the workspace, natively and for `wasm32-unknown-unknown`; an `#[allow]`
  names its reason (review).

## 3. Styles and accessibility

- Spacing, font sizes, radii and colours are `var(--…)` tokens; 1–3 px is an optical correction; a line height has no
  unit; `!important` lives only in the reduced-motion block (`cargo xtask check`).
- Anything interactive is a native element: `button` for an action, `a href` for navigation. One global
  `:focus-visible` rule. A state is never colour alone: a tag carries its word, a chart kind its texture.
- Targets: `--touch-target` 32 px with a fine pointer, 44 px with a coarse one. Text zooms to 200 % (Ctrl+ in the
  window, `zoomHotkeysEnabled`).
- Skeletons are static; animation respects `prefers-reduced-motion`.

## 4. Design rules

`cargo xtask check` judges the token data in every theme and pointer context, as the kit's `tokens:check` did.

| Id                     | Rule                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------ |
| `design/grid`          | a token of role space, size or target is a multiple of 4 px; 1–2 px only for lines                     |
| `design/lh-unitless`   | a line-height token is a number                                                                        |
| `design/lh-grid`       | font size × line height is a multiple of 4 px within 0.01 px; a one-line role is exempt                |
| `design/contrast-text` | every text token reaches 4.5:1 on every surface it meets in every theme, a `large` one 3:1             |
| `design/contrast-ui`   | every ui token reaches 3:1 on every surface it meets                                                   |
| `design/target-fine`   | a target token is at least 24 px                                                                       |
| `design/target-coarse` | a target token is at least 44 px under `any-pointer: coarse`                                           |

## 5. Gates

| Gate                                                     | Command                                                              |
| -------------------------------------------------------- | -------------------------------------------------------------------- |
| formatting                                               | `cargo fmt --all -- --check`                                         |
| design tokens, a stale `_tokens.scss`, the style rules   | `cargo xtask check`                                                  |
| clippy, native and WebAssembly                           | `cargo clippy --workspace --all-targets`, `-p vitals-ui --target wasm32-unknown-unknown` |
| tests                                                    | `cargo test --workspace`                                             |
| commit headers and the PR title                          | `cargo xtask commits`, `cargo xtask header` (CI)                     |
