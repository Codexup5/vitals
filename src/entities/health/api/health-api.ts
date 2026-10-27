import { callCommand } from '@/shared/api/ipc';

import type { HealthEventParams } from '../model/health';
import { emptyDtoSchema, healthEventPageDtoSchema, healthOverviewDtoSchema, minidumpListDtoSchema } from './health-dto';
import { toHealthEventPage, toHealthOverview, toMinidumps } from './health-mapper';

export const fetchHealthOverview = async (days: number, signal?: AbortSignal) =>
  toHealthOverview(await callCommand('health_overview', { days }, healthOverviewDtoSchema, signal));

export const fetchHealthEvents = async (params: HealthEventParams, signal?: AbortSignal) =>
  toHealthEventPage(
    await callCommand(
      'health_events',
      { kinds: [...params.kinds], from: null, to: null, page: params.page - 1 },
      healthEventPageDtoSchema,
      signal,
    ),
  );

export const fetchMinidumps = async (signal?: AbortSignal) =>
  toMinidumps(await callCommand('minidumps', {}, minidumpListDtoSchema, signal));

export const revealMinidump = async (name: string) => {
  await callCommand('reveal_minidump', { name }, emptyDtoSchema);
};
