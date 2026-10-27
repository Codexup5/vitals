import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query';

import { MAX_QUERY_RETRIES, STALE_POLICY } from '@/shared/config/query';
import { reportError } from '@/shared/lib/report-error';

const toKeyScope = (queryKey: readonly unknown[]) =>
  queryKey.filter((part) => typeof part === 'string' || typeof part === 'number');

export const createQueryClient = () =>
  new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        reportError(error, { queryKey: toKeyScope(query.queryKey) });
      },
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _context, mutation) => {
        reportError(error, { mutationKey: mutation.options.mutationKey });
      },
    }),
    defaultOptions: {
      queries: { staleTime: STALE_POLICY.snapshot, retry: MAX_QUERY_RETRIES },
      mutations: { retry: false },
    },
  });
