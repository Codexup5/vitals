# Process notes

The web-react parts of the working process: what to check, where to look for a bug, what to check before and after a dependency, what
a review looks at. The core skills (`verify`, `debug`, `search-first`) and the `reviewer` agent keep the generic steps
and send you here.

## Checks by kind of change

| What changed               | How I check                                                             |
| -------------------------- | ----------------------------------------------------------------------- |
| mapper, schema, `lib/`     | Vitest on the changed function; in `strict` also `pnpm mutation`        |
| query, cache, invalidation | list -> edit -> back: the data updated without F5                       |
| form                       | empty submit, a backend error under the field, a repeat submit          |
| route, lazy screen         | `pnpm build` and `pnpm size`: the screen is not in the start chunk      |
| styles                     | narrow (390 px) and wide screen, dark theme                             |
| move, deletion             | `pnpm typecheck`, `pnpm knip`, a search for string mentions of the path |
| key scenario               | `pnpm test:e2e`                                                         |

`pnpm mutation` (Stryker on `lib/`, mappers, DTOs and schemas, threshold 80%, ~3 min) shows whether a test catches a
breakage: a surviving mutant is a line the test does not check. You do not lower the threshold - you write the test.
Outside `strict` it runs weekly in CI (`.github/workflows/mutation.yml`).

The table assumes `tests` is `on` in `kit.json`. With `later` or `off` new code ships without tests, existing tests keep
running in `pnpm verify`, and e2e and mutation wait until `kit-config tests on`.

A screen - walk the scenario through Playwright MCP on `pnpm dev`: the steps, a console without errors, a network without
extra requests. The browser runs in Docker (headless) and opens the dev server at `localhost` through the MAP rule of
`.mcp.json`; Docker Desktop must be running.

## Debugging boundary

- A production error - Sentry first (MCP `sentry`): stack, release, frequency, browser.
- A screen - Playwright MCP (headless browser in Docker, `localhost` mapped to the host): walk the steps, capture the
  console and the network.
- Where the boundary is: network response (DevTools / MSW) -> `ApiError`/schema -> mapper -> cache (the key!) -> hook ->
  markup. Check each layer against a fact, not by reading code.
- Common causes: the query key did not match the invalidation; `queryFn` without a closure; a `useState` copy of server
  data; a Zod schema stricter than the response (in dev - `kind: 'schema'`); a race between navigation and a mutation.
- A bug in `lib/`, a mapper or a schema - first a **red** Vitest test that reproduces the bug, then the fix until green.
- A scenario bug - a test through the app (`app/test`) or Playwright.

## Before and after adding a dependency

Before: check the native options: `Intl`, `URLSearchParams`, `AbortSignal.timeout`, `structuredClone`, CSS. If none
fits, after installing run `pnpm size` before and after, then `pnpm knip`.

## Review list

What the branch reviewer looks at on top of its generic items:

- **Data:** a DTO above `api/`; `as` on external data; a mutation without a cache event; a key outside the factory;
  `useState` for what belongs in the URL or the cache.
- **Screen:** all five states; an error with a "what to do" text; a `catch` that swallows.
- **Tests:** logic in `lib/`, a mapper or schema without a test (with `tests` later in `kit.json`, a missing test
  is a test-lane card; with off, nothing: not a defect).
- **Accessibility:** a `div` with `onClick`, a field without a `label`, an error shown by color only.
- **Security** (you note it; only what breaks data blocks): secrets in code and `VITE_*`, a token in `localStorage`,
  `dangerouslySetInnerHTML` without `DOMPurify`, personal data in logs and Sentry, unvalidated input, an open
  redirect, a new dependency without a check.
