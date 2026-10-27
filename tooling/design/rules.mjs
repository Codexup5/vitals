import { apcaLc, wcagRatio } from './color.mjs';

const ROOT_FONT_SIZE = 16;
const GRID = 4;
const GRID_TOLERANCE = 0.01;
const TARGET_FINE = 24;
const TARGET_COARSE = 44;
const CONTRAST_TEXT = 4.5;
const CONTRAST_LARGE = 3;
const CONTRAST_UI = 3;
const LC_TEXT = 75;
const LC_LARGE = 60;
const LC_UI = 45;

export const RULES = [
  { id: 'design/grid', level: 'violation', source: 'Material 3 layout, the 4 dp grid' },
  { id: 'design/lh-unitless', level: 'violation', source: 'WCAG 2.2 SC 1.4.4 and 1.4.12; CSSWG #11636' },
  { id: 'design/lh-grid', level: 'violation', source: 'Material 3 type scale; the user, 2026-10-17' },
  { id: 'design/contrast-text', level: 'violation', source: 'WCAG 2.2 SC 1.4.3' },
  { id: 'design/contrast-ui', level: 'violation', source: 'WCAG 2.2 SC 1.4.11' },
  { id: 'design/apca', level: 'warning', source: 'APCA 0.0.98G, a hint outside WCAG; WCAG wins a conflict' },
  { id: 'design/target-fine', level: 'violation', source: 'WCAG 2.2 SC 2.5.8' },
  { id: 'design/target-coarse', level: 'violation', source: 'Apple HIG Buttons (44×44 pt); the user, 2026-10-17' },
];

const LEVELS = Object.fromEntries(RULES.map(({ id, level }) => [id, level]));

const finding = (id, subject, context, tokens, detail, level = LEVELS[id]) => ({
  id,
  level,
  subject,
  context,
  file: tokens[0].file,
  detail,
});

const withRole = (context, role) => context.tokens.filter((candidate) => candidate.kit?.role === role);

const isDimension = (candidate) => candidate.type === 'dimension';

const toPx = ({ value }) => (value.unit === 'rem' ? value.value * ROOT_FONT_SIZE : value.value);

const aliasPath = (alias) => alias.replace(/^\{|\}$/g, '');

const round = (number, digits = 2) => Number(number.toFixed(digits));

// The detail of a failure never rounds a miss up to the bar: 4.499 prints 4.49
const floorTo = (number, digits = 2) => Math.floor(number * 10 ** digits) / 10 ** digits;

const onGrid = (px) => {
  const offset = Math.abs(px / GRID - Math.round(px / GRID)) * GRID;
  return offset <= GRID_TOLERANCE;
};

// 1–2 px is for lines and optical corrections
const isLineOrCorrection = (px) => px >= 1 && px <= 2;

const checkGrid = (contexts) =>
  contexts.flatMap((context) =>
    ['space', 'size', 'target']
      .flatMap((role) => withRole(context, role))
      .filter(isDimension)
      .filter((candidate) => !onGrid(toPx(candidate)) && !isLineOrCorrection(toPx(candidate)))
      .map((candidate) =>
        finding(
          'design/grid',
          candidate.path,
          '',
          [candidate],
          `${round(toPx(candidate), 4)} px is not a multiple of ${GRID} px`,
        ),
      ),
  );

const checkLhUnitless = (contexts) =>
  contexts.flatMap((context) =>
    withRole(context, 'line-height')
      .filter((candidate) => candidate.type !== 'number')
      .map((candidate) =>
        finding('design/lh-unitless', candidate.path, '', [candidate], `${candidate.type} instead of a number`),
      ),
  );

