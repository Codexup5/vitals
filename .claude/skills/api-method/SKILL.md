---
name: api-method
description: New API method — the endpoint, the Zod response schema, the DTO ↔ domain mapper, the mapper test. Use when adding or changing a request to the back end. Triggers «новый метод API», «подключи эндпоинт», «DTO и маппер».
---

The boundary with the back end is one schema file and one mapper per entity. Above `api/` the back-end format does not
exist.

## 1. The wire schema — `api/<domain>-dto.ts`

Write it from a **live response**, not from the documentation: open the response in DevTools or ask the back end.

```ts
export const orderDtoSchema = z.object({
  id: z.string(),
  total_minor: z.number().int(),
  status: z.enum(ORDER_STATUSES),
  paid_at: z.iso.datetime({ offset: true }).nullable().catch(null),
});
export const orderPageDtoSchema = z.object({
  items: lenientArray(orderDtoSchema, 'orderPageDto.items'),
  total: z.number().int(),
});
export type OrderDto = z.infer<typeof orderDtoSchema>;
```

- `z.object`, not `strict`: extra fields are not an error and do not get into the type.
- The schema is strict in dev and prod: a mismatch → `ApiError` `schema`, reported by `QueryCache` (a mutation response
  — by `MutationCache`). Tolerance is per field: a field the screen can live without — `.nullable().catch(null)` and
  `| null` in the domain (the screen shows "—"). Fields without which the entity does not exist (id, name, status) stay
  strict.
- A list — `lenientArray(itemSchema, 'xPageDto.items')` from `shared/api/lenient-array`: broken items are dropped and go
  to Sentry as one event per response (the label says where the list is in the schema), the list lives on.
- No `transform`: the schema describes the wire, translation is the mapper's job. The exception is a `transform` inside
  `lenientArray`.
- The request body is a `*DraftDto` as a `Pick` of the inferred response type (`Pick<UserDto, 'first_name' | …>`); we do
  not write the type by hand (code.md §7).

## 2. The mapper — `api/<domain>-mapper.ts`

```ts
export const toOrder = (dto: OrderDto): Order => ({
  id: dto.id,
  total: dto.total_minor,
  status: dto.status,
  paidAt: dto.paid_at === null ? null : parseIsoDateTime(dto.paid_at),
});
```

- The object is built in full; `as` is forbidden (eslint `no-cast-in-mapper`).
- Dates — `parseIsoDateTime`, money — whole minor units. Formatting is not here but in the page's view model.
- List parameters → query: `toXListQuery(params)`; `httpClient` drops empty values itself.

## 3. Transport — `api/<domain>-api.ts`

```ts
export const fetchOrderById = async (orderId: string, signal?: AbortSignal) =>
  toOrder(await httpClient.get(`/orders/${encodeURIComponent(orderId)}`, { schema: orderDtoSchema, signal }));

export const cancelOrder = (orderId: string) =>
  httpClient.send('POST', `/orders/${encodeURIComponent(orderId)}/cancel`);
```

- The name is in the language of the domain (`cancelOrder`), not HTTP (`postOrderCancel`).
- `signal` is always passed through, an id in the path goes through `encodeURIComponent`.
- A response without a body — `httpClient.send` / `httpClient.delete`.

## 4. Back-end errors

`ApiError.fields` expects the body `{ code, message, fields: { field: text } }` (`shared/api/api-error.ts`). If the
back-end format differs, one function `toHttpApiError` changes, not the handlers by code.

## 5. Test and mocks

- `api/<domain>-dto.test.ts`: a broken optional field → `null`, a broken list item is dropped and the rest stay; a
  response without a required field does not pass.
- `api/<domain>-mapper.test.ts`: the full object, `null` fields → `null`, reverse mapping of the draft.
- An MSW handler in `mocks/handlers.ts` in the wire format + an error response.

## 6. A project with the trpc adapter

Applies when `src/shared/api/trpc-client.ts` exists; sections 1, 4 and 5 (the wire schema, `toHttpApiError`, MSW
handlers) do not apply.

1. The server module and the procedure first: input schema, handler, the router entry — by `.claude/rules/server.md`.
2. The entity api through `trpcClient`: wrap the call in `callTrpc` and pass the signal with `withTimeout(signal)`; an id
   goes in the input object.

```ts
export const fetchOrderById = async (orderId: string, signal?: AbortSignal) =>
  toOrder(await callTrpc(trpcClient.order.byId.query({ id: orderId }, withTimeout(signal))));
```

3. The mapper takes `inferRouterOutputs<AppRouter>['order']['byId']` and builds the domain object in full, no `as`;
   dates through `parseIsoDateTime`.
4. No DTO schema and no MSW handler. The mapper test builds the router output by hand; the screen scenario goes through
   the test hub.

## 7. A project with the trpc-query adapter

Applies when `src/shared/api/trpc-query.ts` exists; section 6 holds, except for step 2: there is no
`entities/<domain>/api/<domain>-api.ts`, the call is part of the query or the mutation in `entities/<domain>/model/`.

```ts
export const orderByIdQuery = (orderId: string) =>
  queryOptions({
    queryKey: ORDER_KEYS.detail(orderId),
    queryFn: ({ signal }) => callTrpc(trpcClient.order.byId.query(toIdInput(orderId), withTimeout(signal))),
    select: toOrder,
    staleTime: STALE_POLICY.detail,
  });
```

- The key comes from the proxy (`trpc.order.byId.queryKey(input)` in `model/order-keys.ts`), so the cache entry holds
  the router output and `select` gives the screen the domain type.
- The mapper is the one of section 6, step 3; the mapper test builds the router output by hand.
