import { describe, expect, it } from 'vitest';

import { healthEventPageDtoSchema, healthOverviewDtoSchema } from './health-dto';
import { toHealthEventPage, toHealthOverview } from './health-mapper';

const counts = { crashes: 2, bsod: 0, gpuReset: 0, nvContainer: 940, whea: 0, iommu: 8, appCrashes: 1, errors: 1916 };

describe('health mappers', () => {
  it('turns days and months into periods', () => {
    const dto = healthOverviewDtoSchema.parse({
      summary: { daysWithoutCrash: 0, systemDriveFreeGb: 267.7 },
      firstDay: '2026-07-13',
      days: [{ day: '2026-10-27', ...counts, topApps: ['Steam.exe ×1'] }],
      months: [{ day: '2026-10', ...counts, topApps: [] }],
    });
    const overview = toHealthOverview(dto);
    expect(overview.days[0]).toEqual({ period: '2026-10-27', topApps: ['Steam.exe ×1'], ...counts });
    expect(overview.months[0]?.period).toBe('2026-10');
    expect(overview.daysWithoutCrash).toBe(0);
  });

  it('pages the feed by fifty and keeps a plain error as its own kind', () => {
    const dto = healthEventPageDtoSchema.parse({
      items: [
        { time: 1_793_097_600, log: 'System', provider: 'Disk', eventId: 7, level: 2, kind: 'error', detail: null },
      ],
      total: 101,
    });
    const page = toHealthEventPage(dto);
    expect(page.pageCount).toBe(3);
    expect(page.events[0]?.kind).toBe('error');
    expect(page.events[0]?.time.toISOString()).toBe('2026-10-27T10:40:00.000Z');
  });

  it('rejects an unknown kind', () => {
    const broken = {
      items: [{ time: 1, log: 'System', provider: 'x', eventId: 1, level: 2, kind: 'meteor', detail: null }],
      total: 1,
    };
    expect(healthEventPageDtoSchema.safeParse(broken).success).toBe(false);
  });
});
