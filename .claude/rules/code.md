---
paths:
  - 'src/**'
  - 'tooling/**'
  - 'mocks/**'
  - 'e2e/**'
  - 'scripts/**'
---

# Code practices

How to write specific code. Where to put files — `architecture.md`. The reference to copy is `entities/user` and
`pages/user-*`: when in doubt, take the reference, not the neighbouring file.

## 1. A component is a function from props to markup

A component calls **one hook of its screen** and draws what it returned. `ui/` has no requests, effects, memo, storage,
calculations by domain rules, IIFEs, interface strings and no more than two `useState`.

The screen hook returns **ready-made** things: a view model (`{ status: 'ready', rows }`), labels, `canEdit`, actions by
meaning (`openAdd()`, `confirm()`), not raw setters. Guideline: a component up to ~150 lines, a hook up to ~80 — more is
a reason to check whether responsibilities are mixed.

A list `key` is the id of the item, never the index of `map` (eslint). A button is the `Button` of `shared/ui` (eslint).

## 2. Hooks

- One responsibility. The name says what it gives: `useUserListPage`, `useUserForm`. The file — `use-user-list-page.ts`.
- Return an object when there are more than two fields. A side effect goes in the name.
- A function without hooks inside is not a hook; it belongs in `lib/`.
- A hook that only calls another hook and adds nothing is not needed: the component calls the original itself.
- The entity id is an argument of type `string` from the page, not `useParams` inside.

## 3. `useEffect` — only to sync with the outside world

Subscriptions, DOM APIs, timers. Everything else is not an effect:

| Instead of an effect           | Do it like this                         |
| ------------------------------ | --------------------------------------- |
| derived state                  | compute during render                   |
| reacting to an event           | a handler or the mutation's `onSuccess` |
| loading data                   | TanStack Query                          |
| syncing a form with a response | `defaultValues` / `key` on the form     |

## 4. Requests (TanStack Query)

- Three transports, one shape above `api/`: `httpClient` with a wire schema (`openapi-rest`), `trpcClient` through
  `callTrpc` with the router-typed output (`trpc`), or the same `trpcClient` called from `entities/*/model` with keys
  from the options proxy of `shared/api/trpc-query.ts` (`trpc-query`). The queries, keys, mutations and cache events
  below are the same for all three.
- Reading — `xQuery(params)` from the entity (`queryOptions`). Keys — an `X_KEYS` factory with the hierarchy
  `all → lists → list(params)`, `details → detail(id)`: invalidate a level (`lists()`), not a key with parameters.
- `queryFn` — always a closure `({ signal }) => fetchX(params, signal)`: the abort signal reaches `fetch` (with `trpc` and `trpc-query`, through
  `withTimeout(signal)`).
- Screen data with a known id — `useSuspenseQuery` + the route `loader` (`queryClient.query({ ...xQuery(id),
staleTime: 'static' })`). Lists — `useQuery` with `placeholderData: keepPreviousData`: "partial" is a working state
  there. A conditional request — `skipToken`, not a fake id.
- Preloading on intent — the router's `defaultPreload: 'intent'`: hovering a link starts the `loader`.
- `staleTime` — only a `STALE_POLICY` level (`shared/config/query.ts`), no numbers in place.
- A mutation is an entity hook (`useCreateUser`); `onSuccess` returns the cache event's promise so that `isPending`
  covers the refetch too. "Not found" is `ApiError` 404, not `null`.

## 5. Errors

Expected ones (validation, 404, 409) are part of the scenario and are shown in context. Unexpected ones go to the error
boundary and to Sentry (`reportError`); `QueryCache.onError` in `app` reports 5xx and a schema mismatch by itself.

| What happened               | Where we show it                                         |
| --------------------------- | -------------------------------------------------------- |
| Field error                 | under the field (`putServerErrors` puts back-end errors) |
| Form submission error       | next to the submit button, `role="alert"`                |
| Screen loading              | the route boundary: what happened + "Retry"              |
| An independent screen block | inside the block, the screen lives on                    |

`catch` always names the error and does something with it: narrows it (`isApiError`), shows it or rethrows it; a promise
`.catch(…)` handler names its error too. The error
text explains what happened and what to do: not "Error 500", but "The server could not process the request. Try again
in a minute".

## 6. Five screen states

