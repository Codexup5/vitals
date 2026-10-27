import { getUserFullName, USER_TEXTS, type UserPage } from '@/entities/user';
import { getErrorMessage } from '@/shared/api/error-message';
import { formatDate } from '@/shared/lib/date';

import { USER_LIST_TEXTS } from '../config/texts';

export interface UserRow {
  id: string;
  fullName: string;
  email: string;
  roleLabel: string;
  createdAtLabel: string;
}

export type UserListView =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'empty'; hasFilters: boolean }
  | { status: 'pastLastPage'; lastPage: number }
  | {
      status: 'ready';
      rows: readonly UserRow[];
      page: number;
      pageCount: number;
      total: number;
      isRefreshing: boolean;
    };

interface UserListState {
  userPage: UserPage | undefined;
  error: unknown;
  page: number;
  hasFilters: boolean;
  isRefreshing: boolean;
}

export const toUserListView = ({ userPage, error, page, hasFilters, isRefreshing }: UserListState): UserListView => {
  if (!userPage) return error ? { status: 'error', message: getErrorMessage(error) } : { status: 'loading' };
  if (userPage.users.length === 0) {
    if (userPage.total === 0) return { status: 'empty', hasFilters };
    // The page is within the list but lenientArray dropped all its rows: a data error, not "no such page"
    return page > userPage.pageCount
      ? { status: 'pastLastPage', lastPage: userPage.pageCount }
      : { status: 'error', message: USER_LIST_TEXTS.brokenPage };
  }

  return {
    status: 'ready',
    rows: userPage.users.map((user) => ({
      id: user.id,
      fullName: getUserFullName(user),
      email: user.email,
      roleLabel: USER_TEXTS.roles[user.role],
      createdAtLabel: formatDate(user.createdAt),
    })),
    page,
    pageCount: userPage.pageCount,
    total: userPage.total,
    isRefreshing,
  };
};
