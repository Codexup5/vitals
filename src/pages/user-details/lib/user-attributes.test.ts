import { describe, expect, it } from 'vitest';

import { USER_TEXTS, type User } from '@/entities/user';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { formatDate } from '@/shared/lib/date';

import { toUserAttributes } from './user-attributes';

const USER: User = {
  id: 'u1',
  firstName: 'Анна',
  lastName: 'Петрова',
  email: 'anna@example.com',
  role: 'editor',
  createdAt: new Date('2026-01-12T09:00:00.000Z'),
};

describe('toUserAttributes', () => {
  it('lists email, role and creation date with their labels', () => {
    expect(toUserAttributes(USER)).toEqual([
      { label: USER_TEXTS.fields.email, text: 'anna@example.com' },
      { label: USER_TEXTS.fields.role, text: USER_TEXTS.roles.editor },
      { label: USER_TEXTS.fields.createdAt, text: formatDate(USER.createdAt) },
    ]);
  });

  it('shows a dash when the creation date is unknown', () => {
    expect(toUserAttributes({ ...USER, createdAt: null })).toContainEqual({
      label: USER_TEXTS.fields.createdAt,
      text: COMMON_TEXTS.missingValue,
    });
  });
});
