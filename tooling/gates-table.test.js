import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
const RULES_DIR = path.join(PACK_ROOT, 'tooling/eslint-plugin-architecture/rules');
const HEADER = ['Rule', 'Checked by', 'Tested by'];
const MIN_ROWS = 20;
const TEST_PATH = /`([^`]+\.test\.(?:js|ts))`/g;

// A cell ends at a pipe that is not escaped as \|.
const splitRow = (line) =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split(/(?<!\\)\|/)
    .map((cell) => cell.trim());

const isSeparator = (cells) => cells.every((cell) => /^:?-+:?$/.test(cell));

// The rows of the table under the header Rule, Checked by, Tested by.
const readRows = () => {
  const lines = readFileSync(path.join(PACK_ROOT, 'docs/gates.md'), 'utf8').split('\n');
  const headerAt = lines.findIndex((line) => {
    const cells = splitRow(line);
    return line.trim().startsWith('|') && HEADER.every((name, index) => cells[index] === name);
  });
  if (headerAt === -1) return [];
  const rows = [];
  for (const line of lines.slice(headerAt + 1)) {
    if (!line.trim().startsWith('|')) break;
    const cells = splitRow(line);
    if (isSeparator(cells)) continue;
    rows.push({ rule: cells[0] ?? '', checkedBy: cells[1] ?? '', testedBy: cells[2] ?? '' });
  }
  return rows;
};

const testPathsOf = (cell) => Array.from(cell.matchAll(TEST_PATH), (match) => match[1]);

const rows = readRows();
const ruleFiles = readdirSync(RULES_DIR)
  .filter((file) => file.endsWith('.js'))
  .map((file) => file.replace(/\.js$/, ''));

describe('gate table of docs/gates.md', () => {
  it(`has the header ${HEADER.join(', ')} and at least ${MIN_ROWS} rows`, () => {
    expect(rows.length).toBeGreaterThanOrEqual(MIN_ROWS);
  });

  describe.each(rows.map((row) => [row.rule.slice(0, 80), row]))('row "%s"', (_title, row) => {
    it('has a Checked by cell', () => {
      expect(row.checkedBy, `Checked by is empty for the rule: ${row.rule}`).not.toBe('');
    });

    it('names a test file in Tested by', () => {
      expect(
        testPathsOf(row.testedBy),
        `Tested by has no .test.js or .test.ts path for the rule: ${row.rule}`,
      ).not.toHaveLength(0);
    });

    it('names tests that exist', () => {
      const missing = testPathsOf(row.testedBy).filter(
        (testPath) => !testPath.startsWith('plugins/kit/') && !existsSync(path.join(PACK_ROOT, testPath)),
      );
      expect(missing, `Tested by names a missing file for the rule: ${row.rule}`).toEqual([]);
    });
  });

  it.each(ruleFiles)('names the rule file %s as architecture/NAME', (name) => {
    const named = rows.some((row) => new RegExp(`architecture/${name}(?![\\w-])`).test(row.checkedBy));
    expect(named, `no row of the table names architecture/${name}`).toBe(true);
  });
});
