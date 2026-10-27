import type { QueryClient } from '@tanstack/react-query';
import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  lazyRouteComponent,
  notFound,
  redirect,
  type RouterHistory,
} from '@tanstack/react-router';

import { NotFoundPage } from '@/pages/not-found';
import { DEFAULT_MODULE, isModuleId, type ModuleId } from '@/shared/config/modules';
import { reportError } from '@/shared/lib/report-error';

import { RouteErrorFallback } from '../errors/RouteErrorFallback';
import { AppLayout } from '../layout/AppLayout';

interface RouterContext {
  queryClient: QueryClient;
}

// notFound({ throw: true }) throws, yet its type returns the error object: the last line is never reached
const toModuleId = (value: string): ModuleId => {
  if (isModuleId(value)) return value;
  notFound({ throw: true });
  return DEFAULT_MODULE;
};

const rootRoute = createRootRouteWithContext<RouterContext>()({
  component: AppLayout,
  notFoundComponent: NotFoundPage,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => redirect({ to: '/$moduleId', params: { moduleId: DEFAULT_MODULE }, throw: true }),
});

const moduleRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '$moduleId',
  params: { parse: ({ moduleId }) => ({ moduleId: toModuleId(moduleId) }) },
  component: lazyRouteComponent(() => import('@/pages/module'), 'ModulePage'),
});

const routeTree = rootRoute.addChildren([indexRoute, moduleRoute]);

export const createAppRouter = (queryClient: QueryClient, history?: RouterHistory) =>
  createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: 'intent',
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteErrorFallback,
    defaultOnCatch: (error) => {
      reportError(error, { boundary: 'route' });
    },
    scrollRestoration: true,
    ...(history && { history }),
  });

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
