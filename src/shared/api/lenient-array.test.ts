import { captureException } from '@sentry/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

import { isApiError } from './api-error';
import { lenientArray } from './lenient-array';

vi.mock('@sentry/react', () => ({ captureException: vi.fn() }));

const itemSchema = z.object({ id: z.string() });

afterEach(() => vi.clearAllMocks());

describe('lenientArray', () => {
  it('keeps valid items untouched', () => {
    expect(lenientArray(itemSchema, 'test.valid').parse([{ id: 'a' }, { id: 'b' }])).toEqual([
      { id: 'a' },
      { id: 'b' },
    ]);
    expect(captureException).not.toHaveBeenCalled();
  });

  it('drops broken items and reports them once per response with the source', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);

    const parsed = lenientArray(itemSchema, 'test.report').parse([{ id: 'a' }, { id: 1 }, { id: 'c' }, { id: null }]);

    expect(parsed).toEqual([{ id: 'a' }, { id: 'c' }]);
    expect(captureException).toHaveBeenCalledOnce();
    const [error, hint] = vi.mocked(captureException).mock.calls[0] ?? [];
    expect(isApiError(error) && error.kind).toBe('schema');
    expect(isApiError(error) && error.message).toBe('test.report: 2 of 4 list items do not match schema');
    expect(hint).toMatchObject({
      extra: {
        source: 'test.report',
        droppedIndexes: [1, 3],
        issues: [expect.objectContaining({ path: ['id'] })],
      },
    });
  });

  it('reports the same broken payload once and still drops its items every time', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const schema = lenientArray(itemSchema, 'test.repeat');
    const payload = [{ id: 'a' }, { id: 1 }];

    expect(schema.parse(payload)).toEqual([{ id: 'a' }]);
    expect(schema.parse(payload)).toEqual([{ id: 'a' }]);

    expect(captureException).toHaveBeenCalledOnce();
  });

  it('reports again when the payload is broken at another index', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const schema = lenientArray(itemSchema, 'test.index');

    schema.parse([{ id: 'a' }, { id: 1 }]);
    schema.parse([{ id: 1 }, { id: 'b' }]);

    expect(captureException).toHaveBeenCalledTimes(2);
  });

  it('still rejects a value that is not an array', () => {
    expect(lenientArray(itemSchema, 'test.shape').safeParse({ id: 'a' }).success).toBe(false);
  });
});
