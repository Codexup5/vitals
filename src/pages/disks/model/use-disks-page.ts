import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

import { disksOverviewQuery, scanStatusQuery, useRevealPath, useScanFinished, useStartScan } from '@/entities/disk';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { usePreferences } from '@/shared/model/use-preferences';

import { DISKS_TEXTS } from '../config/texts';
import { toDisksView } from '../lib/disks-view';
import { toTreemap, type TreemapMode } from '../lib/tiles-view';

const SYSTEM_DRIVE = 'C';

export const useDisksPage = () => {
  const { locale } = usePreferences();
  const texts = DISKS_TEXTS[locale];
  const overview = useQuery(disksOverviewQuery());
  const scanStatus = useQuery(scanStatusQuery());
  const startScan = useStartScan();
  const reveal = useRevealPath();
  const onScanFinished = useScanFinished();
  const [mode, setMode] = useState<TreemapMode>('category');

  // The scan runs in Rust: when its polled status turns idle, the overview is read again for the new result
  const isRunning = scanStatus.data?.isRunning ?? false;
  const wasRunning = useRef(isRunning);
  useEffect(() => {
    if (wasRunning.current && !isRunning) onScanFinished();
    wasRunning.current = isRunning;
  }, [isRunning, onScanFinished]);

  const scan = overview.data?.scan;
  return {
    texts,
    common: COMMON_TEXTS[locale],
    view: toDisksView({
      overview: overview.data,
      error: overview.error,
      scanStatus: scanStatus.data,
      systemDrive: SYSTEM_DRIVE,
      locale,
    }),
    treemap: scan ? toTreemap(scan.tiles, scan.candidates, mode, locale) : null,
    mode,
    isScanning: isRunning || startScan.isPending,
    scanFailed: startScan.isError ? texts.scan.failed : null,
    revealFailed: reveal.isError ? texts.recommendations.revealFailed : null,
    selectMode: setMode,
    startScan: () => startScan.mutate(),
    reveal: (path: string) => reveal.mutate(path),
    retry: () => void overview.refetch(),
  };
};
