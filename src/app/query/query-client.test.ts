import { describe, expect, it, vi } from 'vitest';

import { createQueryClient } from './query-client';

const KEY_WITH_PARAMS = ['events', 'list', { range: '30d' }, 3] as const;
const KEY_SCOPE = ['events', 'list', 3];

describe('query error reporting', () => {
  it('reports a query key without its params object', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const error = new Error('collector failed');
    const client = createQueryClient();

    await expect(
      client.query({ queryKey: KEY_WITH_PARAMS, queryFn: () => Promise.reject(error), retry: false }),
    ).rejects.toBe(error);

    expect(consoleError).toHaveBeenCalledOnce();
    expect(consoleError).toHaveBeenCalledWith(error, { queryKey: KEY_SCOPE });
  });
});
