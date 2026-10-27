import { describe, expect, it } from 'vitest';

import { pluralize, type PluralForms } from './plural';

const USERS: PluralForms = ['пользователь', 'пользователя', 'пользователей'];

describe('pluralize', () => {
  it.each([
    [1, 'пользователь'],
    [21, 'пользователь'],
    [2, 'пользователя'],
    [24, 'пользователя'],
    [5, 'пользователей'],
    [11, 'пользователей'],
    [12, 'пользователей'],
    [14, 'пользователей'],
    [112, 'пользователей'],
    [0, 'пользователей'],
  ])('picks the form for %i', (count, form) => {
    expect(pluralize(count, USERS)).toBe(form);
  });
});
