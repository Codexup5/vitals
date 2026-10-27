import { describe, expect, it } from 'vitest';

import { USER_NAME_MAX_LENGTH } from '../config/user-limits';
import { userDraftSchema } from './user-draft';

const VALID_DRAFT = { firstName: ' Анна ', lastName: 'Петрова', email: 'anna@example.com', role: 'editor' };

describe('userDraftSchema', () => {
  it('trims names', () => {
    expect(userDraftSchema.parse(VALID_DRAFT).firstName).toBe('Анна');
  });

  it('requires every name field', () => {
    const result = userDraftSchema.safeParse({ ...VALID_DRAFT, lastName: '   ' });
    expect(result.success).toBe(false);
    expect(result.error?.issues[0]?.path).toEqual(['lastName']);
  });

  it('enforces the name length limit', () => {
    const result = userDraftSchema.safeParse({ ...VALID_DRAFT, firstName: 'а'.repeat(USER_NAME_MAX_LENGTH + 1) });
    expect(result.success).toBe(false);
  });

  it('rejects a malformed email', () => {
    expect(userDraftSchema.safeParse({ ...VALID_DRAFT, email: 'anna' }).success).toBe(false);
  });

  it('rejects an unknown role', () => {
    expect(userDraftSchema.safeParse({ ...VALID_DRAFT, role: 'owner' }).success).toBe(false);
  });
});
