import { globSync, readFileSync } from 'node:fs';

// schemas live in model/ next to the queries, so they are picked by the zod import, not by file name
const schemaFiles = globSync('src/**/model/*.ts', { exclude: (file) => file.endsWith('.test.ts') }).filter((file) =>
  /from 'zod'/.test(readFileSync(file, 'utf8')),
);

export default {
  // @stryker-mutator/vitest-runner 10.0.0 runs no tests per mutant on Vitest 5 (stryker-js#6210),
  // hence the command runner: each mutant is a separate vitest run. Bring the vitest runner back once fixed.
  // incremental is off with the command runner: it does not know the mutant → test link and misses new tests
  // only the app unit tests next to the mutated globs run per mutant: screen tests made the weekly run hit its timeout
  testRunner: 'command',
  commandRunner: {
    command: 'node node_modules/vitest/vitest.mjs run --project app lib/ api/ model/ --reporter=dot',
  },
  coverageAnalysis: 'off',
  timeoutMS: 120_000,
  mutate: [
    'src/**/lib/**/*.ts',
    'src/**/api/*-mapper.ts',
    'src/**/api/*-dto.ts',
    ...schemaFiles.map((file) => file.replaceAll('\\', '/')),
    '!src/**/*.test.ts',
  ],
  thresholds: { high: 90, low: 80, break: 80 },
  reporters: ['clear-text', 'progress', 'html', 'json'],
  htmlReporter: { fileName: 'reports/mutation/index.html' },
  jsonReporter: { fileName: 'reports/mutation/mutation.json' },
  tempDirName: '.stryker-tmp',
};
