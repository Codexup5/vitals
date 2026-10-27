import { http, HttpResponse } from 'msw';

import { userDb } from './user-db';

// Vitest setup files import these handlers before a test file's vi.mock runs; a static import would bind the real
// @sentry/react through user-dto → lenient-array → report-error, and per-file Sentry mocks would stop working
const parseUserDraft = async (request: Request) => {
  const { userDraftDtoSchema } = await import('@/entities/user/api/user-dto');
  return userDraftDtoSchema.safeParse(await request.json());
};

const badRequest = () =>
  HttpResponse.json({ code: 'BAD_REQUEST', message: 'The request body is not a valid user draft' }, { status: 400 });

const notFound = () => HttpResponse.json({ code: 'NOT_FOUND', message: 'User not found' }, { status: 404 });

const emailTaken = () =>
  HttpResponse.json(
    { code: 'VALIDATION', message: 'Validation failed', fields: { email: 'Эта почта уже занята' } },
    { status: 422 },
  );

const isEmailTaken = (email: string, exceptId?: string) =>
  userDb.list().some((user) => user.email === email && user.id !== exceptId);

export const handlers = [
  http.get('*/api/users', ({ request }) => {
    const url = new URL(request.url);
    const page = Number(url.searchParams.get('page') ?? '1');
    const perPage = Number(url.searchParams.get('per_page') ?? '20');
    const search = url.searchParams.get('search')?.toLowerCase() ?? '';
    const role = url.searchParams.get('role');

    const matched = userDb
      .list()
      .filter((user) => !role || user.role === role)
      .filter((user) => `${user.first_name} ${user.last_name} ${user.email}`.toLowerCase().includes(search));

    return HttpResponse.json({ items: matched.slice((page - 1) * perPage, page * perPage), total: matched.length });
  }),

  http.get('*/api/users/:userId', ({ params }) => {
    const user = userDb.find(String(params.userId));
    return user ? HttpResponse.json(user) : notFound();
  }),

  http.post('*/api/users', async ({ request }) => {
    const parsed = await parseUserDraft(request);
    if (!parsed.success) return badRequest();
    const body = parsed.data;
    if (isEmailTaken(body.email)) return emailTaken();
    const user = userDb.insert({ ...body, id: crypto.randomUUID(), created_at: new Date().toISOString() });
    return HttpResponse.json(user, { status: 201 });
  }),

  http.put('*/api/users/:userId', async ({ params, request }) => {
    const parsed = await parseUserDraft(request);
    if (!parsed.success) return badRequest();
    const body = parsed.data;
    const userId = String(params.userId);
    if (isEmailTaken(body.email, userId)) return emailTaken();
    const user = userDb.update(userId, body);
    return user ? HttpResponse.json(user) : notFound();
  }),

  http.delete('*/api/users/:userId', ({ params }) =>
    userDb.remove(String(params.userId)) ? new HttpResponse(null, { status: 204 }) : notFound(),
  ),
];
