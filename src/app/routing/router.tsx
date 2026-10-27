import type { QueryClient } from '@tanstack/react-query';
import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  lazyRouteComponent,
  redirect,
  stripSearchParams,
  type RouterHistory,
} from '@tanstack/react-router';

import { USER_LIST_SEARCH_DEFAULTS, userByIdQuery, userListSearchSchema } from '@/entities/user';
import { NotFoundPage } from '@/pages/not-found';
import { isApiError } from '@/shared/api/api-error';
import { reportError } from '@/shared/lib/report-error';

import { RouteErrorFallback } from '../errors/RouteErrorFallback';
import { AppLayout } from '../layout/AppLayout';

interface RouterContext {
  queryClient: QueryClient;
}

const rootRoute = createRootRouteWithContext<RouterContext>()({
  component: AppLayout,
  notFoundComponent: NotFoundPage,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => redirect({ to: '/users', search: {}, throw: true }),
});

const userListRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'users',
  validateSearch: userListSearchSchema,
  search: { middlewares: [stripSearchParams(USER_LIST_SEARCH_DEFAULTS)] },
  component: lazyRouteComponent(() => import('@/pages/user-list'), 'UserListPage'),
});

const userCreateRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'users/new',
  component: lazyRouteComponent(() => import('@/pages/user-form'), 'UserCreatePage'),
});

const userDetailsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'users/$userId',
  validateSearch: userListSearchSchema,
  search: { middlewares: [stripSearchParams(USER_LIST_SEARCH_DEFAULTS)] },
  loader: ({ context, params }) => context.queryClient.query({ ...userByIdQuery(params.userId), staleTime: 'static' }),
  pendingComponent: lazyRouteComponent(() => import('@/pages/user-details'), 'UserDetailsSkeleton'),
  component: lazyRouteComponent(() => import('@/pages/user-details'), 'UserDetailsPage'),
});

const userEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'users/$userId/edit',
  loader: ({ context, params }) => context.queryClient.query({ ...userByIdQuery(params.userId), staleTime: 'static' }),
  pendingComponent: lazyRouteComponent(() => import('@/pages/user-form'), 'UserFormSkeleton'),
  component: lazyRouteComponent(() => import('@/pages/user-form'), 'UserEditPage'),
});

const routeTree = rootRoute.addChildren([indexRoute, userListRoute, userCreateRoute, userDetailsRoute, userEditRoute]);

export const createAppRouter = (queryClient: QueryClient, history?: RouterHistory) =>
  createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: 'intent',
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteErrorFallback,
    defaultOnCatch: (error) => {
      if (!isApiError(error)) reportError(error, { boundary: 'route' });
    },
    scrollRestoration: true,
    ...(history && { history }),
  });

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
