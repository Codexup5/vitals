import { QueryClient } from '@tanstack/react-query';
import { createMemoryHistory } from '@tanstack/react-router';
import { describe, expect, it, vi } from 'vitest';

import { createAppRouter } from './router';

describe('route error reporting', () => {
  it('reports a route error once, on the PC only', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const error = new Error('render failed');
    const router = createAppRouter(new QueryClient(), createMemoryHistory());

    router.options.defaultOnCatch?.(error, { componentStack: '' });

    expect(consoleError).toHaveBeenCalledOnce();
    expect(consoleError).toHaveBeenCalledWith(error, { boundary: 'route' });
  });
});
