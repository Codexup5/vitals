import { TREEMAP_HEIGHT, TREEMAP_WIDTH, type LegendEntry, type TileFill } from '../lib/tiles-view';
import type { Rect } from '../lib/treemap';

import styles from './Treemap.module.scss';

type Texture = 'solid' | 'diagonal' | 'antidiagonal' | 'dots' | 'horizontal' | 'vertical' | 'cross';

/** Colour and texture together: a category never reads by colour alone (the brief, §1 p.4). */
const LOOK: Readonly<Record<TileFill, { className: string | undefined; texture: Texture }>> = {
  documentsMedia: { className: styles.documents, texture: 'solid' },
  devTools: { className: styles.devTools, texture: 'diagonal' },
  codeProjects: { className: styles.code, texture: 'dots' },
  sdks: { className: styles.sdks, texture: 'antidiagonal' },
  aiModels: { className: styles.ai, texture: 'cross' },
  packageCaches: { className: styles.caches, texture: 'horizontal' },
  system: { className: styles.system, texture: 'vertical' },
  other: { className: styles.other, texture: 'solid' },
  safe: { className: styles.code, texture: 'dots' },
  review: { className: styles.caches, texture: 'horizontal' },
  none: { className: styles.other, texture: 'solid' },
};

const Mark = ({ texture }: { texture: Texture }) => {
  switch (texture) {
    case 'solid':
      return null;
    case 'dots':
      return <circle cx="4" cy="4" r="1.3" className={styles.mark} />;
    case 'horizontal':
      return <rect width="8" height="2" className={styles.mark} />;
    case 'vertical':
      return <rect width="2" height="8" className={styles.mark} />;
    case 'diagonal':
    case 'antidiagonal':
      return <rect width="2.5" height="8" className={styles.mark} />;
    case 'cross':
      return (
        <>
          <rect width="2" height="8" className={styles.mark} />
          <rect width="8" height="2" className={styles.mark} />
        </>
      );
  }
};

const Textures = () => (
  <defs>
    {Object.entries(LOOK).map(([fill, look]) => (
      <pattern
        key={fill}
        id={`vitals-tile-${fill}`}
        width="8"
        height="8"
        patternUnits="userSpaceOnUse"
        patternTransform={
          look.texture === 'diagonal' ? 'rotate(45)' : look.texture === 'antidiagonal' ? 'rotate(-45)' : undefined
        }
      >
        <rect width="8" height="8" className={look.className} />
        <Mark texture={look.texture} />
      </pattern>
    ))}
  </defs>
);

interface TreemapTileProps {
  key: string;
  path: string;
  rect: Rect;
  fill: TileFill;
  title: string;
  label: string | null;
  sizeLabel: string;
}

interface TreemapProps {
  title: string;
  description: string;
  legendLabel: string;
  tiles: readonly TreemapTileProps[];
  legend: readonly LegendEntry[];
}

export const Treemap = ({ title, description, legendLabel, tiles, legend }: TreemapProps) => (
  <figure className={styles.figure}>
    <ul className={styles.legend} aria-label={legendLabel}>
      {legend.map((entry) => (
        <li key={entry.fill}>
          <svg className={styles.swatch} viewBox="0 0 12 12" aria-hidden="true">
            <rect width="12" height="12" rx="2" fill={`url(#vitals-tile-${entry.fill})`} />
          </svg>
          {entry.label}
        </li>
      ))}
    </ul>
    <svg className={styles.plot} viewBox={`0 0 ${TREEMAP_WIDTH} ${TREEMAP_HEIGHT}`} role="img" aria-label={title}>
      <desc>{description}</desc>
      <Textures />
      {tiles.map((tile) => (
        <g key={tile.key}>
          <title>{tile.title}</title>
          <rect
            x={tile.rect.x + 1}
            y={tile.rect.y + 1}
            width={Math.max(0, tile.rect.width - 2)}
            height={Math.max(0, tile.rect.height - 2)}
            rx="3"
            fill={`url(#vitals-tile-${tile.fill})`}
            className={styles.tile}
          />
          {tile.label ? (
            <>
              <rect
                x={tile.rect.x + 6}
                y={tile.rect.y + 6}
                width={Math.min(tile.rect.width - 12, 220)}
                height="34"
                rx="3"
                className={styles.plate}
              />
              <text x={tile.rect.x + 12} y={tile.rect.y + 20} className={styles.label}>
                {tile.label}
              </text>
              <text x={tile.rect.x + 12} y={tile.rect.y + 35} className={styles.size}>
                {tile.sizeLabel}
              </text>
            </>
          ) : null}
        </g>
      ))}
    </svg>
  </figure>
);
