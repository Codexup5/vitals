import { describe, expect, it } from 'vitest';

import { userListSearchSchema } from './user-list-search';

describe('userListSearchSchema', () => {
  it('fills defaults for an empty URL', () => {
    expect(userListSearchSchema.parse({})).toEqual({ page: 1, search: '', role: undefined });
  });

  it('falls back instead of failing on a hand-edited URL', () => {
    expect(userListSearchSchema.parse({ page: -3, search: 42, role: 'owner' })).toEqual({
      page: 1,
      search: '',
      role: undefined,
    });
  });

  it('keeps valid filters', () => {
    expect(userListSearchSchema.parse({ page: 2, search: ' anna ', role: 'admin' })).toEqual({
      page: 2,
      search: 'anna',
      role: 'admin',
    });
  });
});
