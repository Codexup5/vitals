import { USER_ROLES, type User, type UserDraftInput, type UserRole } from '@/entities/user';

const EMPTY_USER_FORM: UserDraftInput = { firstName: '', lastName: '', email: '', role: 'viewer' };

export const toUserFormValues = (user?: User): UserDraftInput =>
  user ? { firstName: user.firstName, lastName: user.lastName, email: user.email, role: user.role } : EMPTY_USER_FORM;

export const toUserRole = (candidate: string): UserRole =>
  USER_ROLES.find((role) => role === candidate) ?? EMPTY_USER_FORM.role;
