import Color from 'colorjs.io';

// DTCG Color module space names that colorjs.io spells differently
const SPACE_IDS = { 'display-p3': 'p3', 'a98-rgb': 'a98rgb', 'prophoto-rgb': 'prophoto' };

const toComponent = (component) => (component === 'none' ? null : component);

// A DTCG colour object, or a hex string such as #777777
const toColor = (value) =>
  typeof value === 'string'
    ? new Color(value)
    : new Color(SPACE_IDS[value.colorSpace] ?? value.colorSpace, value.components.map(toComponent), value.alpha ?? 1);

export const toCssColor = (value) => toColor(value).toString();

// Source-over: c = a × fg + (1 − a) × bg, per sRGB channel, the way a browser paints it
const compositeOver = (foreground, background) => {
  const alpha = foreground.alpha;
  if (alpha >= 1) return foreground;
  const front = foreground.to('srgb').coords;
  const back = background.to('srgb').coords;
  return new Color(
    'srgb',
    front.map((channel, index) => alpha * (channel ?? 0) + (1 - alpha) * (back[index] ?? 0)),
  );
};

// WCAG 2.2 contrast ratio (L1 + 0.05) / (L2 + 0.05) of a text or ui colour on a surface
export const wcagRatio = (foreground, background) => {
  const surface = toColor(background);
  return compositeOver(toColor(foreground), surface).contrast(surface, 'WCAG21');
};

// APCA 0.0.98G, the absolute Lc of a text or ui colour on a surface
export const apcaLc = (foreground, background) => {
  const surface = toColor(background);
  return Math.abs(surface.contrast(compositeOver(toColor(foreground), surface), 'APCA'));
};
