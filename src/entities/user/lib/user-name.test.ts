import { describe, expect, it } from 'vitest';

import { getUserFullName } from './user-name';

describe('getUserFullName', () => {
  it('joins first and last name', () => {
    expect(getUserFullName({ firstName: 'Анна', lastName: 'Петрова' })).toBe('Анна Петрова');
  });

  it('drops the gap when a part is empty', () => {
    expect(getUserFullName({ firstName: 'Анна', lastName: '' })).toBe('Анна');
    expect(getUserFullName({ firstName: '', lastName: 'Петрова' })).toBe('Петрова');
  });
});
