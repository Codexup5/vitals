import { describe, expect, it } from 'vitest';

import type { Candidate, Tile } from '@/entities/disk';

import { cleanupStatusOf, toTreemap } from './tiles-view';

const tile = (path: string, bytes: number, category: Tile['category'] = 'other'): Tile => ({
  path,
  label: path.replace('C:\\Users\\someone\\', ''),
  bytes,
  category,
  isRest: false,
});

const candidate = (path: string, group: Candidate['group'], bytes = 10): Candidate => ({
  id: path,
  group,
  kind: group === 'safe' ? 'pnpmStore' : 'staleNodeModules',
  path,
  bytes,
  isUpperBound: false,
  lastModified: null,
  needsAdmin: false,
});

describe('cleanupStatusOf', () => {
  it('marks a tile by the strongest candidate inside it or around it', () => {
    const pnpm = tile('C:\\Users\\someone\\AppData\\Local\\pnpm', 100);
    const project = tile('C:\\Users\\someone\\Desktop\\Code\\old', 100);
    const candidates = [
      candidate('C:\\Users\\someone\\AppData\\Local\\pnpm\\store', 'safe'),
      candidate('C:\\Users\\someone\\Desktop\\Code\\old\\node_modules', 'review'),
    ];
    expect(cleanupStatusOf(pnpm, candidates)).toBe('safe');
    expect(cleanupStatusOf(project, candidates)).toBe('review');
    expect(cleanupStatusOf(tile('C:\\Users\\someone\\Documents', 10), candidates)).toBe('none');
  });

  it('does not take a sibling with a common prefix for a parent', () => {
    const tiles = tile('C:\\Users\\someone\\AppData\\Local\\pnpm-tools', 10);
    expect(cleanupStatusOf(tiles, [candidate('C:\\Users\\someone\\AppData\\Local\\pnpm', 'safe')])).toBe('none');
  });

  it('ignores an empty candidate', () => {
    const pnpm = tile('C:\\Users\\someone\\AppData\\Local\\pnpm', 100);
    expect(cleanupStatusOf(pnpm, [candidate('C:\\Users\\someone\\AppData\\Local\\pnpm', 'safe', 0)])).toBe('none');
  });
});

describe('toTreemap', () => {
  it('labels a big tile with its folder name and keeps the path in the title', () => {
    const { tiles } = toTreemap([tile('C:\\Users\\someone\\AppData\\Local\\pnpm', 10)], [], 'category', 'en');
    expect(tiles[0]?.label).toBe('pnpm');
    expect(tiles[0]?.title).toContain('AppData\\Local\\pnpm');
  });

  it('labels only the tiles big enough to hold a label', () => {
    const { tiles, legend } = toTreemap(
      [tile('C:\\Users\\someone\\Big', 1000, 'codeProjects'), tile('C:\\Users\\someone\\Tiny', 1)],
      [],
      'category',
      'en',
    );
    expect(tiles.find((entry) => entry.path.endsWith('Big'))?.label).toBe('Big');
    expect(tiles.find((entry) => entry.path.endsWith('Big'))?.title).toContain('Big');
    expect(tiles.find((entry) => entry.path.endsWith('Tiny'))?.label).toBeNull();
    expect(legend).toHaveLength(8);
  });
});
