import { queryOptions } from '@tanstack/react-query';

import { STALE_POLICY } from '@/shared/config/query';

import { fetchDisksOverview, fetchScanStatus } from '../api/disk-api';
import { SCAN_POLL_MS } from '../config/disk-kinds';
import { DISK_KEYS } from './disk-keys';

export const disksOverviewQuery = () =>
  queryOptions({
    queryKey: DISK_KEYS.overview(),
    queryFn: ({ signal }) => fetchDisksOverview(signal),
    staleTime: STALE_POLICY.snapshot,
  });

/** Polled once a second while a scan runs, not at all otherwise. */
export const scanStatusQuery = () =>
  queryOptions({
    queryKey: DISK_KEYS.scanStatus(),
    queryFn: ({ signal }) => fetchScanStatus(signal),
    staleTime: STALE_POLICY.live,
    refetchInterval: (query) => (query.state.data?.isRunning ? SCAN_POLL_MS : false),
  });
