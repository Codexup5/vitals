import { z } from 'zod';

import { CANDIDATE_KINDS, CATEGORIES } from '../config/disk-kinds';

const bytes = z.number().int().nonnegative();

const tileDtoSchema = z.object({
  path: z.string(),
  label: z.string(),
  bytes,
  category: z.enum(CATEGORIES),
  isRest: z.boolean(),
});

const candidateDtoSchema = z.object({
  id: z.string(),
  group: z.enum(['safe', 'review']),
  kind: z.enum(CANDIDATE_KINDS),
  path: z.string().nullable(),
  bytes,
  isUpperBound: z.boolean(),
  lastModified: z.number().int().nullable(),
  needsAdmin: z.boolean(),
});

const scanDtoSchema = z.object({
  finishedAt: z.number().int(),
  home: z.string(),
  totalBytes: bytes,
  files: bytes,
  links: bytes,
  unreadable: bytes,
  tiles: z.array(tileDtoSchema),
  candidates: z.array(candidateDtoSchema),
});

export const disksOverviewDtoSchema = z.object({
  volumes: z.array(z.object({ letter: z.string(), label: z.string(), totalBytes: bytes, freeBytes: bytes })),
  physicalDisks: z.array(
    z.object({
      name: z.string(),
      media: z.string(),
      sizeBytes: bytes,
      health: z.enum(['healthy', 'warning', 'unhealthy', 'unknown']),
      temperatureC: z.number().int().nullable(),
      wearPercent: z.number().int().nullable(),
      powerOnHours: z.number().int().nullable(),
    }),
  ),
  scan: scanDtoSchema.nullable(),
});

export const scanStatusDtoSchema = z.object({ isRunning: z.boolean(), files: bytes, bytes });

export const startedDtoSchema = z.boolean();

export const revealedDtoSchema = z.null();

export type DisksOverviewDto = z.output<typeof disksOverviewDtoSchema>;
