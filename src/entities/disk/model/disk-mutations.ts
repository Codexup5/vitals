import { useMutation, useQueryClient } from '@tanstack/react-query';

import { revealPath, startScan } from '../api/disk-api';
import { markScanFinished, markScanStarted } from './disk-cache';

export const useStartScan = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => startScan(),
    onSuccess: () => markScanStarted(queryClient),
  });
};

/** The page calls it when the polled status turns from running to idle. */
export const useScanFinished = () => {
  const queryClient = useQueryClient();
  return () => void markScanFinished(queryClient);
};

export const useRevealPath = () =>
  useMutation({
    mutationFn: (path: string) => revealPath(path),
  });
