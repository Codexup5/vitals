export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Placed<T> {
  item: T;
  rect: Rect;
}

interface Weighted<T> {
  item: T;
  area: number;
}

/** The worst aspect ratio of a row laid along a side of this length (Bruls, Huizing, van Wijk). */
const worstRatio = <T>(row: readonly Weighted<T>[], side: number) => {
  const areas = row.map((entry) => entry.area);
  const sum = areas.reduce((total, area) => total + area, 0);
  const largest = Math.max(...areas);
  const smallest = Math.min(...areas);
  return Math.max((side * side * largest) / (sum * sum), (sum * sum) / (side * side * smallest));
};

const placeRow = <T>(row: readonly Weighted<T>[], rect: Rect, placed: Placed<T>[]): Rect => {
  const sum = row.reduce((total, entry) => total + entry.area, 0);
  if (rect.width >= rect.height) {
    const columnWidth = sum / rect.height;
    let y = rect.y;
    for (const entry of row) {
      const height = entry.area / columnWidth;
      placed.push({ item: entry.item, rect: { x: rect.x, y, width: columnWidth, height } });
      y += height;
    }
    return { x: rect.x + columnWidth, y: rect.y, width: rect.width - columnWidth, height: rect.height };
  }
  const rowHeight = sum / rect.width;
  let x = rect.x;
  for (const entry of row) {
    const width = entry.area / rowHeight;
    placed.push({ item: entry.item, rect: { x, y: rect.y, width, height: rowHeight } });
    x += width;
  }
  return { x: rect.x, y: rect.y + rowHeight, width: rect.width, height: rect.height - rowHeight };
};

/** Squarified treemap: each item gets a rectangle whose area is its share of the whole, as square as the row allows. */
export const squarify = <T>(items: readonly T[], valueOf: (item: T) => number, bounds: Rect): Placed<T>[] => {
  const positive = items.filter((item) => valueOf(item) > 0);
  const total = positive.reduce((sum, item) => sum + valueOf(item), 0);
  if (total === 0) return [];
  const scale = (bounds.width * bounds.height) / total;
  const weighted = positive
    .map((item) => ({ item, area: valueOf(item) * scale }))
    .sort((left, right) => right.area - left.area);

  const placed: Placed<T>[] = [];
  let rect = bounds;
  let row: Weighted<T>[] = [];
  for (const entry of weighted) {
    const side = Math.min(rect.width, rect.height);
    if (row.length === 0 || worstRatio([...row, entry], side) <= worstRatio(row, side)) {
      row.push(entry);
    } else {
      rect = placeRow(row, rect, placed);
      row = [entry];
    }
  }
  if (row.length > 0) placeRow(row, rect, placed);
  return placed;
};
