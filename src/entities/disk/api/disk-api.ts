import { callCommand } from '@/shared/api/ipc';

import { disksOverviewDtoSchema, revealedDtoSchema, scanStatusDtoSchema, startedDtoSchema } from './disk-dto';
import { toDisksOverview } from './disk-mapper';

export const fetchDisksOverview = async (signal?: AbortSignal) =>
  toDisksOverview(await callCommand('disks_overview', {}, disksOverviewDtoSchema, signal));

export const fetchScanStatus = (signal?: AbortSignal) => callCommand('scan_status', {}, scanStatusDtoSchema, signal);

export const startScan = () => callCommand('scan_start', {}, startedDtoSchema);

export const revealPath = async (path: string) => {
  await callCommand('reveal_path', { path }, revealedDtoSchema);
};
