---
paths:
  - 'src/**'
  - 'tooling/**'
  - 'mocks/**'
  - 'e2e/**'
  - 'scripts/**'
---

# Architecture canon

How the code is built and why. How to write specific code — `code.md`, styles — `styles.md`. Facts about the current
code (what the public API already has) — `docs/context/public-api.md`, built by `pnpm context`.

## 0. The criterion that settles disputes

**Architecture is the cost of the next change.** A good decision is one after which a new feature touches few files, and
it is clear which ones. Not the one that is "more correct by the book".

| Conflict                     | Who wins                                                                                                       |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------- |
| DRY vs YAGNI                 | **Knowledge** (a rule, formula, limit, text) — one place from the second occurrence. **Code** — from the third |
| DRY vs KISS                  | DRY through composition. If shared code needs a flag for every case, these are different things                |
| Extensibility vs YAGNI       | YAGNI, until a second implementation appears                                                                   |
| Splitting into files vs KISS | Split along axes of change, not by line count                                                                  |
| Layer purity vs speed        | Layers. The import rule is the cheapest and most valuable rule                                                 |

Diverging knowledge is a bug (two places compute a limit differently). Diverging similar markup is normal: a wrong
abstraction costs more than a repeat.

## 1. Layers (FSD 2.1, hybrid)

```
app → pages → widgets → features → entities → shared
```

Imports go strictly down. Slices of one layer do not import each other.

| Layer      | What lives there                                                                                        | What never lives there                     |
| ---------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `app`      | Providers, the router (`routing`), error boundaries, monitoring, global styles, layout                  | Business logic, screens                    |
| `pages`    | A whole screen: its own UI, scenario, screen state. Reads route parameters                              | Code a second page really needs            |
| `widgets`  | **Not created.** A block of one page lives in it, a reusable action lives in `features`                 | —                                          |
| `features` | A user action that 2+ pages need: a mutation with UI, a multi-step scenario                             | A whole domain, reading the route          |
| `entities` | **The contract with the server — always, even with one consumer:** api, DTO, mapper, queryOptions, keys | User actions, JSX more complex than a cell |
| `shared`   | Infrastructure without a subject area: http client, env, general-purpose texts, helpers, tokens         | Domain names and rules ("user", roles)     |

**Why hybrid.** The server contract is shared by nature: the list, the card and the form read it, and a forked mapper is
two truths about the back end. So it goes to `entities` right away. The screen, on the contrary, lives in `pages` until
a second page needs it (FSD 2.1 "pages first").

**Only the page reads a route parameter** (`getRouteApi(...)` in `pages/*/model`). Below it the id arrives as an
argument of type `string`, never `string | undefined` and never `''` by default. Eslint checks it by name: a function
parameter `id` or `*Id` is not optional and not `| undefined`, without an `''` default and without `?? ''`. The
exception is a dependent query, whose id comes from data and may be absent: only the entity factory
`xByOptionalIdQuery(id: string | undefined)` with `skipToken`.

`shared` folders have no public API — import by the direct file path (`@/shared/api/http-client`): a shared barrel
pulls the whole layer into the initial chunk.

## 2. Slice

- One `index.ts` at the root is the public API, explicit re-exports only. From outside, the slice is reachable only
  through it.
- Inside the slice — relative file paths, **never** through its own `index.ts` (an `api ↔ model` cycle).
- No barrels inside the slice: neither `ui/index.ts` nor `model/index.ts`.
- The barrel holds only what other slices import. The barrel is a contract, not an inventory of files (knip checks
  this).
- A domain has the same name on every layer, in the singular: `entities/user`, `pages/user-list`. A feature is named by
  its capability: `invite-user`, `export-report`.
- In `pages` a slice group folder may hold related slices (FSD 2.1). It has no `index.ts` and no segments; each slice in
  it keeps its own `index.ts` and is imported as `@/pages/GROUP/SLICE`.

## 3. Segments

| Segment   | What is inside                                                                      | What is never there                     |
| --------- | ----------------------------------------------------------------------------------- | --------------------------------------- |
| `ui/`     | Markup: data → JSX. Up to two `useState` of local UI (open/closed)                  | Requests, effects, calculations         |
| `model/`  | Hooks (`use-*.ts`): requests, mutations, screen state, handlers, forms; types       | JSX                                     |
| `lib/`    | Pure functions without React: labels, selections, transformations into a view model | Hooks, side effects                     |
| `config/` | Slice constants (`UPPER_SNAKE_CASE`) and the text catalog `texts.ts`                | Logic                                   |
| `api/`    | Only in an entity: transport, Zod DTO schemas, the DTO ↔ domain mapper              | Hooks, domain types (they are in model) |

Dependencies inside a slice: `ui → model → lib → config`; `model → api → (model types)`. `lib` and `model` do not import
`ui` — not even types.

A file is named after the domain, not the role: `model/user.ts`, not `model/types.ts`. Grouping inside a segment is by
screen (`ui/list`, `ui/details`), not by technical role.

`shared/ui` may hold UI behaviour — focus, scroll, gestures and other DOM effects — never data or server state.

