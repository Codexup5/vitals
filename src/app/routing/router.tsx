import type { QueryClient } from '@tanstack/react-query';
import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  lazyRouteComponent,
  notFound,
  redirect,
  stripSearchParams,
  type RouterHistory,
} from '@tanstack/react-router';

import { HEALTH_SEARCH_DEFAULTS, healthSearchSchema } from '@/entities/event-log';
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

const errorsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'errors',
  validateSearch: healthSearchSchema,
  search: { middlewares: [stripSearchParams(HEALTH_SEARCH_DEFAULTS)] },
  component: lazyRouteComponent(() => import('@/pages/errors'), 'ErrorsPage'),
});

const disksRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'disks',
  component: lazyRouteComponent(() => import('@/pages/disks'), 'DisksPage'),
});

const moduleRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '$moduleId',
  params: { parse: ({ moduleId }) => ({ moduleId: toModuleId(moduleId) }) },
  component: lazyRouteComponent(() => import('@/pages/module'), 'ModulePage'),
});

const routeTree = rootRoute.addChildren([indexRoute, errorsRoute, disksRoute, moduleRoute]);

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
