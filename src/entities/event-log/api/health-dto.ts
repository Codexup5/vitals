import { z } from 'zod';

import { EVENT_KINDS } from '../config/event-kinds';

const count = z.number().int().nonnegative();

const periodDtoSchema = z.object({
  day: z.string().regex(/^\d{4}-\d{2}(-\d{2})?$/),
  crashes: count,
  bsod: count,
  gpuReset: count,
  nvContainer: count,
  whea: count,
  iommu: count,
  appCrashes: count,
  errors: count,
  topApps: z.array(z.string()),
});

export const healthOverviewDtoSchema = z.object({
  summary: z.object({
    daysWithoutCrash: z.number().int().nullable(),
    systemDriveFreeGb: z.number().nullable(),
  }),
  firstDay: z.string().nullable(),
  days: z.array(periodDtoSchema),
  months: z.array(periodDtoSchema),
});

const eventDtoSchema = z.object({
  time: z.number().int(),
  log: z.string(),
  provider: z.string(),
  eventId: z.number().int(),
  level: z.number().int(),
  kind: z.enum(EVENT_KINDS),
  detail: z.string().nullable(),
});

export const healthEventPageDtoSchema = z.object({
  items: z.array(eventDtoSchema),
  total: count,
});

export const minidumpListDtoSchema = z.array(
  z.object({
    name: z.string(),
    size: count,
    time: z.number().int(),
    code: z.string().nullable(),
  }),
);

export const emptyDtoSchema = z.null();

export type HealthOverviewDto = z.output<typeof healthOverviewDtoSchema>;
export type HealthEventPageDto = z.output<typeof healthEventPageDtoSchema>;
export type MinidumpListDto = z.output<typeof minidumpListDtoSchema>;
