import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import {
  findForeignChanges,
  findKitFindings,
  hashContent,
  listChangedPaths,
  readKitPaths,
  readUntouchedKitFiles,
} from './kit-files.mjs';

const made = [];

const lockDir = (lock) => {
  const dir = mkdtempSync(path.join(tmpdir(), 'kit-files-'));
  made.push(dir);
  writeFileSync(path.join(dir, 'kit-lock.json'), JSON.stringify(lock));
  return dir;
};

const emptyDir = () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'kit-files-'));
  made.push(dir);
  return dir;
};

const LOCK = { files: { 'eslint.config.js': 'hash-a' }, seeds: { 'docs/deviations.md': 'hash-b' } };

afterEach(() => {
  for (const dir of made.splice(0)) rmSync(dir, { recursive: true, force: true });
});

describe('readKitPaths', () => {
  it('gives the files and the seeds of kit-lock.json as two sets', () => {
    expect(readKitPaths(lockDir(LOCK))).toEqual({
      files: new Set(['eslint.config.js']),
      seeds: new Set(['docs/deviations.md']),
    });
  });

  it('gives two empty sets in a folder without kit-lock.json', () => {
    expect(readKitPaths(emptyDir())).toEqual({ files: new Set(), seeds: new Set() });
  });
});

const sha256 = (text) => createHash('sha256').update(text).digest('hex');

const KIT_TEXT = 'export default {};\nconst a = 1;\n';

describe('hashContent', () => {
  it('hashes the bytes with CRLF read as LF', () => {
    expect(hashContent(Buffer.from(KIT_TEXT.replaceAll('\n', '\r\n')))).toBe(sha256(KIT_TEXT));
  });

  it('keeps a lone CR', () => {
    expect(hashContent(Buffer.from('a\rb\n'))).toBe(sha256('a\rb\n'));
  });
});

describe('readUntouchedKitFiles', () => {
  it('gives the lock path whose file still holds the kit bytes', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT) } });
    writeFileSync(path.join(dir, 'vite.config.ts'), KIT_TEXT);

    expect(readUntouchedKitFiles(dir)).toEqual(new Set(['vite.config.ts']));
  });

  it('counts a CRLF copy of the kit text as untouched', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT) } });
    writeFileSync(path.join(dir, 'vite.config.ts'), KIT_TEXT.replaceAll('\n', '\r\n'));

    expect(readUntouchedKitFiles(dir)).toEqual(new Set(['vite.config.ts']));
  });

  it('leaves out a lock path the project changed', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT), 'eslint.config.js': sha256('kit\n') } });
    writeFileSync(path.join(dir, 'vite.config.ts'), `${KIT_TEXT}const own = 2;\n`);
    writeFileSync(path.join(dir, 'eslint.config.js'), 'kit\n');

    expect(readUntouchedKitFiles(dir)).toEqual(new Set(['eslint.config.js']));
  });

  it('leaves out a lock path whose file was deleted', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT) } });

    expect(readUntouchedKitFiles(dir)).toEqual(new Set());
  });

  it('gives an empty set in a folder without kit-lock.json', () => {
    expect(readUntouchedKitFiles(emptyDir())).toEqual(new Set());
  });

  it('gives a lock path whose .kit-new exists beside a file the project changed', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT) } });
    writeFileSync(path.join(dir, 'vite.config.ts'), `${KIT_TEXT}const own = 2;\n`);
    writeFileSync(path.join(dir, 'vite.config.ts.kit-new'), KIT_TEXT);

    expect(readUntouchedKitFiles(dir)).toEqual(new Set(['vite.config.ts']));
  });

  it('gives a lock path whose .kit-new exists and whose file was deleted', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT) } });
    writeFileSync(path.join(dir, 'vite.config.ts.kit-new'), KIT_TEXT);

    expect(readUntouchedKitFiles(dir)).toEqual(new Set(['vite.config.ts']));
  });

  it('leaves out a changed lock path without a .kit-new, beside one that has it', () => {
    const dir = lockDir({ files: { 'vite.config.ts': sha256(KIT_TEXT), 'eslint.config.js': sha256('kit\n') } });
    writeFileSync(path.join(dir, 'vite.config.ts'), `${KIT_TEXT}const own = 2;\n`);
    writeFileSync(path.join(dir, 'vite.config.ts.kit-new'), KIT_TEXT);
    writeFileSync(path.join(dir, 'eslint.config.js'), 'own\n');

    expect(readUntouchedKitFiles(dir)).toEqual(new Set(['vite.config.ts']));
  });
});

