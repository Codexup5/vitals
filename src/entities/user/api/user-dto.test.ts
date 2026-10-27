import { afterEach, describe, expect, it, vi } from 'vitest';

import { userDraftDtoSchema, userDtoSchema, userPageDtoSchema } from './user-dto';

vi.mock('@sentry/react', () => ({ captureException: vi.fn() }));

const USER_DTO = {
  id: 'u1',
  first_name: 'Анна',
  last_name: 'Петрова',
  email: 'anna@example.com',
  role: 'admin',
  created_at: '2026-01-12T09:00:00.000Z',
};

afterEach(() => vi.restoreAllMocks());

describe('user dto schema', () => {
  it.each([['yesterday'], [42], [undefined], [null]])('turns a broken creation date %j into null', (createdAt) => {
    expect(userDtoSchema.parse({ ...USER_DTO, created_at: createdAt }).created_at).toBeNull();
  });

  it('accepts a date with an offset', () => {
    const createdAt = '2026-01-12T12:00:00+03:00';
    expect(userDtoSchema.parse({ ...USER_DTO, created_at: createdAt }).created_at).toBe(createdAt);
  });

  it('rejects a user without the fields the entity cannot live without', () => {
    expect(userDtoSchema.safeParse({ ...USER_DTO, email: undefined }).success).toBe(false);
    expect(userDtoSchema.safeParse({ ...USER_DTO, role: 'owner' }).success).toBe(false);
  });

  it('keeps only the four draft fields', () => {
    expect(userDraftDtoSchema.parse(USER_DTO)).toEqual({
      first_name: 'Анна',
      last_name: 'Петрова',
      email: 'anna@example.com',
      role: 'admin',
    });
  });

  it('rejects a draft without email or with an unknown role', () => {
    expect(userDraftDtoSchema.safeParse({ ...USER_DTO, email: undefined }).success).toBe(false);
    expect(userDraftDtoSchema.safeParse({ ...USER_DTO, role: 'owner' }).success).toBe(false);
  });

  it('drops a broken list item and keeps the rest', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const page = userPageDtoSchema.parse({ items: [USER_DTO, { ...USER_DTO, id: 7 }], total: 2 });
    expect(page.items.map((user) => user.id)).toEqual(['u1']);
  });
});
