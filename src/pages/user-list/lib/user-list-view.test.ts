import { describe, expect, it } from 'vitest';

import type { UserPage } from '@/entities/user';
import { ApiError } from '@/shared/api/api-error';
import { COMMON_TEXTS } from '@/shared/config/texts';

import { USER_LIST_TEXTS } from '../config/texts';
import { toUserListView } from './user-list-view';

const USER_PAGE: UserPage = {
  users: [
    {
      id: 'u1',
      firstName: 'Анна',
      lastName: 'Петрова',
      email: 'anna@example.com',
      role: 'admin',
      createdAt: new Date('2026-01-12T09:00:00.000Z'),
    },
  ],
  total: 1,
  pageCount: 1,
};

const BASE = { userPage: undefined, error: null, page: 1, hasFilters: false, isRefreshing: false };

describe('toUserListView', () => {
  it('is loading while there is neither data nor error', () => {
    expect(toUserListView(BASE)).toEqual({ status: 'loading' });
  });

  it('explains the error in user terms', () => {
    const error = new ApiError({ kind: 'network', status: 0, code: 'NETWORK', message: 'offline' });
    expect(toUserListView({ ...BASE, error })).toEqual({ status: 'error', message: COMMON_TEXTS.errors.network });
  });

  it('keeps showing stale data when a refetch fails', () => {
    const error = new ApiError({ kind: 'http', status: 500, code: 'HTTP_500', message: 'boom' });
    expect(toUserListView({ ...BASE, userPage: USER_PAGE, error }).status).toBe('ready');
  });

  it('tells an empty list apart from an empty search', () => {
    const emptyPage = { users: [], total: 0, pageCount: 1 };
    expect(toUserListView({ ...BASE, userPage: emptyPage })).toEqual({ status: 'empty', hasFilters: false });
    expect(toUserListView({ ...BASE, userPage: emptyPage, hasFilters: true })).toEqual({
      status: 'empty',
      hasFilters: true,
    });
  });

  it('offers the last page when the page is past the end of the list', () => {
    const pastEnd = { users: [], total: 45, pageCount: 3 };
    expect(toUserListView({ ...BASE, userPage: pastEnd, page: 99 })).toEqual({ status: 'pastLastPage', lastPage: 3 });
  });

  it('reports broken data when every row of a page in range was dropped', () => {
    const allDropped = { users: [], total: 1, pageCount: 1 };
    expect(toUserListView({ ...BASE, userPage: allDropped, page: 1 })).toEqual({
      status: 'error',
      message: USER_LIST_TEXTS.brokenPage,
    });
  });

  it('builds display rows', () => {
    const view = toUserListView({ ...BASE, userPage: USER_PAGE, isRefreshing: true });
    expect(view).toMatchObject({
      status: 'ready',
      isRefreshing: true,
      rows: [{ id: 'u1', fullName: 'Анна Петрова', roleLabel: 'Администратор', createdAtLabel: '12 января 2026 г.' }],
    });
  });

  it('shows a dash for an unknown creation date', () => {
    const users = USER_PAGE.users.map((user) => ({ ...user, createdAt: null }));
    const view = toUserListView({ ...BASE, userPage: { ...USER_PAGE, users } });
    expect(view).toMatchObject({ rows: [{ createdAtLabel: COMMON_TEXTS.missingValue }] });
  });
});
