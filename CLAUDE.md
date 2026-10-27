<!-- kit:web-react -->

# Project

React 19 · TypeScript strict · Vite · pnpm · FSD 2.1 (hybrid) · TanStack Query / Router / Form · Zod · SCSS modules on
tokens · Vitest + Testing Library + MSW · Playwright · Sentry. The back end is reached through the api adapter (openapi-rest, the default: a REST back end the project
does not own; trpc: the own back end of the project in `server/`; trpc-query: trpc with the tRPC options proxy in `entities/*/model`).

```bash
pnpm dev            # localhost:3000, API on MSW mocks (.env.development)
pnpm verify         # typecheck · lint · lint:styles · tokens:check · csp:check · lint:arch · lint:placement · prettier · knip · context:check · canon:check · conventions (a project with `scripts/conventions.mjs`) · test — before finishing a task
pnpm typecheck:baseline # freeze or shrink the debt of a check: also lint:baseline, lint:styles:baseline, lint:arch:baseline, lint:placement:baseline, tokens:baseline, knip:baseline — a baseline only shrinks
pnpm test:e2e       # Playwright scenarios on mobile and desktop, and the built app (project prod)
pnpm test:e2e:docker # the same run in the Playwright image, as CI runs it
pnpm test:visual    # the Linux visual baseline in the Playwright image; test:visual:update rewrites it
pnpm mutation       # Stryker on lib/, mappers and schemas, 80% threshold — in strict and weekly in CI
pnpm context        # rebuild the public API catalog after a change to a slice public API
pnpm canon          # rebuild docs/canon.md after a change to a rule file, docs/gates.md or a skill; canon:check fails a stale one
pnpm tokens         # build the DTCG tokens of src/shared/styles/tokens into tokens.scss
pnpm csp:check      # csp.json against the canon, index.html and the hosting header of vercel.json
```

A back end in `server/` brings its own tsconfig, eslint and vitest configs; `pnpm dev` serves `api/hub.ts` under `/api`.
The trpc-query adapter (`create --template swift --adapter trpc-query`) takes the files of trpc, drops
`entities/*/api/*-api.ts` and adds `shared/api/trpc-query.ts` under `src/` and the user model files. The trpc adapters expect `server/router.ts`, `server/test/test-hub.ts` and `shared/api/auth-headers.ts` under `src/` from the
template, and start the Telegram profile of `packs/telegram` (`shared/lib/telegram-profile.ts` and
`shared/config/telegram-palette.ts` under `src/`).

## What to read and when

This file is a router. The rules in `.claude/rules/` are binding both when generating and when reviewing. They load by
path (`paths:` in each rule), so a session that touches no code does not carry them. Agents run without this file
(`omitClaudeMd`) and read the rules they need themselves; the planner keeps this file, takes the reference from it and
reads `architecture.md` itself.

| Where                                   | What is there                                                                                              |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `docs/canon.md`                         | **read first**: one line per rule — where it is written, the gate that checks it, the skills that apply it |
| `.claude/rules/architecture.md`         | layers, slices, state, data flow, prohibitions and their reasons                                           |
| `.claude/rules/code.md`                 | components, hooks, requests, errors, types, naming, forms, tests                                           |
| `.claude/rules/styles.md`               | tokens, breakpoints, accessibility, motion                                                                 |
| `docs/context/public-api.md`            | **what already exists** in slice public APIs — built by `pnpm context`                                     |
| `docs/gates.md`                         | every canon rule with the machine check and the test behind it                                             |
| `docs/process.md`                       | process notes: checks by kind of change, debugging, a dependency, review list                              |
| `docs/security-headers.md`              | the content security policy: csp.json, the build meta, the hosting header, the Report-Only → enforce path  |
| `docs/deviations.md`                    | deliberate deviations from the canon and debt in numbers                                                   |
| `docs/known-issues.md`                  | recurring traps: symptom → cause → how to avoid it                                                         |
| `src/entities/user`, `src/pages/user-*` | the reference: copy from it, not from the neighbouring file                                                |

**Before creating** a component, hook or helper — `grep` `docs/context/public-api.md`. It exists — use it; it does not
— write it.

**Plan order:** plans go bottom-up `shared → entities → features → pages → app`, with no `widgets` layer.

## Task → skill

| Task                                                     | Skill                                                        |
| -------------------------------------------------------- | ------------------------------------------------------------ |
| New domain: entity, list, card, form                     | `new-domain`                                                 |
| API method, DTO, mapper                                  | `api-method`                                                 |
| Query, mutation, keys, invalidation, loader              | `query`                                                      |
| List: filters and pagination in the URL                  | `list`                                                       |
| Form: create, edit, back-end errors                      | `form`                                                       |
| UI review against the Web Interface Guidelines checklist | `web-design-guidelines`                                      |
| Playwright E2E: scenario, locators, flakes               | `playwright-best-practices`                                  |
| React performance, composition                           | `vercel-react-best-practices`, `vercel-composition-patterns` |
| Accessibility, visual design, FSD questions              | `accessibility`, `frontend-design`, `feature-sliced-design`  |

