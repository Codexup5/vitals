import { useMutation, useQueryClient } from '@tanstack/react-query';

import { createUser, deleteUser, updateUser } from '../api/user-api';
import { dropUserFromCache, putUserInCache } from './user-cache';
import type { UserDraft } from './user-draft';

interface DeleteUserOptions {
  onDeleted: () => Promise<void>;
}

export const useCreateUser = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (draft: UserDraft) => createUser(draft),
    onSuccess: (user) => putUserInCache(queryClient, user),
  });
};

export const useUpdateUser = (userId: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (draft: UserDraft) => updateUser(userId, draft),
    onSuccess: (user) => putUserInCache(queryClient, user),
  });
};

export const useDeleteUser = (userId: string, { onDeleted }: DeleteUserOptions) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => deleteUser(userId),
    onSuccess: () => dropUserFromCache(queryClient, userId, onDeleted),
  });
};
