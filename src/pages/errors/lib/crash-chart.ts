import { CHART_KINDS, type HealthPeriod } from '@/entities/event-log';

export type ChartKind = (typeof CHART_KINDS)[number];

const VALUE_OF: Readonly<Record<ChartKind, (period: HealthPeriod) => number>> = {
  crash: (period) => period.crashes,
  bsod: (period) => period.bsod,
  whea: (period) => period.whea,
  gpuReset: (period) => period.gpuReset,
  appCrash: (period) => period.appCrashes,
};

export const CHART_WIDTH = 720;
export const CHART_HEIGHT = 240;
const PLOT_LEFT = 40;
const PLOT_RIGHT = 8;
const PLOT_TOP = 12;
const PLOT_BOTTOM = 28;
const BAR_SHARE = 0.7;
const SEGMENT_GAP = 2;
const MAX_X_LABELS = 10;

interface ChartSegment {
  kind: ChartKind;
  value: number;
  y: number;
  height: number;
}

interface ChartBar {
  key: string;
  label: string;
  x: number;
  width: number;
  total: number;
  segments: readonly ChartSegment[];
  /** Shown under the axis: at most ten labels so they never collide */
  hasAxisLabel: boolean;
  labelX: number;
}

interface ChartTick {
  value: number;
  y: number;
}

export interface CrashChart {
  bars: readonly ChartBar[];
  ticks: readonly ChartTick[];
  baselineY: number;
  plotLeft: number;
  plotRight: number;
  hasAnyCrash: boolean;
}

/** The smallest of 1, 2, 5, 10, 20, 50… not below the value: the top tick is a round number. */
export const niceCeiling = (value: number) => {
  if (value <= 1) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(value));
  const step = [1, 2, 5, 10].find((multiple) => multiple * magnitude >= value) ?? 10;
  return step * magnitude;
};

export const valueOf = (period: HealthPeriod, kind: ChartKind) => VALUE_OF[kind](period);

export const toCrashChart = (periods: readonly HealthPeriod[], labelOf: (period: string) => string): CrashChart => {
  const totals = periods.map((period) => CHART_KINDS.reduce((sum, kind) => sum + valueOf(period, kind), 0));
  const top = niceCeiling(Math.max(0, ...totals));
  const baselineY = CHART_HEIGHT - PLOT_BOTTOM;
  const plotHeight = baselineY - PLOT_TOP;
  const plotRight = CHART_WIDTH - PLOT_RIGHT;
  const slot = (plotRight - PLOT_LEFT) / Math.max(1, periods.length);
  const barWidth = slot * BAR_SHARE;
  const labelEvery = Math.max(1, Math.ceil(periods.length / MAX_X_LABELS));
  const scale = (value: number) => (value / top) * plotHeight;

  const bars = periods.map((period, index): ChartBar => {
    const x = PLOT_LEFT + index * slot + (slot - barWidth) / 2;
    let stackTop = baselineY;
    const segments = CHART_KINDS.filter((kind) => valueOf(period, kind) > 0).map((kind): ChartSegment => {
      const value = valueOf(period, kind);
      const height = Math.max(1, scale(value) - SEGMENT_GAP);
      const y = stackTop - height;
      stackTop = y - SEGMENT_GAP;
      return { kind, value, y, height };
    });
    return {
      key: period.period,
      label: labelOf(period.period),
      x,
      width: barWidth,
      total: totals[index] ?? 0,
      segments,
      hasAxisLabel: index % labelEvery === 0,
      labelX: x + barWidth / 2,
    };
  });

  return {
    bars,
    ticks: [0, top / 2, top]
      .filter((value) => Number.isInteger(value))
      .map((value) => ({ value, y: baselineY - scale(value) })),
    baselineY,
    plotLeft: PLOT_LEFT,
    plotRight,
    hasAnyCrash: totals.some((total) => total > 0),
  };
};
