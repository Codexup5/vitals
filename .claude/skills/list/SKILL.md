---
name: list
description: List screen — filters, search and pagination in the URL through validateSearch, the five screen states as a view model, a table. Use for any list screen. Triggers «список», «фильтр и пагинация», «сохранялись в ссылке».
---

The reference is `src/pages/user-list`.

## 1. The URL is the source of truth

The parameter schema lives in the entity (`model/<domain>-list-search.ts`); the route's `validateSearch` uses it:

```ts
export const ORDER_LIST_SEARCH_DEFAULTS = { page: 1, search: '' } as const;

export const orderListSearchSchema = z.object({
  page: z.number().int().positive().default(ORDER_LIST_SEARCH_DEFAULTS.page).catch(ORDER_LIST_SEARCH_DEFAULTS.page),
  search: z
    .string()
    .trim()
    .max(ORDER_SEARCH_MAX_LENGTH)
    .default(ORDER_LIST_SEARCH_DEFAULTS.search)
    .catch(ORDER_LIST_SEARCH_DEFAULTS.search),
  status: z.enum(ORDER_STATUSES).optional().catch(undefined),
});
```

`.default` makes a parameter optional in links, `.catch` repairs a hand-edited URL instead of failing. Filters are never
duplicated in `useState`.

Defaults are not written to the URL: the route sets
`search: { middlewares: [stripSearchParams(ORDER_LIST_SEARCH_DEFAULTS)] }` — the same values as the schema, from one
object. An empty filter and the first page give `/orders`, not `/orders?page=1&search=`.

## 2. The page hook — `model/use-x-list-page.ts`

- `getRouteApi('/xs').useSearch()` / `.useNavigate()`.
- `useQuery(xListQuery({ ...search, pageSize: X_LIST_PAGE_SIZE }))`.
- Submitting a filter — `navigate({ search: xListSearchSchema.parse(formValues) })`: the same schema as the route's; a
  new filter resets the page to 1.
- Returns `{ view, filters, handleFilterSubmit, resetFilters, goToPage, retry }`.

## 3. View model — `lib/x-list-view.ts` + test

A discriminated union:
`loading | error | empty(hasFilters) | pastLastPage(lastPage) | ready(rows, page, pageCount, total, isRefreshing)`.

- There is data, but the refetch failed — `ready` (show what there is).
- `empty` with filters — "nothing found" + "Reset filters"; without filters — an explanation + the main action.
- `pastLastPage` — no rows, and `page > pageCount`: the page from the URL is past the end of the list (an old link,
  records were deleted). A `goToPage(lastPage)` button on click, no redirect effect: the user changes the URL, "back"
  does not loop.
- No rows, `total > 0`, and the page is within the list — `lenientArray` dropped every row: this is `error` with a text
  about the data, not `pastLastPage` (the button would lead to the same page).
- Rows are ready to show: enum labels, formatted dates.

## 4. Markup

- The filter form is uncontrolled with a `key` from the URL values — it resets on navigation.
- `table` with `caption`, `th scope="col"`, a link to the card — `Link` with `params`. `aria-busy` during a refetch.
- Pagination — `nav aria-label`, "Back/Next" buttons with `disabled` at the edges.

## 5. Test

A scenario through the app: list → search → the URL updated → "nothing found" → reset; a 503 response →
`role="alert"`.
