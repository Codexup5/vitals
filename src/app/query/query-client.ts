import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query';

import { isApiError } from '@/shared/api/api-error';
import { MAX_QUERY_RETRIES, STALE_POLICY } from '@/shared/config/query';
import { reportError } from '@/shared/lib/report-error';

export const shouldRetry = (failureCount: number, error: unknown) =>
  failureCount < MAX_QUERY_RETRIES && !(isApiError(error) && (error.isClientError || error.kind === 'schema'));

export const isUnexpected = (error: unknown) => !isApiError(error) || error.status >= 500 || error.kind === 'schema';

const toKeyScope = (queryKey: readonly unknown[]) =>
  queryKey.filter((part) => typeof part === 'string' || typeof part === 'number');

export const createQueryClient = () =>
  new QueryClient({
    queryCache: new QueryCache({
      onError: (error, query) => {
        if (isUnexpected(error)) reportError(error, { queryKey: toKeyScope(query.queryKey) });
      },
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _context, mutation) => {
        if (isUnexpected(error)) reportError(error, { mutationKey: mutation.options.mutationKey });
      },
    }),
    defaultOptions: {
      queries: { staleTime: STALE_POLICY.list, retry: shouldRetry },
      mutations: { retry: false },
    },
  });
