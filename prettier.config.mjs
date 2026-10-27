export default {
  printWidth: 120,
  singleQuote: true,
  trailingComma: 'all',
  plugins: ['@ianvs/prettier-plugin-sort-imports'],
  importOrder: [
    '<BUILTIN_MODULES>',
    '<THIRD_PARTY_MODULES>',
    '',
    '^@/(.*)$',
    '',
    '^[.](?!.*[.]s?css$)',
    '',
    '[.]s?css$',
  ],
  importOrderTypeScriptVersion: '6.0.0',
  overrides: [
    // release-please rewrites these with JSON.stringify: one array item per line, as in package.json
    { files: ['kit.json', '**/.claude-plugin/*.json'], options: { parser: 'json-stringify' } },
  ],
};
