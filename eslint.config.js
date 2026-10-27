import eslintComments from '@eslint-community/eslint-plugin-eslint-comments';
import js from '@eslint/js';
import pluginQuery from '@tanstack/eslint-plugin-query';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import architecture, { recommended as architectureRules } from './tooling/eslint-plugin-architecture/index.js';

const SLICE_LAYERS = ['pages', 'widgets', 'features', 'entities'];
// An exception to these rules is a path override here plus a line in docs/deviations.md, never a comment.
const CANON_RULES = [
  'architecture/*',
  'no-restricted-syntax',
  'no-restricted-imports',
  'no-restricted-exports',
  '@typescript-eslint/naming-convention',
  'id-denylist',
  '@typescript-eslint/no-explicit-any',
  '@typescript-eslint/ban-ts-comment',
  'max-lines',
  'react-hooks/*',
  '@tanstack/query/*',
  'jsx-a11y/*',
  '@eslint-community/eslint-comments/*',
];
const ALLOWED_DIRECTIVES = ['disable', 'disable-line', 'disable-next-line', 'enable'].map((kind) => `eslint-${kind}`);
const TEXT_CATALOGS = ['**/config/texts.ts', '**/config/*-texts.ts'];
const NON_PRODUCT_CODE = ['**/*.test.{ts,tsx}', 'mocks/**', 'e2e/**'];
const CYRILLIC = String.raw`/[\u0410-\u044F\u0401\u0451]/`;

const restrictedSyntax = (...entries) => ['error', ...entries];

// ESLint bulk suppressions freeze only errors: a preset rule that comes as a warning is an error here.
const isWarn = (level) => level === 1 || level === 'warn';
const warnsToErrors = (rules) =>
  Object.fromEntries(
    Object.entries(rules).map(([ruleId, entry]) => {
      if (Array.isArray(entry)) {
        const [level, ...options] = entry;
        return [ruleId, isWarn(level) ? ['error', ...options] : entry];
      }
      return [ruleId, isWarn(entry) ? 'error' : entry];
    }),
  );

const RESTRICTED_IMPORTS = {
  paths: [{ name: 'axios', message: 'HTTP only through `shared/api/http-client` (fetch).' }],
  patterns: [
    {
      group: SLICE_LAYERS.filter((layer) => layer !== 'pages').map((layer) => `@/${layer}/*/*`),
      message: 'A slice is reachable only through its public API: `@/entities/<domain>`, not a path inside it.',
    },
    {
      regex: String.raw`^@/pages/[^/]+/(?:(?:ui|model|lib|config|api)(?:/|$)|[^/]+/)`,
      message:
        'A slice is reachable only through its public API: `@/pages/user-list` or `@/pages/GROUP/SLICE`, not a path inside it.',
    },
  ],
};

const BASE_SYNTAX = [
  { selector: 'TSEnumDeclaration', message: 'enum is banned: use a string union or an `as const` array.' },
  { selector: 'ExportAllDeclaration', message: '`export *` hides the slice contract: list the exports explicitly.' },
  {
    selector:
      "NewExpression[callee.name='Date'], CallExpression[callee.object.name='Date'][callee.property.name='parse']",
    message: 'Dates are created and parsed only in `shared/lib/date.ts`.',
  },
  {
    selector:
      "CallExpression[callee.name='fetch'], CallExpression[callee.object.name=/^(window|globalThis|self)$/][callee.property.name='fetch']",
    message: 'Direct `fetch` is banned: HTTP only through `shared/api/http-client`.',
  },
  {
    selector:
      "Property[key.name='staleTime']:matches([value.type='Literal'][value.value=type(number)], [value.type='BinaryExpression'], [value.type='UnaryExpression'], [value.name='Infinity'])",
    message: 'staleTime is a STALE_POLICY level from shared/config/query.ts, not a number',
  },
];

const TEXT_SYNTAX = [
  {
    selector: `Literal[value=${CYRILLIC}]`,
    message: 'UI text is a key of the `config/texts.ts` catalog, not a string in code.',
  },
  {
    selector: `TemplateElement[value.raw=${CYRILLIC}]`,
    message: 'UI text is a function in the `config/texts.ts` catalog.',
  },
  { selector: `JSXText[value=${CYRILLIC}]`, message: 'UI text is a key of the `config/texts.ts` catalog.' },
];