| State   | What we show                                                             |
| ------- | ------------------------------------------------------------------------ |
| Loading | a static skeleton shaped like the content; no pulsing, not a spinner     |
| Empty   | an explanation and the main action; "nothing found" ≠ "nothing here yet" |
| Error   | what happened and what to do                                             |
| Partial | what loaded; a failed block has its own boundary                         |
| Ready   | the content                                                              |

States are a discriminated union from `lib/` (`toUserListView`), not a set of flags in the component.

## 7. Types

- No `any`: `unknown` + narrowing. `as` — only with a comment on why it is safe; on external data — Zod, not `as`.
- Impossible states are unrepresentable: a union instead of flags. A map of screens is indexed by a union key, not
  `string`.
- A string union or an `as const` array instead of `enum`; the type is derived from the array: `(typeof ROLES)[number]`.
- Types come from schemas (`z.infer`/`z.output`), not duplicated by hand. `import type` — `verbatimModuleSyntax` is on.

## 8. Naming

| What                    | Case             | Example                 |
| ----------------------- | ---------------- | ----------------------- |
| Variables, functions    | camelCase        | `getUserFullName`       |
| Components, types       | PascalCase       | `UserTable`, `ApiError` |
| Constants               | UPPER_SNAKE_CASE | `USER_LIST_PAGE_SIZE`   |
| Component file          | PascalCase.tsx   | `UserTable.tsx`         |
| Other files and folders | kebab-case       | `user-mapper.ts`        |

Booleans — `is`/`has`/`can`/`should`/`was`. Functions — verb + noun (`toUser`, `fetchUserPage`). A callback prop is
`onX`, its implementation `handleX`. Empty words are forbidden: `data`, `info`, `temp`, `utils`, `helper`, `manager`,
`processData`; `data` from a query hook is renamed: `const { data: user }`. Types have no `I` prefix.

## 9. Forms — TanStack Form + Zod

- The schema is the single source of truth: validation comes from it (`validators.onDynamic` + `revalidateLogic()`:
  checked on submit, after that on change) and so does the values type. Limits come from the entity's `config/`, the
  same ones the server checks.
- Logic — in `use-x-form.ts`, markup — in the component. Back-end field errors go into the form (`putServerErrors`), the
  rest as text next to the button. Editing a field clears its server error (`clearServerError` in
  `listeners.onChange`).
- Input fields are strings; the schema turns them into data (`trim`, `pipe`).

## 10. Dates, texts, money

- Dates are created and parsed only in `shared/lib/date.ts`; display — `Intl.DateTimeFormat` with `APP_LOCALE`. The
  model carries `Date`, the view model (`lib/`) formats it, not the mapper.
- There are no interface strings in code (eslint: Cyrillic outside `config/texts.ts`). A slice catalog is
  `X_TEXTS … as const`, a text with substitution is a function in the catalog, plural forms — `pluralize`. General words
  without a domain — `COMMON_TEXTS`.
- One meaning — one place; the same word with different meanings — different keys.
- Money — whole minor units, the calculation is a pure `lib/` function under test.

## 11. Confirmations

Anything irreversible (deletion, sending money) asks again in place of the button; anything reversible runs right away. A
server error stays next to the action.

## 12. Tests

- Vitest: mappers, schemas, view models, calculations (`*.test.ts` next to the file).
- Testing Library + MSW: screen scenarios through the whole app (`app/test/render-app.tsx`), selectors by role,
  `getByTestId` as a last resort. The network is mocked with MSW, our own modules are not mocked. Test behaviour, not
  implementation.
- Playwright: 3–7 smoke scenarios on a mobile and a desktop screen (`e2e/`).
- A bug in `lib/`, a mapper or a schema — a failing test first, then the fix.
- `kit.json` `tests` is `on`, `later` or `off`. With `later` or `off` new code ships without tests and the test gates are
  off; existing tests keep running. `later` leaves a test-lane card per feature, `off` none.

## 13. File structure and comments

Import order is kept by prettier: external → `@/*` → relative → styles. Inside a component: data → state → derived
values → handlers → effects → early returns → markup. Named exports; `default` only where a tool requires it (outside `src/`, eslint).

A comment only in two cases: a non-obvious algorithm (one or two lines) and a workaround for an external limitation
(browser, back end, library — otherwise the next person "fixes" the workaround). We do not write: retelling of the code,
decision history, commented-out code, banners, `TODO` without a task number (eslint).
