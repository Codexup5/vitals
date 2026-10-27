import type { QueryClient } from '@tanstack/react-query';

import type { User } from './user';
import { USER_KEYS } from './user-keys';

const invalidateUserLists = (queryClient: QueryClient) =>
  queryClient.invalidateQueries({ queryKey: USER_KEYS.lists() });

export const putUserInCache = (queryClient: QueryClient, user: User) => {
  queryClient.setQueryData(USER_KEYS.detail(user.id), user);
  return invalidateUserLists(queryClient);
};

export const dropUserFromCache = async (queryClient: QueryClient, userId: string, leaveUser: () => Promise<void>) => {
  queryClient.removeQueries({ queryKey: USER_KEYS.lists() });
  await leaveUser();
  // A removed query under a mounted useSuspenseQuery refetches and gets 404, so the detail goes last.
  queryClient.removeQueries({ queryKey: USER_KEYS.detail(userId) });
};
