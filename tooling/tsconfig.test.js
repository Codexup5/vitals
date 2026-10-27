import { createRequire } from 'node:module';
import path from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
// TypeScript names files with forward slashes on every platform.
const toSlash = (fileName) => fileName.split(path.sep).join('/');
const PROBE_FILE = toSlash(path.join(PACK_ROOT, 'src', 'tsconfig-probe.ts'));

const ts = createRequire(path.join(PACK_ROOT, 'package.json'))('typescript');

const INDEXED_ACCESS = 2532; // noUncheckedIndexedAccess
const EXACT_OPTIONAL = 2375; // exactOptionalPropertyTypes
const IMPLICIT_ANY = 7006; // strict (noImplicitAny)

// [case, code, diagnostic code] — the diagnostic must appear (CATCHES) or must not (PASSES).
// Each probe exports what it declares: noUnusedLocals is on.
const CATCHES = [
  [
    'list[0].length on a string[]',
    "export const list: string[] = ['a'];\nexport const size = list[0].length;",
    INDEXED_ACCESS,
  ],
  [
    'an optional name set to undefined',
    'export interface Named {\n  name?: string;\n}\nexport const named: Named = { name: undefined };',
    EXACT_OPTIONAL,
  ],
  ['an arrow (x) => x', 'export const identity = (x) => x;', IMPLICIT_ANY],
];

const PASSES = [
  ['list[0]?.length', "export const list: string[] = ['a'];\nexport const size = list[0]?.length;", INDEXED_ACCESS],
  [
    'an object without the optional name',
    'export interface Named {\n  name?: string;\n}\nexport const named: Named = {};',
    EXACT_OPTIONAL,
  ],
  ['an arrow (x: string) => x', 'export const identity = (x: string) => x;', IMPLICIT_ANY],
];

let options;

beforeAll(() => {
  const configPath = path.join(PACK_ROOT, 'tsconfig.app.json');
  const { config, error } = ts.readConfigFile(configPath, ts.sys.readFile);
  if (error) throw new Error(ts.flattenDiagnosticMessageText(error.messageText, '\n'));
  options = ts.parseJsonConfigFileContent(config, ts.sys, PACK_ROOT).options;
});

// The codes of the diagnostics of one in-memory file: the diagnostics of any other file do not count.
function probeCodes(code) {
  const host = ts.createCompilerHost(options);
  const { fileExists, getSourceFile, readFile } = host;
  host.fileExists = (fileName) => fileName === PROBE_FILE || fileExists.call(host, fileName);
  host.readFile = (fileName) => (fileName === PROBE_FILE ? code : readFile.call(host, fileName));
  host.getSourceFile = (fileName, languageVersionOrOptions, onError, shouldCreate) =>
    fileName === PROBE_FILE
      ? ts.createSourceFile(fileName, code, languageVersionOrOptions)
      : getSourceFile.call(host, fileName, languageVersionOrOptions, onError, shouldCreate);

  const program = ts.createProgram({ rootNames: [PROBE_FILE], options, host });
  const sourceFile = program.getSourceFile(PROBE_FILE);
  if (!sourceFile) throw new Error('The probe file is not in the program');
  return [...program.getSyntacticDiagnostics(sourceFile), ...program.getSemanticDiagnostics(sourceFile)].map(
    (diagnostic) => diagnostic.code,
  );
}

describe('tsconfig.app.json strict options', () => {
  it.each(CATCHES)('catches %s', (_name, code, diagnosticCode) => {
    expect(probeCodes(code)).toContain(diagnosticCode);
  });

  it.each(PASSES)('does not catch %s', (_name, code, diagnosticCode) => {
    expect(probeCodes(code)).not.toContain(diagnosticCode);
  });
});

// The pack has no domain/ folder: the root domain files are reported as existing, the rest is read from disk.
function resolveFromSrc(moduleName, existingFiles) {
  const existing = new Set(existingFiles.map(toSlash));
  const existingDirs = new Set(existingFiles.map((fileName) => toSlash(path.dirname(fileName))));
  const host = {
    ...ts.sys,
    fileExists: (fileName) => existing.has(toSlash(fileName)) || ts.sys.fileExists(fileName),
    directoryExists: (dirName) => existingDirs.has(toSlash(dirName)) || ts.sys.directoryExists(dirName),
  };
  return ts.resolveModuleName(moduleName, PROBE_FILE, options, host).resolvedModule?.resolvedFileName;
}

describe('tsconfig.app.json source aliases', () => {
  const DOMAIN_MONEY = toSlash(path.join(PACK_ROOT, 'domain', 'money.ts'));
  const SRC_DOMAIN_MONEY = toSlash(path.join(PACK_ROOT, 'src', 'domain', 'money.ts'));
  const SRC_DATE = toSlash(path.join(PACK_ROOT, 'src', 'shared', 'lib', 'date.ts'));

  it('maps @/domain/* to the root domain/, not to src/domain/', () => {
    expect(resolveFromSrc('@/domain/money', [DOMAIN_MONEY, SRC_DOMAIN_MONEY])).toBe(DOMAIN_MONEY);
  });

  it('maps @/* to src/', () => {
    expect(resolveFromSrc('@/shared/lib/date', [DOMAIN_MONEY, SRC_DOMAIN_MONEY])).toBe(SRC_DATE);
  });
});
