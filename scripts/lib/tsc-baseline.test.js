import { describe, expect, it } from 'vitest';

import {
  findNewErrors,
  groupTscOutput,
  hasGlobalError,
  isCrash,
  listProjects,
  parseTscOutput,
  pruneBaseline,
  toFingerprints,
  toProjectPath,
} from './tsc-baseline.mjs';

const error = (file, code, message) => ({ file, code, message });

const TYPE_MISMATCH = "Type 'string' is not assignable to type 'number'.";
const MISSING_NAME = "Cannot find name 'foo'.";

describe('parseTscOutput', () => {
  it('gives file, code and message of an error line', () => {
    const text = `src/app/main.ts(12,5): error TS2322: ${TYPE_MISMATCH}\n`;

    expect(parseTscOutput(text)).toEqual([error('src/app/main.ts', 'TS2322', TYPE_MISMATCH)]);
  });

  it('keeps the first message line and drops the indented continuation', () => {
    const text = [
      `src/a.ts(1,1): error TS2322: ${TYPE_MISMATCH}`,
      "  Type 'string' is not assignable to type 'number'.",
      '    Deeper detail line.',
      `src/b.ts(2,2): error TS2304: ${MISSING_NAME}`,
    ].join('\n');

    expect(parseTscOutput(text)).toEqual([
      error('src/a.ts', 'TS2322', TYPE_MISMATCH),
      error('src/b.ts', 'TS2304', MISSING_NAME),
    ]);
  });

  it('turns backslashes in a path into posix slashes', () => {
    const text = String.raw`src\shared\lib\date.ts(3,9): error TS2304: ${MISSING_NAME}`;

    expect(parseTscOutput(text)).toEqual([error('src/shared/lib/date.ts', 'TS2304', MISSING_NAME)]);
  });

  it('ignores the summary line and blank lines', () => {
    const text = [
      '',
      `src/a.ts(1,1): error TS2322: ${TYPE_MISMATCH}`,
      '',
      `src/b.ts(2,2): error TS2304: ${MISSING_NAME}`,
      '',
      'Found 2 errors.',
      '',
    ].join('\n');

    expect(parseTscOutput(text).map((item) => item.file)).toEqual(['src/a.ts', 'src/b.ts']);
  });

  it('reads CRLF output', () => {
    const text = `src/a.ts(1,1): error TS2322: ${TYPE_MISMATCH}\r\nFound 1 error.\r\n`;

    expect(parseTscOutput(text)).toEqual([error('src/a.ts', 'TS2322', TYPE_MISMATCH)]);
  });

  it('gives an empty list for an empty output', () => {
    expect(parseTscOutput('')).toEqual([]);
  });
});

describe('toFingerprints', () => {
  it('counts PATH|CODE|MESSAGE with multiplicity', () => {
    const errors = [
      error('src/a.ts', 'TS2322', TYPE_MISMATCH),
      error('src/a.ts', 'TS2322', TYPE_MISMATCH),
      error('src/b.ts', 'TS2304', MISSING_NAME),
    ];

    expect(toFingerprints(errors)).toEqual({
      [`src/a.ts|TS2322|${TYPE_MISMATCH}`]: 2,
      [`src/b.ts|TS2304|${MISSING_NAME}`]: 1,
    });
  });

  it('gives an empty map for no errors', () => {
    expect(toFingerprints([])).toEqual({});
  });
});

describe('findNewErrors', () => {
  const known = error('src/a.ts', 'TS2322', TYPE_MISMATCH);
  const baseline = { [`src/a.ts|TS2322|${TYPE_MISMATCH}`]: 1 };

  it('gives nothing when every error is within its baseline count', () => {
    expect(findNewErrors([known], baseline)).toEqual([]);
  });

  it('gives the errors past the count: the same error twice against a count of 1 gives one', () => {
    expect(findNewErrors([known, known], baseline)).toEqual([known]);
  });

  it('matches by path, code and message, not by line', () => {
    const parsed = parseTscOutput(`src/a.ts(99,1): error TS2322: ${TYPE_MISMATCH}`);

    expect(findNewErrors(parsed, baseline)).toEqual([]);
  });

  it('gives an error with another message', () => {
    const other = error('src/a.ts', 'TS2322', "Type 'null' is not assignable to type 'number'.");

    expect(findNewErrors([other], baseline)).toEqual([other]);
  });

  it('gives an error in another file or with another code', () => {
    const otherFile = error('src/b.ts', 'TS2322', TYPE_MISMATCH);
    const otherCode = error('src/a.ts', 'TS2345', TYPE_MISMATCH);

    expect(findNewErrors([otherFile, otherCode], baseline)).toEqual([otherFile, otherCode]);
  });

  it('gives every error when the baseline is empty', () => {
    expect(findNewErrors([known], {})).toEqual([known]);
  });

  it('does not change the baseline it is given', () => {
    findNewErrors([known, known], baseline);

    expect(baseline).toEqual({ [`src/a.ts|TS2322|${TYPE_MISMATCH}`]: 1 });
  });
});