<!-- /kit:web-react -->

<!-- kit:core -->

## Task → skill

| Task                                                | Skill                                                                                           |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Non-trivial task, plan, decision                    | `kit:grill-me`, then `kit:plan` at 5+ files                                                     |
| Execute an approved plan step by step               | workflow `/kit:ship` (`plan: docs/plans/…`, `phase`)                                            |
| A task from an issue or text to a PR, autopilot     | `kit:task` — only the user starts it: `/kit:task <issue or text>`, or alone for the Ready queue |
| Bug, regression, "it doesn't work"                  | `kit:debug`                                                                                     |
| Session review: lessons from blocks and corrections | `kit:retro`                                                                                     |
| Refactoring, moving, deleting                       | `kit:refactoring`                                                                               |
| Check that it works before "done"                   | `kit:verify`                                                                                    |
| Branch review before a PR, answering a review       | `kit:review`                                                                                    |
| A separate copy for a task                          | `kit:worktree`                                                                                  |
| Finish a branch: PR, merge, cleanup                 | `kit:finish-branch`                                                                             |
| Commit                                              | `kit:commit`                                                                                    |
| New dependency or utility                           | `kit:search-first`; a ready skill from the registry — `kit:find-skills`                         |
| Roll kit onto an existing project                   | `kit:adopt`                                                                                     |

## Agents and models

The main session runs the task itself. An agent is called when the work is isolated and its result is a conclusion, not
reading files.

Default route is **Kit+**: the session runs on Opus xhigh, agents on the model and effort below. Inside `/kit:ship` the
implementer follows the step risk: light (every file under `docs/`) runs on Haiku high without the step-reviewer, normal
on Sonnet high, critical (a `Risk: critical` line or a `models.critical` file) on Sonnet high with the second route, the
`kit:oracle` tests. A returned step retries one rung up. `kit-config models` shows the route of a kit mode. `kit:runner`
(haiku, high) only runs `kit-plan-steps open`, `seal` and `take`, `kit-commit` and `pnpm verify` inside `/kit:ship`.
With `tests` later or off in kit.json there is no second route: a critical step starts at its implementer.

| Agent               | Model  | Effort | When                                                                                                 |
| ------------------- | ------ | ------ | ---------------------------------------------------------------------------------------------------- |
| `kit:planner`       | opus   | high   | a task of 5+ files or a new abstraction: a plan by layers before code                                |
| `kit:implementer`   | sonnet | high   | a step of an approved plan with a clear file boundary                                                |
| `kit:step-reviewer` | sonnet | high   | a plan step before `kit-commit`: is the step done, then the canon on the diff; read-only             |
| `kit:oracle`        | sonnet | high   | a critical plan step before its implementer: property or table tests from the plan, the second route |
| `kit:reviewer`      | opus   | xhigh  | once before a PR: the branch diff against `main`, read-only                                          |
| `kit:explorer`      | haiku  | high   | a broad code search when an answer is needed, not file contents                                      |

We do not run agents on every edit and do not spawn parallel ones without independent parts of the work.

## Boundaries

- **Before code — `grill-me`**, depth by the cost of a mistake: a one-file edit — no questions; a new abstraction, 5+
  files, a deviation from the canon — questioning with options and a recommendation: independent questions in a batch
  of up to four, dependent ones one at a time, as in `grill-me`.
- **Kit mode** — `canon` by default. Only the user changes it: `! kit-mode <mode>` or a message
  that is exactly `kit: <mode>` (for the phone, where `!` does not work); a call or a write through Bash is denied and
  rolled back (the file `.claude/kit-mode.local`, fallback source — `KIT_MODE`); you do not switch the mode yourself.
  `fast` — small edits without questioning, post-edit runs only prettier; `strict` — questioning one level deeper, Stop
  adds the related tests; `explore` — a prototype without questioning, Stop reports but does not block; `max` — as canon (the same gates, Stop and questioning), every agent on xhigh
  (`kit-config models --mode max`). "Done" in any mode — by the last item.
- **Tests mode** — `tests` in `kit.json`: `on` by default. Only the user changes it: `! kit-config tests <mode>` or a message
  that is exactly `kit: tests <mode>` (`on`, `later` or `off`); an agent's call through Bash is denied and a change of
  the key through Bash is rolled back; you do not switch it yourself, not even on a red gate. With `later` or `off` the test gates and the second route are
  off and new code ships without tests: `later` leaves a test-lane card per feature, `off` none. The tests that exist
  keep running.
- A conflict between the task and the canon — say it before the code. Bypassing a gate by editing the linter config is
  not allowed: a rule changes only by the user's decision.
- **Boy-scout:** a file you touch is brought to the canon; a file you do not touch stays untouched. Large debt outside
  the task boundary — a finding in the report, not an edit.
- Wait for confirmation for: a new domain, deletion, irreversible changes, canon edits.
- Prettier formats through a hook — we do not align by hand.
- Done = `pnpm verify` green **and** the scenario checked (`verify`). Anything unchecked is called unchecked.

<!-- /kit:core -->
