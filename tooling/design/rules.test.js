import { describe, expect, it } from 'vitest';

import { checkTokens } from './rules.mjs';

const FILE = 'src/shared/styles/tokens/base.tokens.json';

const token = (path, type, value, kit = {}) => ({ path, type, value, kit, file: FILE });
const color = (path, value, kit) => token(path, 'color', value, kit);
const dimension = (path, value, unit, kit) => token(path, 'dimension', { value, unit }, kit);
const surface = (path, value) => color(path, value, { role: 'surface' });
const text = (path, value, kit = {}) => color(path, value, { role: 'text', ...kit });
const ui = (path, value, kit = {}) => color(path, value, { role: 'ui', ...kit });
const space = (path, px) => dimension(path, px, 'px', { role: 'space' });
const size = (path, value, unit = 'px') => dimension(path, value, unit, { role: 'size' });
const target = (path, px) => dimension(path, px, 'px', { role: 'target' });
const fontSize = (path, value, unit = 'px') => dimension(path, value, unit, { role: 'font-size' });
const lineHeight = (path, value, kit) => token(path, 'number', value, { role: 'line-height', ...kit });

const theme = (name, tokens) => ({ name, kind: 'theme', tokens });
const pointer = (name, tokens) => ({ name, kind: 'pointer', tokens });
const onWhite = (...tokens) => theme('light', [surface('color.bg', '#ffffff'), ...tokens]);

const ofRule = (findings, id) => findings.filter((finding) => finding.id === id);

const expectFinding = (findings, expected) => {
  expect(findings).toContainEqual(expect.objectContaining(expected));
};

describe('design rules: contrast of text', () => {
  it('P1 fails design/contrast-text for #777777 on white (4.48:1)', () => {
    const found = checkTokens([onWhite(text('color.text', '#777777'))]);
    expectFinding(found, {
      id: 'design/contrast-text',
      level: 'violation',
      subject: 'color.text on color.bg',
      context: 'light',
    });
  });

  it('P2 passes #767676 on white (4.54:1) and warns design/apca at Lc 71.6', () => {
    const found = checkTokens([onWhite(text('color.text', '#767676'))]);
    expect(ofRule(found, 'design/contrast-text')).toEqual([]);
    expectFinding(found, {
      id: 'design/apca',
      level: 'warning',
      subject: 'color.text on color.bg',
      context: 'light',
    });
  });

  it('P3 passes large #777777 on white (4.48:1 at 3:1, Lc 71.1 at 60)', () => {
    const found = checkTokens([onWhite(text('color.text', '#777777', { large: true }))]);
    expect(found).toEqual([]);
  });

  it('P4 fails large #959595 on white (2.99:1 under 3:1)', () => {
    const found = checkTokens([onWhite(text('color.text', '#959595', { large: true }))]);
    expectFinding(found, {
      id: 'design/contrast-text',
      level: 'violation',
      subject: 'color.text on color.bg',
      context: 'light',
    });
  });

  it('P5 exempts an incidental #c5c8ce on white', () => {
    const found = checkTokens([onWhite(text('color.text', '#c5c8ce', { incidental: true }))]);
    expect(found).toEqual([]);
  });

  it('P8 fails #5b616b on #121417 in the dark context and not the light one', () => {
    const found = checkTokens([
      onWhite(text('color.text', '#15171a')),
      theme('dark', [surface('color.bg', '#121417'), text('color.text', '#5b616b')]),
    ]);
    expectFinding(found, {
      id: 'design/contrast-text',
      level: 'violation',
      subject: 'color.text on color.bg',
      context: 'dark',
    });
    expect(ofRule(found, 'design/contrast-text').filter((finding) => finding.context === 'light')).toEqual([]);
  });

  it('P9 pairs on-accent only with the accent it names, not with the page', () => {
    const found = checkTokens([
      theme('light', [
        surface('color.bg', '#ffffff'),
        text('color.accent', '#2f5bea'),
        text('color.on-accent', '#ffffff', { surfaces: ['{color.accent}'] }),
      ]),
    ]);
    expect(ofRule(found, 'design/contrast-text')).toEqual([]);
  });
});

describe('design rules: contrast of ui', () => {
  it('P6 fails #959595 on white (2.99:1 under 3:1)', () => {
    const found = checkTokens([onWhite(ui('color.border-control', '#959595'))]);
    expectFinding(found, {
      id: 'design/contrast-ui',
      level: 'violation',
      subject: 'color.border-control on color.bg',
      context: 'light',
    });
  });

  it('P7 passes #949494 on white (3.03:1, Lc 57.1 at 45)', () => {
    const found = checkTokens([onWhite(ui('color.border-control', '#949494'))]);
    expect(found).toEqual([]);
  });
});

