import { CHART_KINDS, HEALTH_TEXTS, type HealthOverview, type HealthRange } from '@/entities/health';
import { getErrorMessage } from '@/shared/api/error-message';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { formatCalendarDay, formatCalendarMonth } from '@/shared/lib/date';
import type { Locale } from '@/shared/lib/i18n';

import { ERRORS_TEXTS } from '../config/texts';
import { toCrashChart, valueOf, type ChartKind, type CrashChart } from './crash-chart';

interface Kpi {
  key: string;
  label: string;
  value: string;
  hint: string;
  isAlarming: boolean;
}

export interface LegendItem {
  kind: ChartKind;
  label: string;
}

export interface ChartTableRow {
  key: string;
  label: string;
  values: readonly number[];
  total: number;
}

export type OverviewView =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'empty' }
  | {
      status: 'ready';
      kpis: readonly Kpi[];
      chartTitle: string;
      chart: CrashChart;
      legend: readonly LegendItem[];
      tableRows: readonly ChartTableRow[];
      isRefreshing: boolean;
    };

interface OverviewState {
  overview: HealthOverview | undefined;
  error: unknown;
  range: HealthRange;
  locale: Locale;
  isRefreshing: boolean;
}

export const toOverviewView = ({ overview, error, range, locale, isRefreshing }: OverviewState): OverviewView => {
  if (!overview) return error ? { status: 'error', message: getErrorMessage(error, locale) } : { status: 'loading' };
  if (overview.firstDay === null) return { status: 'empty' };

  const texts = ERRORS_TEXTS[locale];
  const kindTexts = HEALTH_TEXTS[locale].kinds;
  const isMonthly = range === 'months';
  const periods = isMonthly ? overview.months : overview.days;
  const labelOf = (period: string) =>
    isMonthly ? formatCalendarMonth(period, locale) : formatCalendarDay(period, locale);
  const sum = (pick: (counts: (typeof periods)[number]) => number) =>
    periods.reduce((total, period) => total + pick(period), 0);
  const hint = isMonthly ? texts.kpis.sinceFirstDay : texts.kpis.inRange;
  const crashes = sum((period) => period.crashes);
  const bsod = sum((period) => period.bsod);
  const whea = sum((period) => period.whea);

  return {
    status: 'ready',
    kpis: [
      {
        key: 'daysWithoutCrash',
        label: texts.kpis.daysWithoutCrash,
        value:
          overview.daysWithoutCrash === null ? COMMON_TEXTS[locale].missingValue : String(overview.daysWithoutCrash),
        hint: texts.kpis.sinceFirstDay,
        isAlarming: overview.daysWithoutCrash === 0,
      },
      { key: 'crashes', label: texts.kpis.crashes, value: String(crashes), hint, isAlarming: crashes > 0 },
      { key: 'bsod', label: texts.kpis.bsod, value: String(bsod), hint, isAlarming: bsod > 0 },
      { key: 'whea', label: texts.kpis.whea, value: String(whea), hint, isAlarming: whea > 0 },
      {
        key: 'appCrashes',
        label: texts.kpis.appCrashes,
        value: String(sum((period) => period.appCrashes)),
        hint,
        isAlarming: false,
      },
    ],
    chartTitle: isMonthly ? texts.chart.titleMonths : texts.chart.title,
    chart: toCrashChart(periods, labelOf),
    legend: CHART_KINDS.map((kind) => ({ kind, label: kindTexts[kind] })),
    tableRows: periods.map((period) => {
      const values = CHART_KINDS.map((kind) => valueOf(period, kind));
      return {
        key: period.period,
        label: labelOf(period.period),
        values,
        total: values.reduce((total, value) => total + value, 0),
      };
    }),
    isRefreshing,
  };
};
