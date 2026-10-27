import { describe, expect, it } from 'vitest';

import { judgeDebt } from './judge.mjs';

const keyOf = ({ file, code, message }) => `${file}|${code}|${message}`;

const F1 = { file: 'src/a.scss', code: 'color-named', message: 'Unexpected named color "red"' };
const F3 = { file: 'src/b.scss', code: 'color-no-hex', message: 'Unexpected hex color "#fff"' };
const F4 = { file: 'src/c.scss', code: 'color-named', message: 'Unexpected named color "blue"' };
const KIT = { file: 'tooling/x.scss', code: 'color-named', message: 'Unexpected named color "red"' };

const F1_KEY = keyOf(F1);
const F3_KEY = keyOf(F3);

const LABELS = { file: 'stylelint-baseline.json', freeze: 'lint:styles:baseline', noun: 'stylelint findings' };
const KIT_FILES = new Set(['tooling/x.scss']);
const NO_KIT_FILES = new Set();

const line = (finding) => `${finding.file}: ${finding.code} — ${finding.message}`;

const judge = ({ mode, findings, baseline = null, kitFiles = NO_KIT_FILES }) =>
  judgeDebt({ mode, findings, baseline, kitFiles, labels: LABELS });

describe('judgeDebt in check mode', () => {
  it('fails on a finding and prints it in the finding form when there is no baseline', () => {
    const result = judge({ mode: 'check', findings: [F1] });

    expect(result.exitCode).toBe(1);
    expect(result.lines).toContain(line(F1));
  });

  it('passes without findings and without a baseline', () => {
    expect(judge({ mode: 'check', findings: [] }).exitCode).toBe(0);
  });

  it('passes on the findings the baseline counts and says how many are known', () => {
    const result = judge({ mode: 'check', findings: [F1, F1], baseline: { [F1_KEY]: 2 } });

    expect(result.exitCode).toBe(0);
    expect(result.lines).toContain('2 known stylelint findings (stylelint-baseline.json)');
  });

  it('fails on the finding past the baseline, once, and says how many are new', () => {
    const result = judge({ mode: 'check', findings: [F1, F1, F1], baseline: { [F1_KEY]: 2 } });

    expect(result.exitCode).toBe(1);
    expect(result.lines.filter((text) => text === line(F1))).toHaveLength(1);
    expect(result.lines).toContain('1 new stylelint findings past stylelint-baseline.json');
  });

  it('passes and reports the fixed findings still counted in the baseline', () => {
    const result = judge({ mode: 'check', findings: [F1], baseline: { [F1_KEY]: 2, [F3_KEY]: 1 } });

    expect(result.exitCode).toBe(0);
    expect(result.lines).toContain(
      '2 fixed stylelint findings still counted in stylelint-baseline.json: run pnpm lint:styles:baseline',
    );
  });
});

describe('judgeDebt in baseline mode', () => {
  it('writes the current counts when there is no baseline, and says how many are frozen', () => {
    const result = judge({ mode: 'baseline', findings: [F1, F1, F3] });

    expect(result.exitCode).toBe(0);
    expect(result.write).toEqual({ [F1_KEY]: 2, [F3_KEY]: 1 });
    expect(result.lines).toContain('stylelint-baseline.json: 3 known stylelint findings');
  });

  it('lowers the counts of the baseline to the current ones and drops the zeros', () => {
    const result = judge({ mode: 'baseline', findings: [F1], baseline: { [F1_KEY]: 2, [F3_KEY]: 1 } });

    expect(result.write).toEqual({ [F1_KEY]: 1 });
  });

  it('never adds to the baseline', () => {
    const result = judge({
      mode: 'baseline',
      findings: [F1, F1, F1, F3, F4],
      baseline: { [F1_KEY]: 2, [F3_KEY]: 1 },
    });

    expect(result.write).toEqual({ [F1_KEY]: 2, [F3_KEY]: 1 });
  });

  it('refuses and writes nothing when a finding is in a kit file', () => {
    const result = judge({ mode: 'baseline', findings: [KIT], kitFiles: KIT_FILES });

    expect(result.exitCode).toBe(1);
    expect(result.write).toBeUndefined();
    expect(result.lines.some((text) => text.includes('tooling/x.scss'))).toBe(true);
    expect(result.lines).toContain('fix the kit files or resolve the .kit-new files first');
  });
});

describe('judgeDebt in sync mode', () => {
  it('adds the new findings to the baseline and says how many it added', () => {
    const result = judge({ mode: 'sync', findings: [F1, F1, F3], baseline: { [F1_KEY]: 1 } });

    expect(result.write).toEqual({ [F1_KEY]: 2, [F3_KEY]: 1 });
    expect(result.lines).toContain('stylelint-baseline.json: 2 findings added by sync');
  });

  it('does nothing and says so when there is no baseline', () => {
    const result = judge({ mode: 'sync', findings: [F1] });

    expect(result.exitCode).toBe(0);
    expect(result.write).toBeUndefined();
    expect(result.lines).toContain('no stylelint-baseline.json: nothing to sync');
  });

  it('lowers the baseline when a finding is fixed', () => {
    const result = judge({ mode: 'sync', findings: [F1], baseline: { [F1_KEY]: 2 } });

    expect(result.write).toEqual({ [F1_KEY]: 1 });
  });

  it('refuses and writes nothing when a finding is in a kit file', () => {
    const result = judge({ mode: 'sync', findings: [KIT], kitFiles: KIT_FILES });

    expect(result.exitCode).toBe(1);
    expect(result.write).toBeUndefined();
    expect(result.lines.some((text) => text.includes('tooling/x.scss'))).toBe(true);
    expect(result.lines).toContain('fix the kit files or resolve the .kit-new files first');
  });
});
