import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { auditPlacement, checkPlacement, listProjectFiles } from './placement.mjs';

const NEVER = () => false;
const ALWAYS = () => true;
const SLICE_INDEXES = (candidate) => /^src\/(pages|entities)\/[^/]+\/index\.ts$/.test(candidate);

const runGit = (cwd, args) => {
  const result = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${args[0]} failed: ${result.stderr}`);
};

describe('listProjectFiles', () => {
  it('lists the tracked and the untracked files of src in a git work tree, and leaves out the ignored ones', () => {
    const root = mkdtempSync(path.join(tmpdir(), 'placement-git-'));
    try {
      runGit(root, ['init', '-q']);
      mkdirSync(path.join(root, 'src/shared/lib'), { recursive: true });
      writeFileSync(path.join(root, 'src/shared/lib/a.ts'), 'export {};\n');
      writeFileSync(path.join(root, '.gitignore'), '*.module.scss.d.ts\n');
      runGit(root, ['add', 'src/shared/lib/a.ts', '.gitignore']);
      runGit(root, [
        '-c',
        'user.name=test',
        '-c',
        'user.email=test@example.com',
        '-c',
        'commit.gpgsign=false',
        'commit',
        '-q',
        '-m',
        'init',
      ]);
      writeFileSync(path.join(root, 'src/shared/lib/b.ts'), 'export {};\n');
      writeFileSync(path.join(root, 'src/x.module.scss.d.ts'), 'export {};\n');

      expect(listProjectFiles(root)).toEqual(['src/shared/lib/a.ts', 'src/shared/lib/b.ts']);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('walks src outside a git work tree', () => {
    const root = mkdtempSync(path.join(tmpdir(), 'placement-walk-'));
    try {
      mkdirSync(path.join(root, 'src'), { recursive: true });
      writeFileSync(path.join(root, 'src/a.ts'), 'export {};\n');

      expect(listProjectFiles(root)).toEqual(['src/a.ts']);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe('checkPlacement', () => {
  it.each([
    'src/pages/user-list/model/use-user-search.ts',
    'src/pages/user-list/ui/UserFilters.tsx',
    'src/entities/user/api/user-avatar-dto.ts',
    'src/shared/lib/money.ts',
    'src/app/routing/routes.tsx',
    'docs/notes.md',
  ])('allows %s', (file) => {
    expect(checkPlacement(file, NEVER)).toBeNull();
  });

  it.each([
    ['src/widgets/user-card/ui/UserCard.tsx', 'widgets', 'widgets'],
    ['src/pages/user-list/lib/use-user-search.ts', 'hook-outside-model', 'model/'],
    ['src/pages/user-list/model/UserFilters.tsx', 'component-outside-ui', 'ui/'],
    ['src/entities/user/model/index.ts', 'nested-index', 'index.ts'],
    ['src/pages/user-list/helpers.ts', 'slice-root', 'slice root'],
    ['src/pages/user-list/utils/format.ts', 'segment', 'segment'],
    ['src/pages/user-list/api/user-dto.ts', 'api-outside-entity', 'entity'],
    ['src/pages/user-list/ui/userFilters.tsx', 'pascal-component', 'PascalCase'],
    ['src/shared/lib/formatMoney.ts', 'kebab-file', 'kebab-case'],
    ['src/pages/userList/ui/UserList.tsx', 'kebab-folder', 'kebab-case'],
  ])('rejects %s', (file, rule, reason) => {
    const violation = checkPlacement(file, SLICE_INDEXES);
    expect(violation?.rule).toBe(rule);
    expect(violation?.message).toContain('File is in the wrong place');
    expect(violation?.message).toContain(reason);
  });

  it.each(['src/shared/lib/index.ts', 'src/shared/api/index.ts', 'src/shared/index.ts'])('rejects %s', (file) => {
    const violation = checkPlacement(file, NEVER);
    expect(violation?.rule).toBe('shared-barrel');
    expect(violation?.message).toContain('File is in the wrong place');
    expect(violation?.message).toContain('import the file directly');
  });

  it('rejects styles without a component next to them', () => {
    const violation = checkPlacement('src/pages/user-list/ui/UserList.module.scss', NEVER);
    expect(violation?.rule).toBe('styles-near-component');
  });

  it('allows styles next to their component', () => {
    const asked = [];
    const exists = (candidate) => {
      asked.push(candidate);
      return true;
    };
    expect(checkPlacement('src/pages/user-list/ui/UserList.module.scss', exists)).toBeNull();
    expect(asked).toEqual(['src/pages/user-list/ui/UserList.tsx']);
    expect(checkPlacement('src/pages/user-list/ui/UserList.module.scss', ALWAYS)).toBeNull();
  });
});

describe('checkPlacement of a slice group in pages', () => {
  const GROUP_FILES = new Set(['src/pages/guide/help/index.ts']);
  const inGroup = (candidate) => GROUP_FILES.has(candidate);
  const groupChildren = (directory) => (directory === 'src/pages/guide' ? ['help', 'ui'] : []);

  it.each([
    'src/pages/guide/help/ui/HelpPage.tsx',
    'src/pages/guide/help/model/use-help-page.ts',
    'src/pages/guide/help/index.ts',
  ])('allows %s when src/pages/guide/index.ts does not exist', (file) => {
    expect(checkPlacement(file, inGroup)).toBeNull();
  });

  it.each([
    ['src/pages/guide/ui/GuidePage.tsx', 'segment'],
    ['src/pages/guide/index.ts', 'group-index'],
    ['src/pages/guide/help/utils/x.ts', 'segment'],
    ['src/pages/guide/help/ui/index.ts', 'nested-index'],
  ])('rejects %s', (file, rule) => {
    expect(checkPlacement(file, inGroup, groupChildren)?.rule).toBe(rule);
  });

  it.each([
    'src/pages/guide/help/index.ts',
    'src/pages/guide/help/ui/HelpPage.tsx',
    'src/pages/guide/releases/model/use-releases-page.ts',
  ])('allows %s before any file of the group exists, as the place gate asks', (file) => {
    expect(checkPlacement(file, NEVER)).toBeNull();
  });

  it('names the slice index in the message of a group index', () => {
    const violation = checkPlacement('src/pages/guide/index.ts', inGroup, groupChildren);
    expect(violation?.message).toContain('File is in the wrong place');
    expect(violation?.message).toContain('A slice group has no public API');
    expect(violation?.message).toContain('src/pages/guide/help/index.ts');
  });

  it.each([
    ['src/features/guide/help/ui/HelpPage.tsx', 'src/features/guide/help/index.ts'],
    ['src/entities/guide/help/model/help.ts', 'src/entities/guide/help/index.ts'],
  ])('still refuses a group in features or entities: %s', (file, index) => {
    expect(checkPlacement(file, (candidate) => candidate === index)?.rule).toBe('segment');
  });
});

describe('auditPlacement', () => {
  const makeProject = (files) => {
    const root = mkdtempSync(path.join(tmpdir(), 'placement-audit-'));
    for (const file of files) {
      const target = path.join(root, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, '');
    }
    return root;
  };

  it('returns the violations with file, rule and message, sorted by file', () => {
    const root = makeProject([
      'src/widgets/x/ui/X.tsx',
      'src/pages/user-list/ui/UserList.module.scss',
      'src/shared/lib/money.ts',
    ]);
    try {
      const violations = auditPlacement(root);
      expect(violations.map((violation) => [violation.file, violation.rule])).toEqual([
        ['src/pages/user-list/ui/UserList.module.scss', 'styles-near-component'],
        ['src/widgets/x/ui/X.tsx', 'widgets'],
      ]);
      expect(violations.every((violation) => violation.message.includes('File is in the wrong place'))).toBe(true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('skips dotfiles and snapshot folders instead of judging or crashing on them', () => {
    const root = makeProject([
      'src/pages/.DS_Store',
      'src/pages/user-list/.DS_Store',
      'src/pages/user-list/index.ts',
      'src/pages/user-list/lib/__snapshots__/user-list-view.test.ts.snap',
    ]);
    try {
      expect(auditPlacement(root)).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('returns no violation for a project with a slice group in pages', () => {
    const root = makeProject([
      'src/pages/guide/help/index.ts',
      'src/pages/guide/help/ui/HelpPage.tsx',
      'src/pages/guide/help/model/use-help-page.ts',
    ]);
    try {
      expect(auditPlacement(root)).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('returns an empty list for a project that holds only money.ts', () => {
    const root = makeProject(['src/shared/lib/money.ts']);
    try {
      expect(auditPlacement(root)).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe('placement CLI', () => {
  const SCRIPT = path.join(import.meta.dirname, 'placement.mjs');
  const run = (file) => spawnSync(process.execPath, [SCRIPT, file], { encoding: 'utf8' });

  it('exits 1 with the message in stderr', () => {
    const result = run('src/widgets/x/ui/X.tsx');
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('The widgets layer is not used');
  });

  it('exits 0 for a correct path', () => {
    expect(run('src/shared/lib/money.ts').status).toBe(0);
  });

  it('audits the pack with --all: exit 0 and no output', () => {
    const result = run('--all');
    expect(result.status).toBe(0);
    expect(result.stdout).toBe('');
    expect(result.stderr).toBe('');
  });

  it('checks the file when run through a linked directory', () => {
    const linkRoot = mkdtempSync(path.join(tmpdir(), 'placement-link-'));
    const link = path.join(linkRoot, 'tooling');
    symlinkSync(import.meta.dirname, link, 'junction');
    try {
      const result = spawnSync(process.execPath, [path.join(link, 'placement.mjs'), 'src/widgets/x/ui/X.tsx'], {
        encoding: 'utf8',
      });
      expect(result.status).toBe(1);
    } finally {
      unlinkSync(link);
      rmSync(linkRoot, { recursive: true, force: true });
    }
  });
});