**Where a constant lives:** one per slice — its `config/`; infrastructural without a domain (timeouts, `staleTime`) —
`shared/config`; a field limit the server also checks — the entity's `config/`. Interface text is a catalog key.

## 4. Where each kind of state lives

The order of choice: **URL first, then the server cache, then local state, and only at the end a store.**

| Kind      | What it is                                 | With what                                            |
| --------- | ------------------------------------------ | ---------------------------------------------------- |
| URL       | filters, pagination, sorting, tab, id      | TanStack Router: `validateSearch` with a Zod schema  |
| Server    | everything that can be refetched           | TanStack Query through the entity's `xQuery(params)` |
| Local UI  | open/closed, step, form draft              | `useState` in the page hook, TanStack Form           |
| Global UI | needed by several unrelated places (theme) | Zustand in `shared/lib` — only when it appears       |

Server data never lives in a store or in `useState`: that is a second source of truth. State that must survive F5 and
travel by link lives in the URL.

## 5. Data flow and the boundary with the back end

The diagram and the wire schema bullet below are the `openapi-rest` adapter (the default): a REST back end the project
does not own. The `trpc` and `trpc-query` adapters are described in the paragraphs after the bullets.

```
ui                  view model from the page hook, camelCase
 ↑
pages/*/model       useQuery(userListQuery(params)) / useSuspenseQuery(userByIdQuery(id))
 ↑
entities/*/model    queryOptions, key factory, mutations and cache events
 ↑
entities/*/api      fetchX → httpClient.get(path, { schema }) → toX(dto)   ← BOUNDARY: no snake_case above
 ↑
shared/api          httpClient (fetch): timeout, auth, refresh, ApiError, schema check
```

- `openapi-rest`: the response is checked by the wire Zod schema in `api/*-dto.ts` the same way in dev and prod: a mismatch →
  `ApiError` `schema`, reported by `QueryCache` (a mutation response — by `MutationCache`). Tolerance lives in the schema
  and per field, not per whole response: a field the screen can live without — `.catch(null)` and `| null` in the
  domain (the screen shows "—"); a list item — `lenientArray(schema, 'xPageDto.items')` from
  `shared/api/lenient-array`: broken items are dropped and go to Sentry as one event per response, the list lives on.
  Fields without which the entity does not exist (id, name, email, role) are strict: a card without them is a screen
  error, a list item without them is dropped.
- The mapper `api/*-mapper.ts` is the only place that knows the back-end format. A field renamed on the back end = a
  mapper edit.
- DTOs do not leave `api/`. Domain types — `model/<domain>.ts`; a draft to send — `z.output` of the schema in `model/`.
- Errors are one type `ApiError` (`kind`, `status`, `code`, `fields`). Above `shared/api` there is no `Response` and no
  `TypeError`.
- An entity exposes a **query object** (`userByIdQuery(id)`), not a hook per way of reading: `useQuery`,
  `useSuspenseQuery`, the router `loader` and `prefetchQuery` all accept it.
- A mutation does not list keys itself: it calls the entity's cache event (`putUserInCache`, `dropUserFromCache`).
  Which queries an event touches is known in one place.

**The `trpc` adapter** (the own back end of the project in `server/`) keeps the same boundary with another transport.
`entities/*/api/*-api.ts` calls `trpcClient` from `shared/api/trpc-client` instead of `httpClient`, and there is no wire
Zod schema and no `*-dto.ts`: the router types the output (`inferRouterOutputs<AppRouter>`), and the input check lives
on the server. The mapper `api/*-mapper.ts` still turns the output into the domain type, so nothing above `api/` sees
the router shape. Errors still become `ApiError`: `callTrpc` maps a `TRPCClientError` to `kind`, `status`, `code` and
`fields`, a failed `fetch` to `network`, and rethrows an abort. The rest of the section (query objects, cache events,
`ApiError` above `shared/api`) holds unchanged.

**The `trpc-query` adapter** (a project whose screens read the router through the tRPC options proxy) extends `trpc` and
puts the proxy in the entity. There is no `entities/*/api/*-api.ts`: the proxy `trpc` of `shared/api/trpc-query.ts`
hands out the keys (`queryKey`, `pathKey`, `mutationKey`), and `model/*-keys.ts`, `model/*-queries.ts`,
`model/*-mutations.ts` and `model/*-cache.ts` are built on them. The calls still go through `trpcClient`, `callTrpc` and
`withTimeout(signal)`, so `ApiError` and the request timeout hold. The query objects stay plain functions (the router
`loader` calls them outside React), `select` turns the router output into the domain type through `api/*-mapper.ts`,
and the hooks and cache events keep their names, so pages, app and the entity `index.ts` do not change. The proxy is
built at module level and needs no React provider. Only an adapter whose `pack.json` entry says `model: true` may put
files under `entities/*/model`; the kit's check of the packs names an entity that has none.

**The shared root domain.** The knowledge a client and a server share (value objects, limits, catalogs) lives in the
root `domain/` of a project with its own back end, and imports nothing outside `domain/`. `src/` imports it through the
alias `@/domain/*` (a `tsconfig.app.json` path and a `vite.config.ts` alias, both placed before `@`), not by a relative
path out of `src/`.

