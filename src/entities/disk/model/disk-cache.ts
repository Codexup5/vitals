import type { QueryClient } from '@tanstack/react-query';

import { DISK_KEYS } from './disk-keys';

/** A scan started: its status is polled again. */
export const markScanStarted = (queryClient: QueryClient) =>
  queryClient.invalidateQueries({ queryKey: DISK_KEYS.scanStatus() });

/** A scan finished: the overview carries the new treemap and preview. */
export const markScanFinished = (queryClient: QueryClient) =>
  queryClient.invalidateQueries({ queryKey: DISK_KEYS.overview() });
