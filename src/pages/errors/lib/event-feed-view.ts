import { CHART_KINDS, HEALTH_TEXTS, type EventKind, type HealthEventPage } from '@/entities/health';
import { getErrorMessage } from '@/shared/api/error-message';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { formatDateTime } from '@/shared/lib/date';
import type { Locale } from '@/shared/lib/i18n';

import { ERRORS_TEXTS } from '../config/texts';
import type { ChartKind } from './crash-chart';

interface FeedRow {
  id: string;
  timeLabel: string;
  kindLabel: string;
  /** The chart colour of the kind, when the chart shows it */
  chartKind: ChartKind | null;
  source: string;
  detail: string;
}

export type FeedView =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'empty'; message: string }
  | {
      status: 'ready';
      rows: readonly FeedRow[];
      found: string;
      page: number;
      pageCount: number;
      position: string;
      isRefreshing: boolean;
    };

interface FeedState {
  eventPage: HealthEventPage | undefined;
  error: unknown;
  kinds: readonly EventKind[];
  page: number;
  locale: Locale;
  isRefreshing: boolean;
}

const isChartKind = (kind: EventKind): kind is ChartKind => CHART_KINDS.some((chartKind) => chartKind === kind);

export const toFeedView = ({ eventPage, error, kinds, page, locale, isRefreshing }: FeedState): FeedView => {
  if (!eventPage) return error ? { status: 'error', message: getErrorMessage(error, locale) } : { status: 'loading' };
  const texts = ERRORS_TEXTS[locale].feed;
  if (eventPage.total === 0) return { status: 'empty', message: kinds.length > 0 ? texts.empty : texts.emptyAll };

  const kindTexts = HEALTH_TEXTS[locale].kinds;
  const missing = COMMON_TEXTS[locale].missingValue;
  return {
    status: 'ready',
    rows: eventPage.events.map((event) => ({
      id: event.id,
      timeLabel: formatDateTime(event.time, locale),
      kindLabel: kindTexts[event.kind],
      chartKind: isChartKind(event.kind) ? event.kind : null,
      source: `${event.provider} · ${event.eventId}`,
      detail: event.detail ?? missing,
    })),
    found: texts.found(eventPage.total),
    page: Math.min(page, eventPage.pageCount),
    pageCount: eventPage.pageCount,
    position: COMMON_TEXTS[locale].pagination.position(Math.min(page, eventPage.pageCount), eventPage.pageCount),
    isRefreshing,
  };
};
