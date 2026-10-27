import { CHART_HEIGHT, CHART_WIDTH, type ChartKind, type CrashChart as CrashChartModel } from '../lib/crash-chart';
import type { ChartTableRow, LegendItem } from '../lib/errors-view';

import styles from './CrashChart.module.scss';

/** Colour and texture together: a kind never reads by colour alone (the brief, §1 p.4). */
const KIND_FILL: Readonly<Record<ChartKind, string>> = {
  crash: 'var(--color-chart-crash)',
  bsod: 'url(#vitals-texture-bsod)',
  whea: 'url(#vitals-texture-whea)',
  gpuReset: 'url(#vitals-texture-gpu-reset)',
  appCrash: 'url(#vitals-texture-app-crash)',
};

interface CrashChartProps {
  title: string;
  axisLabel: string;
  legendLabel: string;
  tableLabel: string;
  periodLabel: string;
  totalLabel: string;
  calmLabel: string;
  chart: CrashChartModel;
  legend: readonly LegendItem[];
  tableRows: readonly ChartTableRow[];
}

const Textures = () => (
  <defs>
    <pattern id="vitals-texture-bsod" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <rect width="6" height="6" className={styles.bsod} />
      <rect width="2" height="6" className={styles.stripe} />
    </pattern>
    <pattern id="vitals-texture-whea" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(-45)">
      <rect width="6" height="6" className={styles.whea} />
      <rect width="2" height="6" className={styles.stripe} />
    </pattern>
    <pattern id="vitals-texture-gpu-reset" width="6" height="6" patternUnits="userSpaceOnUse">
      <rect width="6" height="6" className={styles.gpuReset} />
      <circle cx="3" cy="3" r="1.2" className={styles.stripe} />
    </pattern>
    <pattern id="vitals-texture-app-crash" width="6" height="6" patternUnits="userSpaceOnUse">
      <rect width="6" height="6" className={styles.appCrash} />
      <rect width="6" height="2" className={styles.stripe} />
    </pattern>
  </defs>
);

export const CrashChart = ({
  title,
  axisLabel,
  legendLabel,
  tableLabel,
  periodLabel,
  totalLabel,
  calmLabel,
  chart,
  legend,
  tableRows,
}: CrashChartProps) => (
  <figure className={styles.figure}>
    <figcaption className={styles.caption}>
      <h2>{title}</h2>
      {chart.hasAnyCrash ? null : <p className={styles.calm}>{calmLabel}</p>}
    </figcaption>
    <p className={styles.unit}>{axisLabel}</p>
    <ul className={styles.legend} aria-label={legendLabel}>
      {legend.map((item) => (
        <li key={item.kind}>
          <svg className={styles.swatch} viewBox="0 0 12 12" aria-hidden="true">
            <rect width="12" height="12" rx="2" fill={KIND_FILL[item.kind]} />
          </svg>
          {item.label}
        </li>
      ))}
    </ul>
    <svg className={styles.plot} viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} role="img" aria-label={title}>
      <Textures />
      {chart.ticks.map((tick) => (
        <g key={tick.value}>
          <line x1={chart.plotLeft} x2={chart.plotRight} y1={tick.y} y2={tick.y} className={styles.grid} />
          <text x={chart.plotLeft - 6} y={tick.y} className={styles.tick} textAnchor="end" dominantBaseline="middle">
            {tick.value}
          </text>
        </g>
      ))}
      {chart.bars.map((bar) => (
        <g key={bar.key}>
          <title>{`${bar.label}: ${bar.total}`}</title>
          <rect x={bar.x} y={0} width={bar.width} height={chart.baselineY} className={styles.hit} />
          {bar.segments.map((segment) => (
            <rect
              key={segment.kind}
              x={bar.x}
              y={segment.y}
              width={bar.width}
              height={segment.height}
              fill={KIND_FILL[segment.kind]}
            />
          ))}
          {bar.hasAxisLabel ? (
            <text x={bar.labelX} y={chart.baselineY + 18} className={styles.tick} textAnchor="middle">
              {bar.label}
            </text>
          ) : null}
        </g>
      ))}
      <line
        x1={chart.plotLeft}
        x2={chart.plotRight}
        y1={chart.baselineY}
        y2={chart.baselineY}
        className={styles.axis}
      />
    </svg>
    <details className={styles.table}>
      <summary>{tableLabel}</summary>
      <div className={styles.scroll}>
        <table>
          <thead>
            <tr>
              <th scope="col">{periodLabel}</th>
              {legend.map((item) => (
                <th scope="col" key={item.kind}>
                  {item.label}
                </th>
              ))}
              <th scope="col">{totalLabel}</th>
            </tr>
          </thead>
          <tbody>
            {tableRows.map((row) => (
              <tr key={row.key}>
                <th scope="row">{row.label}</th>
                {row.values.map((value, index) => (
                  <td key={legend[index]?.kind ?? index}>{value}</td>
                ))}
                <td>{row.total}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </details>
  </figure>
);
