import { httpClient } from '@/shared/api/http-client';

import type { UserListParams } from '../model/user';
import type { UserDraft } from '../model/user-draft';
import { userDtoSchema, userPageDtoSchema } from './user-dto';
import { toUser, toUserDraftDto, toUserListQuery, toUserPage } from './user-mapper';

export const fetchUserPage = async (params: UserListParams, signal?: AbortSignal) => {
  const dto = await httpClient.get('/users', { schema: userPageDtoSchema, query: toUserListQuery(params), signal });
  return toUserPage(dto, params.pageSize);
};

export const fetchUserById = async (userId: string, signal?: AbortSignal) => {
  const dto = await httpClient.get(`/users/${encodeURIComponent(userId)}`, { schema: userDtoSchema, signal });
  return toUser(dto);
};

export const createUser = async (draft: UserDraft) => {
  const dto = await httpClient.post('/users', { schema: userDtoSchema, body: toUserDraftDto(draft) });
  return toUser(dto);
};

export const updateUser = async (userId: string, draft: UserDraft) => {
  const dto = await httpClient.put(`/users/${encodeURIComponent(userId)}`, {
    schema: userDtoSchema,
    body: toUserDraftDto(draft),
  });
  return toUser(dto);
};

export const deleteUser = (userId: string) => httpClient.delete(`/users/${encodeURIComponent(userId)}`);