const checkLhGrid = (contexts) =>
  contexts.flatMap((context) =>
    withRole(context, 'line-height')
      .filter((candidate) => candidate.type === 'number' && candidate.kit.fontSize)
      .flatMap((candidate) => {
        const size = context.tokens.find((other) => other.path === aliasPath(candidate.kit.fontSize));
        if (!size || !isDimension(size)) return [];
        const box = toPx(size) * candidate.value;
        if (onGrid(box)) return [];
        const level = candidate.kit.oneLine ? 'warning' : 'violation';
        return [
          finding(
            'design/lh-grid',
            candidate.path,
            '',
            [candidate],
            `${size.path} ${toPx(size)} px × ${candidate.value} = ${round(box, 4)} px, not a multiple of ${GRID} px`,
            level,
          ),
        ];
      }),
  );

const isColor = (candidate) => candidate.type === 'color';

// The surfaces a text or ui token meets: the aliases of its `surfaces`, else every surface of the context
const surfacesOf = (context, candidate) => {
  if (candidate.kit.surfaces) {
    return candidate.kit.surfaces
      .map((alias) => context.tokens.find((other) => other.path === aliasPath(alias)))
      .filter((other) => other !== undefined && isColor(other));
  }
  return withRole(context, 'surface').filter(isColor);
};

const contrastPairs = (contexts) =>
  contexts
    .filter((context) => context.kind === 'theme')
    .flatMap((context) =>
      ['text', 'ui'].flatMap((role) =>
        withRole(context, role)
          .filter((candidate) => isColor(candidate) && !candidate.kit.incidental)
          .flatMap((candidate) =>
            surfacesOf(context, candidate).map((surface) => ({ context, role, candidate, surface })),
          ),
      ),
    );

const wcagBar = (role, candidate) => {
  if (role === 'ui') return CONTRAST_UI;
  return candidate.kit.large ? CONTRAST_LARGE : CONTRAST_TEXT;
};

const lcBar = (role, candidate) => {
  if (role === 'ui') return LC_UI;
  return candidate.kit.large ? LC_LARGE : LC_TEXT;
};

const checkContrast = (id, judgedRole) => (contexts) =>
  contrastPairs(contexts)
    .filter(({ role }) => role === judgedRole)
    .flatMap(({ context, role, candidate, surface }) => {
      const bar = wcagBar(role, candidate);
      const ratio = wcagRatio(candidate.value, surface.value);
      if (ratio >= bar) return [];
      return [
        finding(
          id,
          `${candidate.path} on ${surface.path}`,
          context.name,
          [candidate, surface],
          `${floorTo(ratio).toFixed(2)} < ${bar}`,
        ),
      ];
    });

const checkApca = (contexts) =>
  contrastPairs(contexts).flatMap(({ context, role, candidate, surface }) => {
    const bar = lcBar(role, candidate);
    const lc = apcaLc(candidate.value, surface.value);
    if (lc >= bar) return [];
    return [
      finding(
        'design/apca',
        `${candidate.path} on ${surface.path}`,
        context.name,
        [candidate, surface],
        `Lc ${floorTo(lc, 1).toFixed(1)} < ${bar}`,
      ),
    ];
  });

const checkTarget = (id, pointerName, minimum) => (contexts) =>
  contexts
    .filter((context) => context.kind === 'pointer' && context.name === pointerName)
    .flatMap((context) =>
      withRole(context, 'target')
        .filter(isDimension)
        .filter((candidate) => toPx(candidate) < minimum)
        .map((candidate) =>
          finding(id, candidate.path, context.name, [candidate], `${round(toPx(candidate), 4)} px < ${minimum} px`),
        ),
    );

// In the order of RULES
const CHECKS = [
  checkGrid,
  checkLhUnitless,
  checkLhGrid,
  checkContrast('design/contrast-text', 'text'),
  checkContrast('design/contrast-ui', 'ui'),
  checkApca,
  checkTarget('design/target-fine', 'fine', TARGET_FINE),
  checkTarget('design/target-coarse', 'coarse', TARGET_COARSE),
];

// A context-free rule sees a base token in every context: one finding per subject and file
const findingKey = ({ id, subject, context, file }) => `${id}|${subject}|${context}|${file}`;

export const checkTokens = (contexts) => {
  const findings = CHECKS.flatMap((check) => check(contexts));
  return [...new Map(findings.map((item) => [findingKey(item), item])).values()];
};
