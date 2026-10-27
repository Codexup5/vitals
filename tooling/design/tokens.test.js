import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '../..');
const CLI = path.join(PACK_ROOT, 'tooling/design/tokens.mjs');
const FIXTURE = path.join(PACK_ROOT, 'tooling/design/fixtures/prefixed');
const SCSS = path.join('src', 'shared', 'styles', 'tokens.scss');
const BASELINE = 'design-baseline.json';
const STALE_LINE = ['--dv-space-1: 4px;', '--dv-space-1: 5px;'];
// Each case starts node and Style Dictionary per CLI run, up to a dozen times: past the 5 s default on a loaded runner
const CLI_TIMEOUT = 30_000;

const temps = [];

const tempRoot = () => {
  const root = mkdtempSync(path.join(tmpdir(), 'design-tokens-'));
  temps.push(root);
  return root;
};

const copyFixture = () => {
  const root = tempRoot();
  cpSync(FIXTURE, root, { recursive: true });
  return root;
};

const readText = (file) => readFileSync(file, 'utf8').replace(/\r\n/g, '\n');

const runCli = (args) => {
  const result = spawnSync(process.execPath, [CLI, ...args], { cwd: PACK_ROOT, encoding: 'utf8' });
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
};

const editJson = (root, file, edit) => {
  const target = path.join(root, 'src', 'shared', 'styles', 'tokens', file);
  const data = JSON.parse(readFileSync(target, 'utf8'));
  edit(data);
  writeFileSync(target, `${JSON.stringify(data, null, 2)}\n`);
};

// Rebuilds tokens.scss after a token edit, so a check judges the rules and not the freshness
const buildCopy = (root) => {
  const { status, output } = runCli(['--root', root]);
  expect(status, output).toBe(0);
};

