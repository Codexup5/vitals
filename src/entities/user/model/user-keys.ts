import type { UserListParams } from './user';

export const USER_KEYS = {
  all: () => ['user'] as const,
  lists: () => [...USER_KEYS.all(), 'list'] as const,
  list: (params: UserListParams) => [...USER_KEYS.lists(), params] as const,
  details: () => [...USER_KEYS.all(), 'detail'] as const,
  detail: (userId: string) => [...USER_KEYS.details(), userId] as const,
};
