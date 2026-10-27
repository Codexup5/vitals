---
name: query
description: TanStack Query reads and mutations — queryOptions, the key factory, cache events, loader and preloading, optimistic updates. Use for any read or write of server data. Triggers «запрос», «мутация», «инвалидация», «кеш не обновился».
---

Everything is declared **in the entity** (`entities/<domain>/model`); pages only call it.

## Keys — `model/<domain>-keys.ts`

```ts
export const ORDER_KEYS = {
  all: () => ['order'] as const,
  lists: () => [...ORDER_KEYS.all(), 'list'] as const,
  list: (params: OrderListParams) => [...ORDER_KEYS.lists(), params] as const,
  details: () => [...ORDER_KEYS.all(), 'detail'] as const,
  detail: (orderId: string) => [...ORDER_KEYS.details(), orderId] as const,
};
```

The root is written once. Invalidate a **level** (`lists()`): a key with specific parameters silently misses.

## Reading — `model/<domain>-queries.ts`

```ts
export const orderByIdQuery = (orderId: string) =>
  queryOptions({
    queryKey: ORDER_KEYS.detail(orderId),
    queryFn: ({ signal }) => fetchOrderById(orderId, signal),
    staleTime: STALE_POLICY.detail,
  });
```

| Where                    | How                                                                         |
| ------------------------ | --------------------------------------------------------------------------- |
| a screen with a known id | `useSuspenseQuery(orderByIdQuery(id))` + the route `loader`                 |
| a route loader           | `context.queryClient.query({ ...orderByIdQuery(id), staleTime: 'static' })` |
| a list                   | `useQuery(orderListQuery(params))` with `placeholderData: keepPreviousData` |
| an optional id           | `useQuery(orderByOptionalIdQuery(id))`                                      |
| preloading on hover      | the router's `defaultPreload: 'intent'` — already on                        |

A dependent query, whose id comes from data and may be absent, gets its own factory next to `orderByIdQuery`:

```ts
export const orderByOptionalIdQuery = (orderId: string | undefined) =>
  queryOptions({
    queryKey: orderId === undefined ? ORDER_KEYS.details() : ORDER_KEYS.detail(orderId),
    queryFn: orderId === undefined ? skipToken : ({ signal }) => fetchOrderById(orderId, signal),
    staleTime: STALE_POLICY.detail,
  });
```

It is a second factory because `skipToken` inside `orderByIdQuery` breaks the types of `useSuspenseQuery`.

## Writing — `model/<domain>-cache.ts` + `model/<domain>-mutations.ts`

```ts
export const putOrderInCache = (queryClient: QueryClient, order: Order) => {
  queryClient.setQueryData(ORDER_KEYS.detail(order.id), order);
  return queryClient.invalidateQueries({ queryKey: ORDER_KEYS.lists() });
};

export const useCancelOrder = (orderId: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => cancelOrder(orderId),
    onSuccess: (order) => putOrderInCache(queryClient, order),
  });
};
```

- `onSuccess` returns the event's promise: `isPending` covers the refetch too.
- Deleting what the screen shows: leaving is an `onDeleted` callback in the options of the entity's `useMutation` hook
  (`useDeleteOrder(orderId, { onDeleted })`); `onSuccess` first awaits `onDeleted()`, then returns the cache event. The
  other way round, the `useSuspenseQuery` of the still-open screen refetches the deleted item and gets a 404. The
  `mutate(…)` callbacks do not fit here: they are not called after unmount.
- A mutation of another domain touches its own cache — the **page** calls that entity's event in the `onSuccess` of the
  `mutate(…, { onSuccess })` call: an entity does not import its neighbour.
- Optimistic update only where the response is predictable (a toggle, a mark): `onMutate` snapshot → `setQueryData`,
  `onError` rollback, `onSettled` cache event.

## Check

List → edit → back: the data updated without F5. `pnpm test` is green.
