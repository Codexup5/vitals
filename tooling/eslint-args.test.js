import { describe, expect, it } from 'vitest';

import { baselineRuns, eslintArgv } from './eslint-args.mjs';

const FLAG = ['--flag', 'v10_config_lookup_from_file'];
const ARGS = ['.', '--max-warnings=0'];

describe('eslintArgv', () => {
  it('adds the config lookup flag before the arguments on ESLint 9', () => {
    expect(eslintArgv('9.39.5', ARGS)).toEqual([...FLAG, ...ARGS]);
  });

  it('leaves the arguments alone on ESLint 10', () => {
    expect(eslintArgv('10.0.0', ARGS)).toEqual(ARGS);
  });

  it('leaves the arguments alone on a later ESLint 10 minor', () => {
    expect(eslintArgv('10.13.0', ARGS)).toEqual(ARGS);
  });

  it('adds the flag on a prerelease of ESLint 9', () => {
    expect(eslintArgv('9.0.0-rc.1', ARGS)).toEqual([...FLAG, ...ARGS]);
  });

  it('throws on a version that is not a version, and names it', () => {
    expect(() => eslintArgv('x', ARGS)).toThrow(/\bx\b/);
  });
});

describe('baselineRuns', () => {
  it('suppresses and then prunes when there are no suppressions yet, on the baseline mode', () => {
    expect(baselineRuns('baseline', false)).toEqual([
      ['.', '--suppress-all'],
      ['.', '--prune-suppressions'],
    ]);
  });

  it('only prunes when there are suppressions already, on the baseline mode', () => {
    expect(baselineRuns('baseline', true)).toEqual([['.', '--prune-suppressions']]);
  });

  it('suppresses and then prunes on the sync mode when there are suppressions', () => {
    expect(baselineRuns('sync', true)).toEqual([
      ['.', '--suppress-all'],
      ['.', '--prune-suppressions'],
    ]);
  });

  it('runs nothing on the sync mode without suppressions', () => {
    expect(baselineRuns('sync', false)).toEqual([]);
  });
});
