import { describe, expect, it } from 'vitest';

import { squarify } from './treemap';

const BOUNDS = { x: 0, y: 0, width: 1000, height: 500 };

describe('squarify', () => {
  it('gives each item its share of the area and stays inside the bounds', () => {
    const values = [500, 300, 120, 50, 20, 10];
    const placed = squarify(values, (value) => value, BOUNDS);
    const total = values.reduce((sum, value) => sum + value, 0);
    for (const { item, rect } of placed) {
      expect((rect.width * rect.height) / (BOUNDS.width * BOUNDS.height)).toBeCloseTo(item / total, 6);
      expect(rect.x).toBeGreaterThanOrEqual(-1e-9);
      expect(rect.y).toBeGreaterThanOrEqual(-1e-9);
      expect(rect.x + rect.width).toBeLessThanOrEqual(BOUNDS.width + 1e-6);
      expect(rect.y + rect.height).toBeLessThanOrEqual(BOUNDS.height + 1e-6);
    }
  });

  it('never overlaps two tiles', () => {
    const placed = squarify([40, 30, 20, 10, 5, 5, 3, 2], (value) => value, BOUNDS);
    for (const [index, first] of placed.entries()) {
      for (const second of placed.slice(index + 1)) {
        const overlapX =
          Math.min(first.rect.x + first.rect.width, second.rect.x + second.rect.width) -
          Math.max(first.rect.x, second.rect.x);
        const overlapY =
          Math.min(first.rect.y + first.rect.height, second.rect.y + second.rect.height) -
          Math.max(first.rect.y, second.rect.y);
        expect(Math.min(overlapX, overlapY)).toBeLessThanOrEqual(1e-6);
      }
    }
  });

  it('keeps tiles near square, unlike plain slices', () => {
    const placed = squarify([1, 1, 1, 1], (value) => value, { x: 0, y: 0, width: 400, height: 400 });
    for (const { rect } of placed) expect(rect.width / rect.height).toBeCloseTo(1, 6);
  });

  it('drops empty items and an empty list', () => {
    expect(squarify([0, 0], (value) => value, BOUNDS)).toEqual([]);
    expect(squarify([5, 0], (value) => value, BOUNDS)).toHaveLength(1);
  });
});
