import { captureException } from '@sentry/react';
import { QueryClient } from '@tanstack/react-query';
import { createMemoryHistory } from '@tanstack/react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '@/shared/api/api-error';

import { createAppRouter } from './router';

vi.mock('@sentry/react', () => ({ captureException: vi.fn() }));

afterEach(() => vi.clearAllMocks());

const catchRouteError = (error: Error) => {
  const router = createAppRouter(new QueryClient(), createMemoryHistory());
  router.options.defaultOnCatch?.(error, { componentStack: '' });
};

describe('route error reporting', () => {
  it('reports an unexpected route error once', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const error = new Error('render failed');

    catchRouteError(error);

    expect(captureException).toHaveBeenCalledOnce();
    expect(captureException).toHaveBeenCalledWith(error, { extra: { boundary: 'route' } });
  });

  it('leaves an ApiError to the query cache', () => {
    catchRouteError(new ApiError({ kind: 'http', status: 404, code: 'HTTP_404', message: 'GET /users/x failed' }));

    expect(captureException).not.toHaveBeenCalled();
  });
});