## 6. Prohibitions and their reasons

| Forbidden                                                  | Why                                                                    |
| ---------------------------------------------------------- | ---------------------------------------------------------------------- |
| Importing into another slice past its `index.ts`           | the slice contract stops being a contract                              |
| `index.ts` inside a slice, `export *`                      | hides the graph, breaks tree-shaking and knip, gives cycles            |
| A domain name in `shared`                                  | `shared` becomes a node everything depends on                          |
| A request, effect or calculation in `ui/`                  | the component stops being a function of props                          |
| `queryOptions`/`useMutation` outside `entities/*/model`    | keys, transport and invalidation scatter across screens                |
| A query key as an inline array                             | invalidation silently misses                                           |
| `queryFn: fetchX` as a direct reference                    | with an import cycle it becomes `undefined` in the prod bundle         |
| `fetch`/`axios` outside `shared/api`                       | bypasses timeout, auth, error normalization and the schema             |
| `as` in a mapper and on external data                      | disables the only completeness check of the object                     |
| `catch` without a parameter or with an unused one          | network, 500 and a broken schema become one fallback                   |
| `new Date()` outside `shared/lib/date.ts`                  | five ways of parsing give five results                                 |
| An interface string in code                                | cannot be found or translated; text is a catalog key                   |
| `useState` for what belongs in the URL or the cache        | breaks the link, "back" and the cache                                  |
| An abstraction with a single implementation, `enum`, `any` | repair costs more than a repeat; a union and `unknown` are more honest |

## 7. A rule without an automatic check does not exist

Every rule of this canon is checked by a machine, and `docs/gates.md` lists each one with its check and the test behind
it. A rule described here but missing there is a candidate for an automatic check or for removal.

## 8. Deviations from the canon

A deviation is allowed when the canon costs more than it gives, and it is always **explicit**: a line in
`docs/deviations.md` — where, from what, why. No marks in the code. An exception to a linter rule is only a path
override in `eslint.config.js` or `stylelint.config.mjs` plus a line in `docs/deviations.md`. A comment that disables a
canon rule fails lint; over any other rule a disable names the rule and its reason after `--`. Repeated twice — fix the canon, not the code.
Anything irreversible (data schema, public contract, deletion) — only after the user confirms. A silent deviation is
worse than a violation: a violation is visible to the linter, a silent one to no one.

## 9. An existing project

The kit is also rolled onto a live project: the `adopt` skill runs the procedure through `create-kit adopt`, and the
transitional mode (new code complies, old code is fixed on touch, a baseline only shrinks, a move is separate from an
edit) is in its doc. What is web-react:

One rule for every check: the debt of the project is frozen in a baseline file, and the baseline only shrinks.

| Check         | Script (verify) | Freeze                  | Baseline file              | Key                                   |
| ------------- | --------------- | ----------------------- | -------------------------- | ------------------------------------- |
| types         | typecheck       | typecheck:baseline      | `tsc-baseline.json`        | file, TS code, message                |
| ESLint        | lint            | lint:baseline           | `eslint-suppressions.json` | ESLint's own: file, rule, count       |
| stylelint     | lint:styles     | lint:styles:baseline    | `stylelint-baseline.json`  | file, rule, text                      |
| steiger       | lint:arch       | lint:arch:baseline      | `steiger-baseline.json`    | path, rule, message                   |
| placement     | lint:placement  | lint:placement:baseline | `placement-baseline.json`  | file, rule, first line of the message |
| design tokens | tokens:check    | tokens:baseline         | `design-baseline.json`     | file, id, subject and context         |
| knip          | knip            | knip:baseline           | `knip-baseline.json`       | file, kind, name                      |

- A freeze without its file writes every current finding (the file is written even when empty: its presence means the
  project freezes that check). With the file it lowers each count to the current one and drops zeros, never adding.
- The check fails on a finding past the file, and reports "N fixed … still counted in FILE: run pnpm FREEZE" when a
  frozen finding is gone. A check without its file fails on every finding.
- A rule with debt stays an error: there is no `warn` level, its debt is frozen in the baseline. A rule is clean when
  its baseline holds no entry for it.
- A freeze refuses a finding in a kit file the project left as the kit wrote it (a path of `files` in `kit-lock.json`
  whose content still hashes to the lock value, or that has its `PATH.kit-new` beside it): the kit file is fixed, or its `.kit-new` resolved, first. A file the
  project kept as its own or merged no longer hashes to the lock and freezes like its code.
- After a kit update, `pnpm debt:sync` freezes the debt of the rules the update brought: it writes the current findings
  of every check that has its baseline file, and only while nothing outside the kit files changed (it refuses and names
  the paths otherwise: commit your own changes first). It is the only way a baseline grows.
- A project without `src/shared/styles/tokens/config.json` has the design checks off until its tokens are DTCG data.
- The gates judge the files a session or a step touched by what is new: a file with frozen debt passes until it adds a
  finding.
