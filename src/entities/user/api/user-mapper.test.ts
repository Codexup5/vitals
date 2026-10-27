import { describe, expect, it } from 'vitest';

import type { UserDto } from './user-dto';
import { toUser, toUserDraftDto, toUserListQuery, toUserPage } from './user-mapper';

const USER_DTO: UserDto = {
  id: 'u1',
  first_name: 'Анна',
  last_name: 'Петрова',
  email: 'anna@example.com',
  role: 'admin',
  created_at: '2026-01-12T09:00:00.000Z',
};

describe('user mapper', () => {
  it('maps wire format to the domain model', () => {
    expect(toUser(USER_DTO)).toEqual({
      id: 'u1',
      firstName: 'Анна',
      lastName: 'Петрова',
      email: 'anna@example.com',
      role: 'admin',
      createdAt: new Date('2026-01-12T09:00:00.000Z'),
    });
  });

  it('keeps a missing creation date as null instead of throwing', () => {
    expect(toUser({ ...USER_DTO, created_at: null }).createdAt).toBeNull();
  });

  it('rounds page count up and never returns zero pages', () => {
    expect(toUserPage({ items: [USER_DTO], total: 41 }, 20).pageCount).toBe(3);
    expect(toUserPage({ items: [], total: 0 }, 20).pageCount).toBe(1);
  });

  it('maps a draft back to snake_case', () => {
    const draft = { firstName: 'Анна', lastName: 'Петрова', email: 'anna@example.com', role: 'viewer' } as const;
    expect(toUserDraftDto(draft)).toEqual({
      first_name: 'Анна',
      last_name: 'Петрова',
      email: 'anna@example.com',
      role: 'viewer',
    });
  });

  it('keeps empty filters out of the query string', () => {
    expect(toUserListQuery({ page: 2, pageSize: 20 })).toEqual({
      page: 2,
      per_page: 20,
      search: undefined,
      role: undefined,
    });
  });
});
