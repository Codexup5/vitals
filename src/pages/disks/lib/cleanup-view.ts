import { DISK_TEXTS, type Candidate, type CandidateGroup } from '@/entities/disk';
import { formatDate } from '@/shared/lib/date';
import { formatBytes } from '@/shared/lib/format-bytes';
import type { Locale } from '@/shared/lib/i18n';

import { DISKS_TEXTS } from '../config/texts';

/** Below a megabyte a row is noise: an empty .turbo of every package of a monorepo */
const WORTH_SHOWING = 1024 * 1024;

interface RecommendationRow {
  id: string;
  title: string;
  hint: string;
  path: string | null;
  sizeLabel: string;
  notes: readonly string[];
}

export interface RecommendationGroup {
  group: CandidateGroup;
  title: string;
  hint: string;
  totalLabel: string;
  rows: readonly RecommendationRow[];
}

export const sumOf = (candidates: readonly Candidate[], group: CandidateGroup) =>
  candidates.filter((candidate) => candidate.group === group).reduce((total, candidate) => total + candidate.bytes, 0);

const toRow = (candidate: Candidate, locale: Locale): RecommendationRow => {
  const texts = DISKS_TEXTS[locale].recommendations;
  const kindTexts = DISK_TEXTS[locale].candidates[candidate.kind];
  const size = formatBytes(candidate.bytes, locale);
  const notes: string[] = [];
  if (candidate.lastModified) notes.push(texts.changed(formatDate(candidate.lastModified, locale)));
  return {
    id: candidate.id,
    title: kindTexts.title,
    hint: kindTexts.hint,
    path: candidate.path,
    sizeLabel: candidate.isUpperBound ? texts.upTo(size) : size,
    notes,
  };
};

/** Biggest first inside each group; a candidate with nothing in it is not worth a row. */
export const toRecommendationGroups = (candidates: readonly Candidate[], locale: Locale): RecommendationGroup[] => {
  const texts = DISKS_TEXTS[locale].recommendations;
  return (['safe', 'review'] as const).map((group) => ({
    group,
    title: group === 'safe' ? texts.safe : texts.review,
    hint: group === 'safe' ? texts.safeHint : texts.reviewHint,
    totalLabel: texts.total(formatBytes(sumOf(candidates, group), locale)),
    rows: candidates
      .filter((candidate) => candidate.group === group && (candidate.bytes >= WORTH_SHOWING || candidate.needsAdmin))
      .toSorted((left, right) => right.bytes - left.bytes)
      .map((candidate) => toRow(candidate, locale)),
  }));
};
