import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';

import { reportError } from '@/shared/lib/report-error';

import { AppErrorFallback } from './errors/AppErrorFallback';
import { createQueryClient } from './query/query-client';
import { createAppRouter } from './routing/router';

export const App = () => {
  const [queryClient] = useState(createQueryClient);
  const [router] = useState(() => createAppRouter(queryClient));

  return (
    <ErrorBoundary FallbackComponent={AppErrorFallback} onError={(error) => reportError(error, { boundary: 'app' })}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ErrorBoundary>
  );
};
