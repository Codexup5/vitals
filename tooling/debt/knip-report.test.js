import { describe, expect, it } from 'vitest';

import { parseKnipReport } from './tools.mjs';

const REPORT = {
  issues: [
    {
      file: 'package.json',
      owners: [{ name: '@team' }],
      devDependencies: [{ name: '@testing-library/react', line: 5, col: 6, pos: 90 }],
      exports: [],
    },
    { file: 'src\\shared\\lib\\orphan.ts', files: [{ name: 'src/shared/lib/orphan.ts' }] },
    {
      file: 'src/entities/probe/index.ts',
      exports: [{ name: 'unused', line: 2 }],
      types: [{ name: 'Unused', line: 3 }],
      enumMembers: [{ namespace: 'Color', name: 'Red' }],
      duplicates: [[{ name: 'a' }, { name: 'b' }]],
    },
  ],
};

const PROBE = 'src/entities/probe/index.ts';

describe('parseKnipReport', () => {
  it('turns the knip 6 report into the six findings, sorted by file, code and message', () => {
    const findings = parseKnipReport(JSON.stringify(REPORT));

    expect(findings).toEqual([
      { file: 'package.json', code: 'devDependencies', message: '@testing-library/react' },
      { file: PROBE, code: 'duplicates', message: 'a, b' },
      { file: PROBE, code: 'enumMembers', message: 'Color.Red' },
      { file: PROBE, code: 'exports', message: 'unused' },
      { file: PROBE, code: 'types', message: 'Unused' },
      { file: 'src/shared/lib/orphan.ts', code: 'files', message: 'src/shared/lib/orphan.ts' },
    ]);
  });

  it('keeps line, col and pos out of a finding and counts "file" and "owners" as no issue type', () => {
    const findings = parseKnipReport(JSON.stringify(REPORT));

    expect(findings.every((finding) => Object.keys(finding).sort().join() === 'code,file,message')).toBe(true);
    expect(findings.map(({ code }) => code)).not.toContain('file');
    expect(findings.map(({ code }) => code)).not.toContain('owners');
  });

  it('gives no finding for an empty issues array', () => {
    expect(parseKnipReport('{"issues": []}')).toEqual([]);
  });

  it('throws an error that starts with "knip printed no JSON" for a text that is no JSON', () => {
    expect(() => parseKnipReport('Loading config...')).toThrow(/^knip printed no JSON/);
  });

  it('throws an error that starts with "knip printed no issues array" for JSON without issues', () => {
    expect(() => parseKnipReport('{"files": []}')).toThrow(/^knip printed no issues array/);
    expect(() => parseKnipReport('{"issues": {}}')).toThrow(/^knip printed no issues array/);
  });
});
