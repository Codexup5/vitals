---
name: new-domain
description: New domain — an entity (api, DTO, mapper, queryOptions, mutations) and list, card and form pages after the user reference. Use when a new back-end resource appears. Triggers «новый домен», «новая сущность», «список, карточка и форма по эталону user».
---

The reference is `src/entities/user` and `src/pages/user-*`. Copy the structure, change the content. Before starting —
`grill-me` (a new domain needs confirmation) and `plan`.

## 1. Entity `src/entities/<domain>/`

```
api/<domain>-dto.ts        Zod wire schemas (snake_case), *Dto types through z.infer
api/<domain>-mapper.ts     toX(dto), toXPage(dto), toXDraftDto(draft), toXListQuery(params) — no `as`
api/<domain>-mapper.test.ts
api/<domain>-api.ts        fetchXPage, fetchXById, createX, updateX, deleteX → httpClient + mapper
config/<domain>-limits.ts  field limits (the same as on the server)
config/texts.ts            X_TEXTS: field labels, enum values, validation texts
model/<domain>.ts          domain types (camelCase), a union from an `as const` array
model/<domain>-draft.ts    Zod schema of the form draft, UserDraft = z.output
model/<domain>-list-search.ts  schema of the list URL parameters for validateSearch
model/<domain>-keys.ts     X_KEYS: all → lists → list(params), details → detail(id)
model/<domain>-queries.ts  xListQuery(params), xByIdQuery(id) through queryOptions
model/<domain>-cache.ts    cache events: putXInCache, dropXFromCache
model/<domain>-mutations.ts useCreateX, useUpdateX(id), useDeleteX(id)
index.ts                   only what the pages import
```

Methods and schemas — the `api-method` skill; queries, keys and events — `query`.

## 2. Pages

- `pages/<domain>-list` — the `list` skill.
- `pages/<domain>-details` — `useSuspenseQuery(xByIdQuery(id))`, a `loader` in the router, a skeleton with the same
  frame, deletion with confirmation in place of the button.
- `pages/<domain>-form` — the `form` skill; two pages (create and edit) share one `use-x-form.ts`.

## 3. Routes `src/app/routing/router.tsx`

```ts
const xListRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'xs',
  validateSearch: xListSearchSchema,
  component: lazyRouteComponent(() => import('@/pages/x-list'), 'XListPage'),
});
```

The component and the skeleton only through `lazyRouteComponent`; a static import from a page pulls it into the initial
chunk.

## 4. Mocks and tests

- `mocks/<domain>-db.ts` + handlers in `mocks/handlers.ts` — in the wire format (snake_case), with 404 and 422.
- Vitest: the mapper, the draft schema, the search schema, the list view model.
- A scenario in `src/app/test/*.test.tsx`: list → filter → card → form with a back-end error.
- If the domain is a key one — a Playwright scenario in `e2e/`.

## 5. Done

`pnpm verify` is green, `pnpm context` updated `docs/context/public-api.md`, the scenario is walked in the browser
(`verify`). The first real domain then changes the reference (section 7).

## 6. A project with the trpc adapter

Applies when `src/shared/api/trpc-client.ts` exists. The server comes first, the front end follows:

1. The server module and its procedures — by `.claude/rules/server.md`. The router (`server/router.ts`) types the output.
2. `api/<domain>-api.ts` calls `trpcClient.<domain>.<procedure>` through `callTrpc(...)` with `withTimeout(signal)`; the
   method names stay in the language of the domain.
3. `api/<domain>-mapper.ts` turns `inferRouterOutputs<AppRouter>['<domain>']` into the domain type, no `as`.
4. No `api/<domain>-dto.ts`, no DTO schema and no MSW handler in `mocks/handlers.ts`: tests reach the real router through
   the test hub.

The entity `model/`, the pages, routes and tests of sections 2 and 3 are the same.

## 6a. A project with the trpc-query adapter

Applies when `src/shared/api/trpc-query.ts` exists. Section 6 holds for the server, the mapper and the missing DTO and
MSW handler; the transport moves from `api/` into `model/`:

1. No `api/<domain>-api.ts`. The calls live in the model files, through `trpcClient`, `callTrpc` and `withTimeout(signal)`.
2. `model/<domain>-keys.ts` takes `X_KEYS` from the proxy: `trpc.<domain>.list.pathKey()`, `queryKey(input)`; the input
   builders (`toListInput`, `toIdInput`) sit beside them.
3. `model/<domain>-queries.ts`: `xListQuery(params)` and `xByIdQuery(id)` stay plain functions over `queryOptions`, with
   `select` mapping the router output to the domain type through `api/<domain>-mapper.ts`.
4. `model/<domain>-mutations.ts` keeps `useCreateX`, `useUpdateX(id)`, `useDeleteX(id)` with `mutationKey` from the proxy;
   `model/<domain>-cache.ts` writes the router output under the detail key.
5. The pages, routes and the entity `index.ts` are the same as in the other adapters.

## 7. Changing the reference

When the first real domain is built by this skill and `pnpm verify` is green:

1. The reference moves to it: the reference row of `CLAUDE.md` "What to read" (and `server/modules/user` in a trpc
   project), the reference lines of `.claude/rules/code.md` and of the `new-domain`, `form` and `list` skills. The
   reference lives in more than one place: the `api/` of the default adapter, the overlay `api/` of every adapter
   (`adapters/trpc`), the `model/` files of the trpc-query adapter (`adapters/trpc-query`) and, in a swift project, the
   server context `server/modules/user`. A domain that lands in one of them only is incomplete for the others.
2. `user` goes whole, in a commit of its own after the move: `src/entities/user`, `src/pages/user-*`, their routes in
   `src/app/routing/router.tsx`, `mocks/user-db.ts` and its handlers in `mocks/handlers.ts`, `e2e/user.spec.ts`, the
   scenarios in `src/app/test`, and in a trpc project `server/modules/user` with its router entry.
3. `pnpm context`, then `pnpm verify` and the harness audit are green (the audit fails on a `CLAUDE.md` path that no
   longer exists).