export default tseslint.config(
  {
    ignores: [
      'dist',
      'coverage',
      'public',
      'node_modules',
      'adapters',
      'playwright-report',
      'test-results',
      'reports',
      '.stryker-tmp',
    ],
  },

  { linterOptions: { reportUnusedDisableDirectives: 'error' } },

  js.configs.recommended,

  {
    files: ['**/*.{js,mjs}'],
    languageOptions: { globals: globals.node },
  },

  {
    files: ['**/*.{js,mjs,ts,tsx}'],
    plugins: { '@eslint-community/eslint-comments': eslintComments },
    rules: {
      '@eslint-community/eslint-comments/no-restricted-disable': ['error', ...CANON_RULES],
      '@eslint-community/eslint-comments/require-description': 'error',
      '@eslint-community/eslint-comments/no-unlimited-disable': 'error',
      '@eslint-community/eslint-comments/disable-enable-pair': 'error',
      '@eslint-community/eslint-comments/no-aggregating-enable': 'error',
      '@eslint-community/eslint-comments/no-duplicate-disable': 'error',
      '@eslint-community/eslint-comments/no-unused-enable': 'error',
      '@eslint-community/eslint-comments/no-use': ['error', { allow: ALLOWED_DIRECTIVES }],
    },
  },

  {
    files: ['scripts/**/*.{js,mjs}', 'tooling/**/*.{js,mjs}'],
    plugins: { architecture },
    rules: {
      'architecture/no-swallowed-error': 'error',
      'architecture/no-junk-comments': 'error',
    },
  },

  {
    files: ['**/*.{ts,tsx}'],
    extends: [tseslint.configs.strictTypeChecked, tseslint.configs.stylisticTypeChecked],
    languageOptions: {
      globals: globals.browser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: { architecture, '@tanstack/query': pluginQuery },
    rules: {
      ...warnsToErrors(pluginQuery.configs['flat/recommended'][0].rules),
      ...architectureRules,

      '@typescript-eslint/consistent-type-imports': ['error', { fixStyle: 'inline-type-imports' }],
      '@typescript-eslint/consistent-type-definitions': ['error', 'interface'],
      '@typescript-eslint/switch-exhaustiveness-check': ['error', { considerDefaultExhaustiveForUnions: true }],
      '@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
      '@typescript-eslint/no-confusing-void-expression': ['error', { ignoreArrowShorthand: true }],
      '@typescript-eslint/naming-convention': [
        'error',
        {
          selector: 'variable',
          types: ['boolean'],
          format: ['PascalCase'],
          prefix: ['is', 'has', 'can', 'should', 'was'],
        },
        { selector: 'typeLike', format: ['PascalCase'] },
        { selector: 'interface', format: ['PascalCase'], custom: { regex: '^I[A-Z]', match: false } },
      ],

      'id-denylist': ['error', 'data', 'info', 'temp', 'tmp', 'utils', 'helper', 'helpers', 'manager', 'processData'],
      'no-console': ['error', { allow: ['error', 'warn'] }],
      'no-restricted-syntax': restrictedSyntax(...BASE_SYNTAX, ...TEXT_SYNTAX),
      'no-restricted-imports': ['error', RESTRICTED_IMPORTS],
      'max-lines': ['error', { max: 250, skipBlankLines: true, skipComments: true }],
      eqeqeq: ['error', 'smart'],
    },
  },

  {
    files: ['src/**/*.{ts,tsx}'],
    ...reactHooks.configs.flat['recommended-latest'],
    rules: warnsToErrors(reactHooks.configs.flat['recommended-latest'].rules),
  },
  {
    files: ['src/**/*.tsx'],
    ...jsxA11y.flatConfigs.recommended,
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-exports': [
        'error',
        {
          restrictDefaultExports: {
            direct: true,
            named: true,
            defaultFrom: true,
            namedFrom: true,
            namespaceFrom: true,
          },
        },
      ],
    },
  },

  {
    files: ['src/**/*.{ts,tsx}'],
    ignores: ['src/pages/**', 'src/app/**'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [
            ...RESTRICTED_IMPORTS.paths,
            {
              name: '@tanstack/react-router',
              importNames: ['getRouteApi', 'useParams', 'useSearch', 'useLoaderData'],
              message: 'Only a page (`pages`) reads route params; below it they arrive as an argument.',
            },
          ],
          patterns: RESTRICTED_IMPORTS.patterns,
        },
      ],
    },
  },

  {
    files: ['src/shared/lib/date.ts'],
    rules: {
      'no-restricted-syntax': restrictedSyntax(
        ...BASE_SYNTAX.filter((entry) => !entry.selector.includes("'Date'")),
        ...TEXT_SYNTAX,
      ),
    },
  },
  {
    files: ['src/shared/api/http-client.ts'],
    rules: {
      'no-restricted-syntax': restrictedSyntax(
        ...BASE_SYNTAX.filter((entry) => !entry.selector.includes("'fetch'")),
        ...TEXT_SYNTAX,
      ),
    },
  },
  {
    files: [...TEXT_CATALOGS, ...NON_PRODUCT_CODE],
    rules: {
      'no-restricted-syntax': restrictedSyntax(...BASE_SYNTAX.filter((entry) => !entry.selector.includes("'Date'"))),
    },
  },
  {
    files: ['mocks/**'],
    rules: {
      'architecture/no-dto-outside-api': 'off',
      'no-restricted-imports': [
        'error',
        {
          paths: RESTRICTED_IMPORTS.paths,
          patterns: [
            {
              regex: String.raw`^@/(${SLICE_LAYERS.join('|')})/[^/]+/(?!api/[^/]+-dto$)`,
              message: 'Mocks take only the wire format from a slice: `@/entities/<domain>/api/<domain>-dto`.',
            },
          ],
        },
      ],
    },
  },
  {
    files: [...TEXT_CATALOGS, ...NON_PRODUCT_CODE],
    rules: { 'max-lines': 'off' },
  },
  {
    files: NON_PRODUCT_CODE,
    rules: {
      '@typescript-eslint/no-non-null-assertion': 'off',
      '@typescript-eslint/no-unsafe-type-assertion': 'off',
    },
  },
);