describe('design rules: grid', () => {
  it('P10 fails a space of 6 px with no context', () => {
    const found = checkTokens([pointer('fine', [space('space.1', 6)])]);
    expectFinding(found, { id: 'design/grid', level: 'violation', subject: 'space.1', context: '' });
  });

  it('P11 passes spaces of 8, 2 and 1 px and a size of 0.5 rem', () => {
    const found = checkTokens([
      pointer('fine', [
        space('space.1', 8),
        space('space.hair', 2),
        space('space.optical', 1),
        size('size.icon', 0.5, 'rem'),
      ]),
    ]);
    expect(ofRule(found, 'design/grid')).toEqual([]);
  });

  it('P12 fails a space of 3 px', () => {
    const found = checkTokens([pointer('fine', [space('space.1', 3)])]);
    expectFinding(found, { id: 'design/grid', level: 'violation', subject: 'space.1', context: '' });
  });

  it('P13 fails a target of 46 px as grid, with no target finding', () => {
    const tokens = [target('touch-target', 46)];
    const found = checkTokens([pointer('fine', tokens), pointer('coarse', tokens)]);
    expectFinding(found, { id: 'design/grid', level: 'violation', subject: 'touch-target', context: '' });
    expect(found.filter((finding) => finding.id.startsWith('design/target'))).toEqual([]);
  });

  it('P13a fails a space of 7 px that only the coarse layer sets, once for a base space every context sees', () => {
    const base = space('space.2', 6);
    const coarseSpace = { ...space('space.1', 7), file: 'src/shared/styles/tokens/coarse.tokens.json' };
    const found = checkTokens([pointer('fine', [base]), pointer('coarse', [base, coarseSpace])]);
    expectFinding(found, { id: 'design/grid', subject: 'space.1', file: coarseSpace.file });
    expect(ofRule(found, 'design/grid').filter((finding) => finding.subject === 'space.2')).toHaveLength(1);
  });
});

describe('design rules: line height', () => {
  it('P14 fails a line height in px, which is not a number', () => {
    const found = checkTokens([
      pointer('fine', [token('line-height.body', 'dimension', { value: 24, unit: 'px' }, { role: 'line-height' })]),
    ]);
    expectFinding(found, {
      id: 'design/lh-unitless',
      level: 'violation',
      subject: 'line-height.body',
      context: '',
    });
  });

  it('P15 fails 15 px × 1.5 (22.5 px) on the grid', () => {
    const found = checkTokens([
      pointer('fine', [
        fontSize('font-size.body', 15),
        lineHeight('line-height.body', 1.5, { fontSize: '{font-size.body}' }),
      ]),
    ]);
    expectFinding(found, {
      id: 'design/lh-grid',
      level: 'violation',
      subject: 'line-height.body',
      context: '',
    });
  });

  it('P16 only warns on the grid for a one-line role', () => {
    const found = checkTokens([
      pointer('fine', [
        fontSize('font-size.body', 15),
        lineHeight('line-height.body', 1.5, { fontSize: '{font-size.body}', oneLine: true }),
      ]),
    ]);
    expectFinding(found, {
      id: 'design/lh-grid',
      level: 'warning',
      subject: 'line-height.body',
      context: '',
    });
    expect(ofRule(found, 'design/lh-grid')).not.toContainEqual(expect.objectContaining({ level: 'violation' }));
  });

  it('P17 passes 16 px × 1.5, 1 rem × 1.5 and 14 px × 1.4286 (within 0.01 px)', () => {
    const found = checkTokens([
      pointer('fine', [
        fontSize('font-size.a', 16),
        lineHeight('line-height.a', 1.5, { fontSize: '{font-size.a}' }),
        fontSize('font-size.b', 1, 'rem'),
        lineHeight('line-height.b', 1.5, { fontSize: '{font-size.b}' }),
        fontSize('font-size.c', 14),
        lineHeight('line-height.c', 1.4286, { fontSize: '{font-size.c}' }),
      ]),
    ]);
    expect(ofRule(found, 'design/lh-grid')).toEqual([]);
  });
});

describe('design rules: targets', () => {
  it('P18 fails a target of 20 px with a fine pointer', () => {
    const found = checkTokens([pointer('fine', [target('touch-target', 20)])]);
    expectFinding(found, {
      id: 'design/target-fine',
      level: 'violation',
      subject: 'touch-target',
      context: 'fine',
    });
  });

  it('P19 passes a target of 24 px fine and 44 px coarse', () => {
    const found = checkTokens([
      pointer('fine', [target('touch-target', 24)]),
      pointer('coarse', [target('touch-target', 44)]),
    ]);
    expect(found.filter((finding) => finding.id.startsWith('design/target'))).toEqual([]);
  });

  it('P20 fails a target of 40 px with a coarse pointer', () => {
    const found = checkTokens([pointer('coarse', [target('touch-target', 40)])]);
    expectFinding(found, {
      id: 'design/target-coarse',
      level: 'violation',
      subject: 'touch-target',
      context: 'coarse',
    });
  });
});
