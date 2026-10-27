import { z } from 'zod';

import { EVENT_KINDS, HEALTH_RANGES } from '../config/event-kinds';
import type { EventKind, HealthRange } from './health';

export const HEALTH_SEARCH_DEFAULTS: { range: HealthRange; kinds: EventKind[]; page: number } = {
  range: '30d',
  kinds: [],
  page: 1,
};

/** The state of the errors screen that survives a reload: the chart range, the feed filter, the feed page. */
export const healthSearchSchema = z.object({
  range: z.enum(HEALTH_RANGES).default(HEALTH_SEARCH_DEFAULTS.range).catch(HEALTH_SEARCH_DEFAULTS.range),
  kinds: z
    .array(z.enum(EVENT_KINDS))
    .default([...HEALTH_SEARCH_DEFAULTS.kinds])
    .catch([...HEALTH_SEARCH_DEFAULTS.kinds]),
  page: z.number().int().positive().default(HEALTH_SEARCH_DEFAULTS.page).catch(HEALTH_SEARCH_DEFAULTS.page),
});