afterEach(() => {
  for (const root of temps.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe('tokens CLI on the prefixed fixture', { timeout: CLI_TIMEOUT }, () => {
  it('--check exits 0 on a copy whose tokens.scss is fresh', () => {
    const { status, output } = runCli(['--check', '--root', copyFixture()]);
    expect(status, output).toBe(0);
  });

  it('--check exits 1 and names the file and the command after one line changes', () => {
    const root = copyFixture();
    const file = path.join(root, SCSS);
    const original = readText(file);
    const stale = original.replace(...STALE_LINE);
    expect(stale, 'the probe line is not in the fixture').not.toBe(original);
    writeFileSync(file, stale);
    const { status, output } = runCli(['--check', '--root', root]);
    expect(status, output).toBe(1);
    expect(output).toContain('src/shared/styles/tokens.scss');
    expect(output).toContain('pnpm tokens');
  });

  it('--check exits 1 when tokens.scss is deleted', () => {
    const root = copyFixture();
    rmSync(path.join(root, SCSS));
    const { status, output } = runCli(['--check', '--root', root]);
    expect(status, output).toBe(1);
    expect(output).toContain('pnpm tokens');
  });

  it('the default mode writes a missing tokens.scss equal to the fixture one', () => {
    const root = copyFixture();
    rmSync(path.join(root, SCSS));
    const { status, output } = runCli(['--root', root]);
    expect(status, output).toBe(0);
    expect(readText(path.join(root, SCSS))).toBe(readText(path.join(FIXTURE, SCSS)));
  });

  it('a folder without config.json has the design tokens off and exits 0', () => {
    const { status, output } = runCli(['--check', '--root', tempRoot()]);
    expect(status, output).toBe(0);
    expect(output).toContain('design tokens are off');
  });

  it.each([
    [
      'a hex that its components do not give',
      'light.tokens.json',
      (data) => {
        data.color.text.max.$value.hex = '#eeeeee';
      },
      'token color.text.max has hex #eeeeee, but its components give #101114',
    ],
    [
      'a dimension written as a string',
      'dark.tokens.json',
      (data) => {
        data.space['1'].$value = '6px';
      },
      'token space.1 is a dimension',
    ],
    [
      'an alias to a missing token',
      'dark.tokens.json',
      (data) => {
        data.color.text.max.$value = '{color.nope}';
      },
      'token color.text.max refers to {color.nope}, which is not a token',
    ],
    [
      'a layer with neither theme nor pointer',
      'config.json',
      (data) => {
        delete data.layers[1].theme;
      },
      'layers[1]: a layer has "theme" or "pointer"',
    ],
  ])('stops the build on %s, naming it', (_, file, edit, message) => {
    const root = copyFixture();
    editJson(root, file, edit);
    const { status, output } = runCli(['--check', '--root', root]);
    expect(status, output).toBe(1);
    expect(output).toContain(message);
  });
});

describe('tokens CLI judges the design rules', { timeout: CLI_TIMEOUT }, () => {
  it('--check exits 0 on the copy and prints no violation', () => {
    const { status, output } = runCli(['--check', '--root', copyFixture()]);
    expect(status, output).toBe(0);
    expect(output).not.toMatch(/^violation /m);
  });

  it('a 6 px space and an unflagged ghost text fail for dark and for light', () => {
    const root = copyFixture();
    editJson(root, 'dark.tokens.json', (data) => {
      data.space['1'].$value = { value: 6, unit: 'px' };
      delete data.color.text.ghost.$extensions.kit.incidental;
    });
    buildCopy(root);
    const { status, output } = runCli(['--check', '--root', root]);
    expect(status, output).toBe(1);
    expect(output).toContain('violation design/grid — space.1');
    expect(output).toContain('violation design/contrast-text — color.text.ghost on color.s1 (dark)');
    expect(output).toContain('violation design/contrast-text — color.text.ghost on color.s1 (light)');
  });

  it('a light text at #767676 exits 0 with a count, and lists design/apca only with --warnings', () => {
    const root = copyFixture();
    editJson(root, 'light.tokens.json', (data) => {
      data.color.text.max.$value = { colorSpace: 'srgb', components: [0.4627, 0.4627, 0.4627], hex: '#767676' };
    });
    buildCopy(root);
    const plain = runCli(['--check', '--root', root]);
    expect(plain.status, plain.output).toBe(0);
    expect(plain.output).toMatch(/\d+ warnings: pnpm tokens:check --warnings lists them/);
    expect(plain.output).not.toContain('warning design/apca');
    const listed = runCli(['--check', '--warnings', '--root', root]);
    expect(listed.status, listed.output).toBe(0);
    expect(listed.output).toContain('warning design/apca — color.text.max on color.s1 (light)');
  });

  it('--check at the pack root, the reference, exits 0 and prints no violation', () => {
    const { status, output } = runCli(['--check']);
    expect(status, output).toBe(0);
    expect(output).not.toMatch(/^violation /m);
  });
});

describe('tokens CLI freezes the design debt in a shrink-only baseline', { timeout: CLI_TIMEOUT }, () => {
  it('freezes the ghost debt, reports its fix, and never hides a new violation', () => {
    const root = copyFixture();
    const baselineEntries = () => Object.keys(JSON.parse(readFileSync(path.join(root, BASELINE), 'utf8')).violations);

    editJson(root, 'dark.tokens.json', (data) => {
      delete data.color.text.ghost.$extensions.kit.incidental;
    });
    buildCopy(root);
    const debt = runCli(['--check', '--root', root]);
    expect(debt.status, debt.output).toBe(1);
    expect(debt.output).toContain('violation design/contrast-text — color.text.ghost on color.s1 (dark)');
    expect(debt.output).toContain('violation design/contrast-text — color.text.ghost on color.s1 (light)');

    const frozen = runCli(['--baseline', '--root', root]);
    expect(frozen.status, frozen.output).toBe(0);
    expect(baselineEntries()).toHaveLength(2);
    const known = runCli(['--check', '--root', root]);
    expect(known.status, known.output).toBe(0);
    expect(known.output).toMatch(/2 known design violations/);

    editJson(root, 'light.tokens.json', (data) => {
      data.color.text.ghost.$value = { colorSpace: 'srgb', components: [0.3569, 0.3804, 0.4196], hex: '#5b616b' };
    });
    buildCopy(root);
    const fixed = runCli(['--check', '--root', root]);
    expect(fixed.status, fixed.output).toBe(0);
    expect(fixed.output).toMatch(
      /1 fixed design violations still counted in design-baseline\.json: run pnpm tokens:baseline/,
    );
    const shrunk = runCli(['--baseline', '--root', root]);
    expect(shrunk.status, shrunk.output).toBe(0);
    const entries = baselineEntries();
    expect(entries).toHaveLength(1);
    expect(entries[0]).toContain('design/contrast-text|color.text.ghost on color.s1 (dark)');

    editJson(root, 'dark.tokens.json', (data) => {
      data.space['1'].$value = { value: 6, unit: 'px' };
    });
    buildCopy(root);
    const grid = runCli(['--check', '--root', root]);
    expect(grid.status, grid.output).toBe(1);
    const violationLines = grid.output.split('\n').filter((line) => line.startsWith('violation '));
    expect(violationLines).toHaveLength(1);
    expect(violationLines[0]).toContain('design/grid — space.1');

    const grown = runCli(['--baseline', '--root', root]);
    expect(grown.status, grown.output).toBe(0);
    expect(baselineEntries()).toHaveLength(1);
    expect(runCli(['--check', '--root', root]).status).toBe(1);
  });
});

describe('tokens CLI refuses to freeze kit files', { timeout: CLI_TIMEOUT }, () => {
  const DARK = 'src/shared/styles/tokens/dark.tokens.json';
  const REFUSAL = 'design-baseline.json was not written, it would freeze kit files:';
  const FIX_LINE = 'fix the kit files or resolve the .kit-new files first';

  const sha256 = (text) => createHash('sha256').update(text.replace(/\r\n/g, '\n')).digest('hex');

  // a copy with one 6 px space violation in dark.tokens.json, and a kit-lock.json that holds that file's bytes
  const copyWithKitViolation = () => {
    const root = copyFixture();
    editJson(root, 'dark.tokens.json', (data) => {
      data.space['1'].$value = { value: 6, unit: 'px' };
    });
    buildCopy(root);
    const lock = { files: { [DARK]: sha256(readFileSync(path.join(root, DARK), 'utf8')) } };
    writeFileSync(path.join(root, 'kit-lock.json'), `${JSON.stringify(lock, null, 2)}\n`);
    return root;
  };

  const touchDark = (root) => writeFileSync(path.join(root, DARK), `${readFileSync(path.join(root, DARK), 'utf8')}\n`);

  it('--baseline exits 1 on a violation in an untouched kit file, writing nothing', () => {
    const root = copyWithKitViolation();
    const { status, output } = runCli(['--baseline', '--root', root]);
    expect(status, output).toBe(1);
    expect(existsSync(path.join(root, BASELINE))).toBe(false);
    expect(output).toContain(REFUSAL);
    expect(output).toContain(DARK);
    expect(output).toContain('design/grid');
    expect(output).toContain('space.1');
    expect(output).toContain(FIX_LINE);
  });

  it('--baseline refuses while a .kit-new waits beside the changed kit file', () => {
    const root = copyWithKitViolation();
    touchDark(root);
    writeFileSync(path.join(root, `${DARK}.kit-new`), '{}\n');
    const { status, output } = runCli(['--baseline', '--root', root]);
    expect(status, output).toBe(1);
    expect(existsSync(path.join(root, BASELINE))).toBe(false);
    expect(output).toContain(REFUSAL);
  });

  it('--baseline exits 0 once the project changed the file and no .kit-new is beside it', () => {
    const root = copyWithKitViolation();
    touchDark(root);
    const { status, output } = runCli(['--baseline', '--root', root]);
    expect(status, output).toBe(0);
    expect(Object.keys(JSON.parse(readFileSync(path.join(root, BASELINE), 'utf8')).violations)).toHaveLength(1);
  });

  it('--sync refuses the same way in a repository whose only change is kit-lock.json', () => {
    const root = copyWithKitViolation();
    const lockText = readFileSync(path.join(root, 'kit-lock.json'), 'utf8');
    rmSync(path.join(root, 'kit-lock.json'));
    writeFileSync(path.join(root, BASELINE), `${JSON.stringify({ violations: {} })}\n`);
    const identity = ['-c', 'user.name=t', '-c', 'user.email=t@t.test', '-c', 'commit.gpgsign=false'];
    for (const args of [
      ['init', '-q'],
      ['add', '-A'],
      ['commit', '-q', '-m', 'adopt'],
    ]) {
      const result = spawnSync('git', [...identity, ...args], { cwd: root, encoding: 'utf8' });
      expect(result.status, result.stderr).toBe(0);
    }
    writeFileSync(path.join(root, 'kit-lock.json'), lockText);
    const before = readFileSync(path.join(root, BASELINE), 'utf8');
    const { status, output } = runCli(['--sync', '--root', root]);
    expect(status, output).toBe(1);
    expect(output).toContain(REFUSAL);
    expect(output).toContain(DARK);
    expect(output).toContain(FIX_LINE);
    expect(readFileSync(path.join(root, BASELINE), 'utf8')).toBe(before);
  });

  it('--baseline with a kit-lock.json that is not JSON exits 1 on stderr, with no stack trace', () => {
    const root = copyFixture();
    writeFileSync(path.join(root, 'kit-lock.json'), '{ "files": ');
    writeFileSync(path.join(root, BASELINE), `${JSON.stringify({ violations: {} })}\n`);
    const result = spawnSync(process.execPath, [CLI, '--baseline', '--root', root], {
      cwd: PACK_ROOT,
      encoding: 'utf8',
    });
    expect(result.status, result.stdout + result.stderr).toBe(1);
    expect(result.stderr).toContain('kit-lock.json cannot be read');
    // node indents its stack frames by four spaces, so the frame pattern is a line with leading blanks then "at "
    expect(result.stderr).not.toMatch(/^\s+at /m);
  });
});

describe('tokens CLI syncs a kit update into the baseline', { timeout: CLI_TIMEOUT }, () => {
  const LIGHT = 'src/shared/styles/tokens/light.tokens.json';
  const sha256 = (text) => createHash('sha256').update(text.replace(/\r\n/g, '\n')).digest('hex');

  // a copy with three off-grid space tokens in dark.tokens.json, zeta before alpha so the file order is not the key
  // order; the lock does not list that file but lists light.tokens.json as the kit wrote it, and the commit holds
  // every other file, so kit-lock.json is the only change
  const lockOnlyRepo = (baseline) => {
    const root = copyFixture();
    editJson(root, 'dark.tokens.json', (data) => {
      data.space['1'].$value = { value: 6, unit: 'px' };
      data.space.zeta = { $value: { value: 10, unit: 'px' } };
      data.space.alpha = { $value: { value: 14, unit: 'px' } };
    });
    buildCopy(root);
    if (baseline !== null) writeFileSync(path.join(root, BASELINE), `${JSON.stringify({ violations: baseline })}\n`);
    const identity = ['-c', 'user.name=t', '-c', 'user.email=t@t.test', '-c', 'commit.gpgsign=false'];
    for (const args of [
      ['init', '-q'],
      ['add', '-A'],
      ['commit', '-q', '-m', 'adopt'],
    ]) {
      const result = spawnSync('git', [...identity, ...args], { cwd: root, encoding: 'utf8' });
      expect(result.status, result.stderr).toBe(0);
    }
    const lock = { files: { [LIGHT]: sha256(readFileSync(path.join(root, LIGHT), 'utf8')) } };
    writeFileSync(path.join(root, 'kit-lock.json'), `${JSON.stringify(lock, null, 2)}\n`);
    return root;
  };

  it('--sync replaces a stale baseline with the current violations sorted by key and prints their sum', () => {
    const root = lockOnlyRepo({ 'zzz|stale finding': 4 });
    const { status, output } = runCli(['--sync', '--root', root]);
    expect(status, output).toBe(0);
    const { violations } = JSON.parse(readFileSync(path.join(root, BASELINE), 'utf8'));
    const keys = Object.keys(violations);
    expect(keys).toHaveLength(3);
    expect(keys).toEqual([...keys].sort());
    expect(keys.map((key) => key.split('|').at(-1))).toEqual(['space.1', 'space.alpha', 'space.zeta']);
    expect(keys.every((key) => key.includes('design/grid'))).toBe(true);
    const sum = Object.values(violations).reduce((total, count) => total + count, 0);
    expect(output).toContain(`design-baseline.json: ${sum} known design violations`);
  });

  it('--sync without a baseline exits 0, says so and writes none', () => {
    const root = lockOnlyRepo(null);
    const { status, output } = runCli(['--sync', '--root', root]);
    expect(status, output).toBe(0);
    expect(output).toContain('no design-baseline.json: nothing to sync');
    expect(existsSync(path.join(root, BASELINE))).toBe(false);
  });
});
