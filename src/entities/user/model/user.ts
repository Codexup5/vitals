import type { USER_ROLES } from '../config/user-roles';

export type UserRole = (typeof USER_ROLES)[number];

export interface User {
  id: string;
  firstName: string;
  lastName: string;
  email: string;
  role: UserRole;
  createdAt: Date | null;
}

export interface UserListParams {
  page: number;
  pageSize: number;
  search?: string | undefined;
  role?: UserRole | undefined;
}

export interface UserPage {
  users: readonly User[];
  total: number;
  pageCount: number;
}
