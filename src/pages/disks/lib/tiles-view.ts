import { CATEGORIES, DISK_TEXTS, type Candidate, type Category, type Tile } from '@/entities/disk';
import { formatBytes } from '@/shared/lib/format-bytes';
import type { Locale } from '@/shared/lib/i18n';

import { DISKS_TEXTS } from '../config/texts';
import { squarify, type Rect } from './treemap';

export const TREEMAP_WIDTH = 1000;
export const TREEMAP_HEIGHT = 520;
const LABEL_MIN_WIDTH = 110;
const LABEL_MIN_HEIGHT = 44;

export type TreemapMode = 'category' | 'cleanup';

export type CleanupStatus = 'safe' | 'review' | 'none';

/** The fill of a tile: its category, or what cleaning it could do */
export type TileFill = Category | CleanupStatus;

interface TreemapTile {
  key: string;
  path: string;
  rect: Rect;
  fill: TileFill;
  title: string;
  label: string | null;
  sizeLabel: string;
}

export interface LegendEntry {
  fill: TileFill;
  label: string;
}

const isInside = (path: string, folder: string) => {
  const child = path.toLowerCase();
  const parent = folder.toLowerCase();
  return child === parent || child.startsWith(`${parent}\\`);
};

/** A tile is "safe" when a whitelisted candidate lies inside it (or holds it), "review" for a review-only one. */
export const cleanupStatusOf = (tile: Tile, candidates: readonly Candidate[]): CleanupStatus => {
  const related = candidates.filter(
    (candidate) =>
      candidate.bytes > 0 &&
      candidate.path !== null &&
      !tile.isRest &&
      (isInside(candidate.path, tile.path) || isInside(tile.path, candidate.path)),
  );
  if (related.some((candidate) => candidate.group === 'safe')) return 'safe';
  if (related.length > 0) return 'review';
  return 'none';
};

export const toTreemap = (
  tiles: readonly Tile[],
  candidates: readonly Candidate[],
  mode: TreemapMode,
  locale: Locale,
): { tiles: TreemapTile[]; legend: LegendEntry[] } => {
  const categoryTexts = DISK_TEXTS[locale].categories;
  const texts = DISKS_TEXTS[locale].treemap;
  const placed = squarify(tiles, (tile) => tile.bytes, { x: 0, y: 0, width: TREEMAP_WIDTH, height: TREEMAP_HEIGHT });
  // The tile shows the folder name; the hover title carries the whole path from the home
  const shortName = (label: string) => label.split('\\').at(-1) ?? label;
  const nameOf = (tile: Tile) =>
    tile.isRest ? `${tile.label || categoryTexts[tile.category]} · ${texts.rest}` : tile.label;

  return {
    tiles: placed.map(({ item, rect }) => {
      const sizeLabel = formatBytes(item.bytes, locale);
      const name = nameOf(item);
      const label = item.isRest ? name : shortName(name);
      const isLabelled = rect.width >= LABEL_MIN_WIDTH && rect.height >= LABEL_MIN_HEIGHT;
      return {
        key: `${item.path}|${item.label}|${item.category}|${String(item.isRest)}`,
        path: item.path,
        rect,
        fill: mode === 'category' ? item.category : cleanupStatusOf(item, candidates),
        title: `${name} — ${sizeLabel} — ${categoryTexts[item.category]}`,
        label: isLabelled ? label : null,
        sizeLabel,
      };
    }),
    legend:
      mode === 'category'
        ? CATEGORIES.map((category) => ({ fill: category, label: categoryTexts[category] }))
        : (['safe', 'review', 'none'] as const).map((status) => ({ fill: status, label: texts[status] })),
  };
};
