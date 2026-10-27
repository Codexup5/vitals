import { captureException } from '@sentry/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '@/shared/api/api-error';
import { MAX_QUERY_RETRIES } from '@/shared/config/query';

import { createQueryClient, isUnexpected, shouldRetry } from './query-client';

vi.mock('@sentry/react', () => ({ captureException: vi.fn() }));

afterEach(() => vi.clearAllMocks());

const KEY_WITH_PARAMS = ['users', 'list', { search: 'Anna Petrova', page: 2 }, 3] as const;
const KEY_SCOPE = ['users', 'list', 3];

const apiError = (kind: ApiError['kind'], status: number) =>
  new ApiError({ kind, status, code: 'TEST', message: 'test error' });

describe('shouldRetry', () => {
  it('stops on a schema error', () => {
    expect(shouldRetry(0, apiError('schema', 200))).toBe(false);
  });

  it('stops on a client error', () => {
    expect(shouldRetry(0, apiError('http', 404))).toBe(false);
  });

  it('stops at the retry limit', () => {
    expect(shouldRetry(MAX_QUERY_RETRIES, apiError('network', 0))).toBe(false);
  });

  it('retries a network error below the limit', () => {
    expect(shouldRetry(0, apiError('network', 0))).toBe(true);
  });

  it('retries a server error below the limit', () => {
    expect(shouldRetry(0, apiError('http', 503))).toBe(true);
  });
});

describe('isUnexpected', () => {
  it.each([
    ['a server error', apiError('http', 500), true],
    ['a schema error', apiError('schema', 200), true],
    ['a plain Error', new Error('boom'), true],
    ['a not found error', apiError('http', 404), false],
    ['a network error', apiError('network', 0), false],
  ])('%s', (_name, error, expected) => {
    expect(isUnexpected(error)).toBe(expected);
  });
});

describe('query error reporting', () => {
  it('reports a query key without its params object', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const error = new Error('render failed');
    const client = createQueryClient();

    await expect(
      client.query({ queryKey: KEY_WITH_PARAMS, queryFn: () => Promise.reject(error), retry: false }),
    ).rejects.toBe(error);

    expect(captureException).toHaveBeenCalledOnce();
    expect(captureException).toHaveBeenCalledWith(error, { extra: { queryKey: KEY_SCOPE } });
  });
});
