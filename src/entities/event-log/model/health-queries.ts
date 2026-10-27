import { keepPreviousData, queryOptions } from '@tanstack/react-query';

import { STALE_POLICY } from '@/shared/config/query';

import { fetchHealthEvents, fetchHealthOverview, fetchMinidumps } from '../api/health-api';
import type { HealthEventParams } from './health';
import { HEALTH_KEYS } from './health-keys';

export const healthOverviewQuery = (days: number) =>
  queryOptions({
    queryKey: HEALTH_KEYS.overview(days),
    queryFn: ({ signal }) => fetchHealthOverview(days, signal),
    staleTime: STALE_POLICY.snapshot,
    placeholderData: keepPreviousData,
  });

export const healthEventsQuery = (params: HealthEventParams) =>
  queryOptions({
    queryKey: HEALTH_KEYS.eventList(params),
    queryFn: ({ signal }) => fetchHealthEvents(params, signal),
    staleTime: STALE_POLICY.snapshot,
    placeholderData: keepPreviousData,
  });

export const minidumpsQuery = () =>
  queryOptions({
    queryKey: HEALTH_KEYS.minidumps(),
    queryFn: ({ signal }) => fetchMinidumps(signal),
    staleTime: STALE_POLICY.snapshot,
  });
