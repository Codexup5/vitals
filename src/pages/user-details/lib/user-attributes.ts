import { USER_TEXTS, type User } from '@/entities/user';
import { formatDate } from '@/shared/lib/date';

export const toUserAttributes = (user: User) => [
  { label: USER_TEXTS.fields.email, text: user.email },
  { label: USER_TEXTS.fields.role, text: USER_TEXTS.roles[user.role] },
  { label: USER_TEXTS.fields.createdAt, text: formatDate(user.createdAt) },
];
