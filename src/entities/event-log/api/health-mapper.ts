import { fromUnixSeconds } from '@/shared/lib/date';

import { EVENT_PAGE_SIZE } from '../config/event-kinds';
import type { HealthEventPage, HealthOverview, HealthPeriod, Minidump } from '../model/health';
import type { HealthEventPageDto, HealthOverviewDto, MinidumpListDto } from './health-dto';

const toPeriod = ({ day, topApps, ...counts }: HealthOverviewDto['days'][number]): HealthPeriod => ({
  period: day,
  topApps,
  ...counts,
});

export const toHealthOverview = (dto: HealthOverviewDto): HealthOverview => ({
  daysWithoutCrash: dto.summary.daysWithoutCrash,
  systemDriveFreeGb: dto.summary.systemDriveFreeGb,
  firstDay: dto.firstDay,
  days: dto.days.map(toPeriod),
  months: dto.months.map(toPeriod),
});

export const toHealthEventPage = (dto: HealthEventPageDto): HealthEventPage => ({
  events: dto.items.map((item) => ({
    // The log and its time identify an entry; the record id stays on the Rust side
    id: `${item.log}-${item.time}-${item.provider}-${item.eventId}-${item.detail ?? ''}`,
    time: fromUnixSeconds(item.time),
    kind: item.kind,
    provider: item.provider,
    eventId: item.eventId,
    detail: item.detail,
  })),
  total: dto.total,
  pageCount: Math.max(1, Math.ceil(dto.total / EVENT_PAGE_SIZE)),
});

export const toMinidumps = (dto: MinidumpListDto): Minidump[] =>
  dto.map((item) => ({ name: item.name, sizeBytes: item.size, time: fromUnixSeconds(item.time), code: item.code }));
