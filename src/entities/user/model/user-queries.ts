import { keepPreviousData, queryOptions } from '@tanstack/react-query';

import { STALE_POLICY } from '@/shared/config/query';

import { fetchUserById, fetchUserPage } from '../api/user-api';
import type { UserListParams } from './user';
import { USER_KEYS } from './user-keys';

export const userListQuery = (params: UserListParams) =>
  queryOptions({
    queryKey: USER_KEYS.list(params),
    queryFn: ({ signal }) => fetchUserPage(params, signal),
    staleTime: STALE_POLICY.list,
    placeholderData: keepPreviousData,
  });

export const userByIdQuery = (userId: string) =>
  queryOptions({
    queryKey: USER_KEYS.detail(userId),
    queryFn: ({ signal }) => fetchUserById(userId, signal),
    staleTime: STALE_POLICY.detail,
  });
