import { describe, expect, it } from 'vitest';

import type { HealthPeriod } from '@/entities/event-log';

import { CHART_HEIGHT, niceCeiling, toCrashChart } from './crash-chart';

const period = (name: string, counts: Partial<HealthPeriod> = {}): HealthPeriod => ({
  period: name,
  topApps: [],
  crashes: 0,
  bsod: 0,
  gpuReset: 0,
  nvContainer: 0,
  whea: 0,
  iommu: 0,
  appCrashes: 0,
  errors: 0,
  ...counts,
});

describe('niceCeiling', () => {
  it.each([
    [0, 1],
    [1, 1],
    [3, 5],
    [7, 10],
    [12, 20],
    [51, 100],
  ])('%d → %d', (value, expected) => {
    expect(niceCeiling(value)).toBe(expected);
  });
});

describe('toCrashChart', () => {
  it('stacks the crash kinds bottom up with gaps and ignores noise kinds', () => {
    const chart = toCrashChart(
      [period('2026-10-27', { crashes: 2, bsod: 1, nvContainer: 940, errors: 1916 })],
      (key) => key,
    );
    const [bar] = chart.bars;
    expect(bar?.total).toBe(3);
    expect(bar?.segments.map((segment) => segment.kind)).toEqual(['crash', 'bsod']);
    const [crash, bsod] = bar?.segments ?? [];
    // The crash sits on the baseline, the BSOD above it, a gap between them
    expect((crash?.y ?? 0) + (crash?.height ?? 0)).toBeCloseTo(chart.baselineY);
    expect((bsod?.y ?? 0) + (bsod?.height ?? 0)).toBeLessThan(crash?.y ?? 0);
    expect(chart.ticks.map((tick) => tick.value)).toEqual([0, 5]);
  });

  it('keeps every mark inside the drawing', () => {
    const periods = Array.from({ length: 90 }, (_, index) => period(`d${index}`, { appCrashes: index % 7 }));
    const chart = toCrashChart(periods, (key) => key);
    for (const bar of chart.bars) {
      expect(bar.x).toBeGreaterThanOrEqual(chart.plotLeft);
      expect(bar.x + bar.width).toBeLessThanOrEqual(chart.plotRight);
      for (const segment of bar.segments) expect(segment.y).toBeGreaterThanOrEqual(0);
    }
    expect(chart.bars.filter((bar) => bar.hasAxisLabel).length).toBeLessThanOrEqual(11);
    expect(chart.baselineY).toBeLessThan(CHART_HEIGHT);
  });

  it('says when the period had no crash at all', () => {
    expect(toCrashChart([period('2026-10-26')], (key) => key).hasAnyCrash).toBe(false);
  });
});
