import { useQuery } from '@tanstack/react-query';
import { getRouteApi } from '@tanstack/react-router';

import {
  EVENT_KINDS,
  HEALTH_RANGE_DAYS,
  HEALTH_RANGES,
  HEALTH_TEXTS,
  healthEventsQuery,
  healthOverviewQuery,
  minidumpsQuery,
  useRevealMinidump,
  type EventKind,
  type HealthRange,
} from '@/entities/health';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { usePreferences } from '@/shared/model/use-preferences';

import { ERRORS_TEXTS } from '../config/texts';
import { toOverviewView } from '../lib/errors-view';
import { toFeedView } from '../lib/event-feed-view';
import { toMinidumpView } from '../lib/minidump-view';

const route = getRouteApi('/errors');

export const useErrorsPage = () => {
  const { locale } = usePreferences();
  const texts = ERRORS_TEXTS[locale];
  const search = route.useSearch();
  const navigate = route.useNavigate();

  const overview = useQuery(healthOverviewQuery(HEALTH_RANGE_DAYS[search.range]));
  const events = useQuery(healthEventsQuery({ kinds: search.kinds, page: search.page }));
  const minidumps = useQuery(minidumpsQuery());
  const reveal = useRevealMinidump();

  const kindTexts = HEALTH_TEXTS[locale].kinds;

  return {
    texts,
    common: COMMON_TEXTS[locale],
    ranges: HEALTH_RANGES.map((range) => ({ range, label: texts.ranges[range], isActive: range === search.range })),
    kindFilters: EVENT_KINDS.map((kind) => ({ kind, label: kindTexts[kind], isActive: search.kinds.includes(kind) })),
    isAllKinds: search.kinds.length === 0,
    overviewView: toOverviewView({
      overview: overview.data,
      error: overview.error,
      range: search.range,
      locale,
      isRefreshing: overview.isPlaceholderData,
    }),
    feedView: toFeedView({
      eventPage: events.data,
      error: events.error,
      kinds: search.kinds,
      page: search.page,
      locale,
      isRefreshing: events.isPlaceholderData,
    }),
    minidumpView: toMinidumpView({ minidumps: minidumps.data, error: minidumps.error, locale }),
    revealFailed: reveal.isError ? texts.dumps.revealFailed : null,
    selectRange: (range: HealthRange) => void navigate({ search: (previous) => ({ ...previous, range }) }),
    toggleKind: (kind: EventKind) =>
      void navigate({
        search: (previous) => ({
          ...previous,
          page: 1,
          kinds: previous.kinds.includes(kind)
            ? previous.kinds.filter((active) => active !== kind)
            : [...previous.kinds, kind],
        }),
      }),
    showAllKinds: () => void navigate({ search: (previous) => ({ ...previous, page: 1, kinds: [] }) }),
    goToPage: (page: number) => void navigate({ search: (previous) => ({ ...previous, page }) }),
    revealMinidump: (name: string) => reveal.mutate(name),
    retryOverview: () => void overview.refetch(),
    retryEvents: () => void events.refetch(),
    retryMinidumps: () => void minidumps.refetch(),
  };
};