describe('pruneBaseline', () => {
  const keyA = `src/a.ts|TS2322|${TYPE_MISMATCH}`;
  const keyB = `src/b.ts|TS2304|${MISSING_NAME}`;

  it('lowers the counts to the current ones', () => {
    const errors = [error('src/a.ts', 'TS2322', TYPE_MISMATCH)];

    expect(pruneBaseline(errors, { [keyA]: 3 })).toEqual({ [keyA]: 1 });
  });

  it('drops an entry whose error is gone', () => {
    const errors = [error('src/a.ts', 'TS2322', TYPE_MISMATCH)];

    expect(pruneBaseline(errors, { [keyA]: 1, [keyB]: 2 })).toEqual({ [keyA]: 1 });
  });

  it('never raises a count', () => {
    const errors = [error('src/a.ts', 'TS2322', TYPE_MISMATCH), error('src/a.ts', 'TS2322', TYPE_MISMATCH)];

    expect(pruneBaseline(errors, { [keyA]: 1 })).toEqual({ [keyA]: 1 });
  });

  it('never adds an error that is not in the baseline', () => {
    const errors = [error('src/a.ts', 'TS2322', TYPE_MISMATCH), error('src/b.ts', 'TS2304', MISSING_NAME)];

    expect(pruneBaseline(errors, { [keyA]: 1 })).toEqual({ [keyA]: 1 });
  });

  it('gives an empty map when no error is left', () => {
    expect(pruneBaseline([], { [keyA]: 2 })).toEqual({});
  });
});

describe('groupTscOutput', () => {
  const lineA = `src/a.ts(1,1): error TS2322: ${TYPE_MISMATCH}`;
  const lineB = `src/b.ts(2,2): error TS2304: ${MISSING_NAME}`;

  it('gives each error with its line and its indented continuation lines', () => {
    const text = [lineA, "  Type 'string' is not assignable to type 'number'.", '    Deeper detail line.', lineB].join(
      '\n',
    );

    expect(groupTscOutput(text)).toEqual({
      groups: [
        {
          error: error('src/a.ts', 'TS2322', TYPE_MISMATCH),
          lines: [lineA, "  Type 'string' is not assignable to type 'number'.", '    Deeper detail line.'],
        },
        { error: error('src/b.ts', 'TS2304', MISSING_NAME), lines: [lineB] },
      ],
      unrecognized: [],
    });
  });

  it('does not count blank lines, the summary and a global error as unrecognized', () => {
    const text = [
      '',
      lineA,
      '',
      'error TS2318: Cannot find global type Array.',
      'Found 2 errors in 2 files.',
      'Found 1 error.',
    ].join('\n');

    expect(groupTscOutput(text).unrecognized).toEqual([]);
  });

  it('gives a Debug Failure line and an unindented stack line as unrecognized', () => {
    const text = [lineA, 'Debug Failure.', 'at Object.x (file.js:1:2)'].join('\n');

    expect(groupTscOutput(text).unrecognized).toEqual(['Debug Failure.', 'at Object.x (file.js:1:2)']);
  });

  it('reads CRLF output and gives nothing for an empty one', () => {
    const text = [lineA, 'Found 1 error.', ''].join('\r\n');

    expect(groupTscOutput(text).groups).toHaveLength(1);
    expect(groupTscOutput(text).unrecognized).toEqual([]);
    expect(groupTscOutput('')).toEqual({ groups: [], unrecognized: [] });
  });
});

describe('hasGlobalError', () => {
  it('is true on a line starting with error TSNNNN', () => {
    const text = "src/a.ts(1,1): error TS2322: x\nerror TS2318: Cannot find global type 'Array'.\n";

    expect(hasGlobalError(text)).toBe(true);
  });

  it('is false on a path error', () => {
    expect(hasGlobalError(`src/a.ts(1,1): error TS2322: ${TYPE_MISMATCH}`)).toBe(false);
  });

  it('is true on a config error that names a tsconfig file with a path (TS6310)', () => {
    expect(hasGlobalError('tsconfig.json(38,18): error TS6310: Referenced project may not disable emit.')).toBe(true);
  });

  it('is true on an unknown option named in a tsconfig file with a path (TS5023)', () => {
    expect(hasGlobalError("tsconfig.app.json(2,3): error TS5023: Unknown compiler option 'x'.")).toBe(true);
  });

  it('is false on a code error in a source file (TS6133)', () => {
    expect(hasGlobalError("src/a.ts(1,7): error TS6133: 'x' is declared but its value is never read.")).toBe(false);
  });

  it('is true on a global error that cannot read a config file (TS5083)', () => {
    expect(hasGlobalError("error TS5083: Cannot read file 'tsconfig.base.json'.")).toBe(true);
  });
});

describe('toProjectPath', () => {
  it('makes an absolute posix path relative to the root', () => {
    expect(toProjectPath('C:/work/app/src/a.ts', 'C:/work/app')).toBe('src/a.ts');
  });

  it('makes an absolute path with backslashes relative to the root, with slashes', () => {
    expect(toProjectPath(String.raw`C:\work\app\src\a.ts`, 'C:/work/app')).toBe('src/a.ts');
  });

  it('keeps a relative path, with slashes', () => {
    expect(toProjectPath('src/a.ts', 'C:/work/app')).toBe('src/a.ts');
  });
});

