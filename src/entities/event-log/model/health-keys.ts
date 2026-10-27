import type { HealthEventParams } from './health';

export const HEALTH_KEYS = {
  all: () => ['health'] as const,
  overviews: () => [...HEALTH_KEYS.all(), 'overview'] as const,
  overview: (days: number) => [...HEALTH_KEYS.overviews(), days] as const,
  eventLists: () => [...HEALTH_KEYS.all(), 'events'] as const,
  eventList: (params: HealthEventParams) => [...HEALTH_KEYS.eventLists(), params] as const,
  minidumps: () => [...HEALTH_KEYS.all(), 'minidumps'] as const,
};
