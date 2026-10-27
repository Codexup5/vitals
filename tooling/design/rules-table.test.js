import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { RULES } from './rules.mjs';

const PACK_ROOT = path.resolve(import.meta.dirname, '../..');
const STYLES = path.join(PACK_ROOT, '.claude/rules/styles.md');
const GATES = path.join(PACK_ROOT, 'docs/gates.md');
const HEADER = ['Id', 'Rule', 'Level', 'Source', 'Checked by'];
const ID_CELL = /^`([^`]+)`$/;

// A cell ends at a pipe that is not escaped as \|.
const splitRow = (line) =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split(/(?<!\\)\|/)
    .map((cell) => cell.trim());

const isSeparator = (cells) => cells.every((cell) => /^:?-+:?$/.test(cell));

const idOf = (cell) => ID_CELL.exec(cell)?.[1] ?? null;

// The rows of the table under the header Id, Rule, Level, Source, Checked by
const readDesignRows = () => {
  const lines = readFileSync(STYLES, 'utf8').split('\n');
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
    rows.push(cells);
  }
  return rows;
};

const rows = readDesignRows();

describe('design rules table of .claude/rules/styles.md', () => {
  it(`has the header ${HEADER.join(', ')}`, () => {
    expect(rows.length, 'no design rules table under the header').toBeGreaterThan(0);
  });

  it('lists the ids of RULES, in their order, each in backticks', () => {
    expect(rows.map(([id]) => idOf(id))).toEqual(RULES.map((rule) => rule.id));
  });

  it('gives each rule the level of RULES', () => {
    expect(rows.map(([, , level]) => level)).toEqual(RULES.map((rule) => rule.level));
  });

  it('gives each rule the source of RULES', () => {
    expect(rows.map(([, , , source]) => source)).toEqual(RULES.map((rule) => rule.source));
  });
});

describe('gates of docs/gates.md', () => {
  const gates = readFileSync(GATES, 'utf8');

  it.each(RULES.map((rule) => [rule.id]))('names %s', (id) => {
    expect(gates, `docs/gates.md does not name ${id}`).toContain(id);
  });
});