describe('findKitFindings', () => {
  it('gives the findings whose file is a kit file, and not the project own files', () => {
    const kitFinding = { file: 'eslint.config.js', message: 'Unexpected token.' };
    const ownFinding = { file: 'src/a.ts', message: 'Unexpected token.' };

    expect(findKitFindings([kitFinding, ownFinding], new Set(['eslint.config.js']))).toEqual([kitFinding]);
  });
});

const git = (root, ...args) => {
  const identity = ['-c', 'user.name=t', '-c', 'user.email=t@t.test', '-c', 'commit.gpgsign=false'];
  const result = spawnSync('git', [...identity, ...args], { cwd: root, encoding: 'utf8' });
  expect(result.status, result.stderr).toBe(0);
};

const writeProjectFile = (root, file, text) => {
  mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
  writeFileSync(path.join(root, file), text);
};

const lockText = (files) => JSON.stringify({ files });

// a repository whose HEAD commits kit-lock.json with two kit files, both present
const committedKitRepo = () => {
  const root = emptyDir();
  git(root, 'init', '-q');
  writeProjectFile(root, 'kit-lock.json', lockText({ 'src/app/old.ts': 'H1', 'src/app/kept.ts': 'H2' }));
  writeProjectFile(root, 'src/app/old.ts', 'old\n');
  writeProjectFile(root, 'src/app/kept.ts', 'kept\n');
  git(root, 'add', '-A');
  git(root, 'commit', '-q', '-m', 'adopt');
  return root;
};

describe('a kit file an update removed', () => {
  it('is no foreign change when the lock of HEAD held it', () => {
    const root = committedKitRepo();
    rmSync(path.join(root, 'src/app/old.ts'));
    writeProjectFile(root, 'kit-lock.json', lockText({ 'src/app/kept.ts': 'H2' }));

    expect(findForeignChanges(listChangedPaths(root), readKitPaths(root))).toEqual([]);
  });

  it('leaves a change to a file in neither lock foreign', () => {
    const root = committedKitRepo();
    rmSync(path.join(root, 'src/app/old.ts'));
    writeProjectFile(root, 'kit-lock.json', lockText({ 'src/app/kept.ts': 'H2' }));
    writeProjectFile(root, 'src/app/own.ts', 'own\n');

    expect(findForeignChanges(listChangedPaths(root), readKitPaths(root))).toEqual(['src/app/own.ts']);
  });

  it('gives the files of kit-lock.json alone in a folder that is no git repository', () => {
    const dir = lockDir({ files: { 'src/app/kept.ts': 'H2' } });

    expect(readKitPaths(dir).files).toEqual(new Set(['src/app/kept.ts']));
  });
});

describe('a kit-lock.json that is not JSON', () => {
  it('makes readKitPaths and readUntouchedKitFiles throw the same Error', () => {
    const dir = emptyDir();
    writeFileSync(path.join(dir, 'kit-lock.json'), '{');
    const messageOf = (read) => {
      try {
        read(dir);
      } catch (error) {
        return error.message;
      }
      return null;
    };
    const fromPaths = messageOf(readKitPaths);

    expect(fromPaths).toMatch(/^kit-lock\.json cannot be read: /);
    expect(messageOf(readUntouchedKitFiles)).toBe(fromPaths);
  });
});

describe('findForeignChanges', () => {
  it('gives the changed paths that are neither a kit file, a seed, nor a file the kit allows to change', () => {
    const changed = [
      'eslint.config.js',
      'pnpm-lock.yaml',
      'src/a.ts',
      'prettier.config.mjs.kit-new',
      'tsc-baseline.json',
    ];

    expect(findForeignChanges(changed, readKitPaths(lockDir(LOCK)))).toEqual(['src/a.ts']);
  });
});
