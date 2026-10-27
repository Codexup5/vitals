import type { QueryParams } from '@/shared/api/http-client';
import { parseIsoDateTime } from '@/shared/lib/date';

import type { User, UserListParams, UserPage } from '../model/user';
import type { UserDraft } from '../model/user-draft';
import type { UserDraftDto, UserDto, UserPageDto } from './user-dto';

export const toUser = (dto: UserDto): User => ({
  id: dto.id,
  firstName: dto.first_name,
  lastName: dto.last_name,
  email: dto.email,
  role: dto.role,
  createdAt: dto.created_at === null ? null : parseIsoDateTime(dto.created_at),
});

export const toUserPage = (dto: UserPageDto, pageSize: number): UserPage => ({
  users: dto.items.map(toUser),
  total: dto.total,
  pageCount: Math.max(1, Math.ceil(dto.total / pageSize)),
});

export const toUserDraftDto = (draft: UserDraft): UserDraftDto => ({
  first_name: draft.firstName,
  last_name: draft.lastName,
  email: draft.email,
  role: draft.role,
});

export const toUserListQuery = ({ page, pageSize, search, role }: UserListParams): QueryParams => ({
  page,
  per_page: pageSize,
  search,
  role,
});
