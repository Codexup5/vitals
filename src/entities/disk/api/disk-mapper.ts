import { fromUnixSeconds } from '@/shared/lib/date';

import type { DisksOverview } from '../model/disk';
import type { DisksOverviewDto } from './disk-dto';

export const toDisksOverview = (dto: DisksOverviewDto): DisksOverview => ({
  volumes: dto.volumes,
  physicalDisks: dto.physicalDisks,
  scan: dto.scan && {
    ...dto.scan,
    finishedAt: fromUnixSeconds(dto.scan.finishedAt),
    candidates: dto.scan.candidates.map((candidate) => ({
      ...candidate,
      lastModified: candidate.lastModified === null ? null : fromUnixSeconds(candidate.lastModified),
    })),
  },
});