describe('isCrash', () => {
  it('is true for a non-zero status with an unrecognized line', () => {
    expect(isCrash(1, ['Debug Failure.'], null)).toBe(true);
  });

  it('is false for status 0', () => {
    expect(isCrash(0, ['Debug Failure.'], null)).toBe(false);
  });

  it('is false when no line is unrecognized', () => {
    expect(isCrash(2, [], null)).toBe(false);
  });

  it('is true when a signal is set, even with a null status and no unrecognized line', () => {
    expect(isCrash(null, [], 'SIGKILL')).toBe(true);
  });

  it('is false when the signal is null and the other rules say no crash', () => {
    expect(isCrash(2, [], null)).toBe(false);
    expect(isCrash(0, [], null)).toBe(false);
  });
});

describe('listProjects', () => {
  const folder = (name, hasTsconfig) => ({ name, hasTsconfig });
  const ROOT = ['tsconfig.json'];

  it('gives the root tsconfig first, then NAME/tsconfig.json of each folder that has one, in name order', () => {
    const folders = [folder('server', true), folder('src', false), folder('api', true)];

    expect(listProjects({ rootConfigs: ROOT, referenced: [], extended: [], folders })).toEqual([
      'tsconfig.json',
      'api/tsconfig.json',
      'server/tsconfig.json',
    ]);
  });

  it('skips node_modules and folders that start with a dot', () => {
    const folders = [
      folder('node_modules', true),
      folder('.sandbox', true),
      folder('.claude', true),
      folder('server', true),
    ];

    expect(listProjects({ rootConfigs: ROOT, referenced: [], extended: [], folders })).toEqual([
      'tsconfig.json',
      'server/tsconfig.json',
    ]);
  });

  it('gives only the nested ones when there is no root tsconfig', () => {
    const folders = [folder('server', true), folder('api', true)];

    expect(listProjects({ rootConfigs: [], referenced: [], extended: [], folders })).toEqual([
      'api/tsconfig.json',
      'server/tsconfig.json',
    ]);
  });

  it('gives the root tsconfig alone when no folder has one', () => {
    expect(
      listProjects({
        rootConfigs: ROOT,
        referenced: [],
        extended: [],
        folders: [folder('src', false), folder('docs', false)],
      }),
    ).toEqual(['tsconfig.json']);
    expect(listProjects({ rootConfigs: ROOT, referenced: [], extended: [], folders: [] })).toEqual(['tsconfig.json']);
  });

  it('gives nothing when there is no tsconfig at all', () => {
    expect(listProjects({ rootConfigs: [], referenced: [], extended: [], folders: [folder('src', false)] })).toEqual(
      [],
    );
  });

  describe('with the root configs of a solution', () => {
    const ROOT_CONFIGS = [
      'tsconfig.json',
      'tsconfig.app.json',
      'tsconfig.node.json',
      'tsconfig.server.json',
      'tsconfig.e2e.json',
    ];
    const FOLDERS = [
      folder('server', true),
      folder('node_modules', true),
      folder('.cache', true),
      folder('src', false),
    ];

    it('gives the root configs neither referenced nor extended, the root first, then the folders', () => {
      expect(
        listProjects({
          rootConfigs: ROOT_CONFIGS,
          referenced: ['tsconfig.app.json', 'tsconfig.node.json'],
          extended: [],
          folders: FOLDERS,
        }),
      ).toEqual(['tsconfig.json', 'tsconfig.e2e.json', 'tsconfig.server.json', 'server/tsconfig.json']);
    });

    it('leaves out a folder whose tsconfig is referenced', () => {
      expect(
        listProjects({
          rootConfigs: ROOT_CONFIGS,
          referenced: ['tsconfig.app.json', 'tsconfig.node.json', 'server/tsconfig.json'],
          extended: [],
          folders: FOLDERS,
        }),
      ).toEqual(['tsconfig.json', 'tsconfig.e2e.json', 'tsconfig.server.json']);
    });

    it('without a root tsconfig.json, gives the other root configs and then the folders', () => {
      expect(
        listProjects({
          rootConfigs: ROOT_CONFIGS.slice(1),
          referenced: ['tsconfig.app.json', 'tsconfig.node.json'],
          extended: [],
          folders: FOLDERS,
        }),
      ).toEqual(['tsconfig.e2e.json', 'tsconfig.server.json', 'server/tsconfig.json']);
    });

    it('leaves out a base config that is extended, as the list above does', () => {
      expect(
        listProjects({
          rootConfigs: [...ROOT_CONFIGS, 'tsconfig.base.json'],
          referenced: ['tsconfig.app.json', 'tsconfig.node.json'],
          extended: ['tsconfig.base.json'],
          folders: FOLDERS,
        }),
      ).toEqual(['tsconfig.json', 'tsconfig.e2e.json', 'tsconfig.server.json', 'server/tsconfig.json']);
    });
  });
});
