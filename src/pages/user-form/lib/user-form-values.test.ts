import { describe, expect, it } from 'vitest';

import type { User } from '@/entities/user';

import { toUserFormValues, toUserRole } from './user-form-values';

const USER: User = {
  id: 'u1',
  firstName: 'Анна',
  lastName: 'Петрова',
  email: 'anna@example.com',
  role: 'admin',
  createdAt: new Date('2026-01-12T09:00:00.000Z'),
};

describe('toUserFormValues', () => {
  it('takes the editable fields of an existing user', () => {
    expect(toUserFormValues(USER)).toEqual({
      firstName: 'Анна',
      lastName: 'Петрова',
      email: 'anna@example.com',
      role: 'admin',
    });
  });

  it('starts a new user empty with the least privileged role', () => {
    expect(toUserFormValues()).toEqual({ firstName: '', lastName: '', email: '', role: 'viewer' });
  });
});

describe('toUserRole', () => {
  it.each(['admin', 'editor', 'viewer'])('keeps the known role %s', (role) => {
    expect(toUserRole(role)).toBe(role);
  });

  it('falls back to viewer for an unknown value', () => {
    expect(toUserRole('owner')).toBe('viewer');
  });
});
