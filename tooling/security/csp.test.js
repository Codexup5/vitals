import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  cspHeader,
  cspPlugin,
  findCspDefects,
  findOriginDefects,
  fromCspHeader,
  listCspHeaders,
  readCspConfig,
  toPolicy,
} from './csp.mjs';

const PACK_ROOT = path.resolve(import.meta.dirname, '../..');
const CLI = path.join(PACK_ROOT, 'tooling/security/check-csp.mjs');
const INDEX = readFileSync(path.join(PACK_ROOT, 'index.html'), 'utf8');

const REF_HEADER =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";
const REF_META =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'";
const TG_HEADER =
  "default-src 'self'; script-src 'self' https://telegram.org; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'self' https://web.telegram.org";

const REF = {
  mode: 'enforce',
  directives: {
    'default-src': ["'self'"],
    'script-src': ["'self'"],
    'style-src': ["'self'"],
    'img-src': ["'self'", 'data:'],
    'font-src': ["'self'"],
    'connect-src': ["'self'"],
    'object-src': ["'none'"],
    'base-uri': ["'self'"],
    'form-action': ["'self'"],
    'frame-ancestors': ["'none'"],
  },
};

const TG = {
  mode: 'report-only',
  directives: {
    'default-src': ["'self'"],
    'script-src': ["'self'", 'https://telegram.org'],
    'style-src': ["'self'", "'unsafe-inline'"],
    'img-src': ["'self'", 'data:', 'https:'],
    'font-src': ["'self'"],
    'connect-src': ["'self'"],
    'object-src': ["'none'"],
    'base-uri': ["'self'"],
    'form-action': ["'self'"],
    'frame-ancestors': ["'self'", 'https://web.telegram.org'],
  },
};

const clone = (config) => structuredClone(config);

// A copy of a config with one directive replaced, or removed when sources is undefined.
const withDirective = (config, name, sources) => {
  const copy = clone(config);
  if (sources === undefined) delete copy.directives[name];
  else copy.directives[name] = sources;
  return copy;
};

const vercelWith = (headers) => ({ headers: [{ source: '/(.*)', headers }] });

const temporaryRoots = [];
const makeRoot = (files) => {
  const root = mkdtempSync(path.join(tmpdir(), 'csp-probe-'));
  temporaryRoots.push(root);
  for (const [name, content] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    writeFileSync(path.join(root, name), content);
  }
  return root;
};

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) rmSync(root, { recursive: true, force: true });
});

// The one finding of the probe: its id, its file, and its detail with the quoted text.
const expectOne = (findings, id, file, text) => {
  expect(findings).toHaveLength(1);
  expect(findings[0]).toMatchObject({ id, file });
  expect(findings[0].detail).toContain(text);
};

describe('toPolicy and cspHeader', () => {
  it('P1 writes the header policy of the reference', () => {
    expect(toPolicy(REF, 'header')).toBe(REF_HEADER);
  });

  it('P2 writes the meta policy of the reference, without frame-ancestors', () => {
    expect(toPolicy(REF, 'meta')).toBe(REF_META);
  });

  it('P3 appends report-uri to the header only', () => {
    const config = { ...clone(REF), reportUri: 'https://csp.example.com/r' };
    expect(toPolicy(config, 'header')).toBe(`${REF_HEADER}; report-uri https://csp.example.com/r`);
    expect(toPolicy(config, 'meta')).toBe(REF_META);
  });

  it('P4 names the header key by the mode', () => {
    expect(cspHeader(REF)).toEqual({ key: 'Content-Security-Policy', value: REF_HEADER });
    expect(cspHeader(TG)).toEqual({ key: 'Content-Security-Policy-Report-Only', value: TG_HEADER });
  });
});

describe('findCspDefects', () => {
  it('P5 passes the reference against the index.html of the pack', () => {
    expect(findCspDefects({ config: REF, indexHtml: INDEX, vercel: null })).toEqual([]);
  });

  it("P6 finds 'unsafe-inline' in script-src", () => {
    const config = withDirective(REF, 'script-src', ["'self'", "'unsafe-inline'"]);
    expectOne(
      findCspDefects({ config, indexHtml: INDEX, vercel: null }),
      'csp/script-src',
      'csp.json',
      "'unsafe-inline'",
    );
  });

  it('P7 finds https: in script-src', () => {
    const config = withDirective(REF, 'script-src', ["'self'", 'https:']);
    expectOne(findCspDefects({ config, indexHtml: INDEX, vercel: null }), 'csp/script-src', 'csp.json', 'https:');
  });

  it('P8 finds * in default-src', () => {
    const config = withDirective(REF, 'default-src', ['*']);
    expectOne(findCspDefects({ config, indexHtml: INDEX, vercel: null }), 'csp/script-src', 'csp.json', 'default-src');
  });

  it("P9 requires object-src, and then requires it to be 'none'", () => {
    const missing = withDirective(REF, 'object-src', undefined);
    expectOne(
      findCspDefects({ config: missing, indexHtml: INDEX, vercel: null }),
      'csp/required',
      'csp.json',
      'object-src',
    );

    const wrong = withDirective(REF, 'object-src', ["'self'"]);
    expectOne(
      findCspDefects({ config: wrong, indexHtml: INDEX, vercel: null }),
      'csp/object-src',
      'csp.json',
      'object-src',
    );
  });

  it('P10 requires frame-ancestors', () => {
    const config = withDirective(REF, 'frame-ancestors', undefined);
    expectOne(
      findCspDefects({ config, indexHtml: INDEX, vercel: null }),
      'csp/required',
      'csp.json',
      'frame-ancestors',
    );
  });

  it('P11 accepts base-uri only as self or none', () => {
    const config = withDirective(REF, 'base-uri', ['https://cdn.example.com']);
    expectOne(findCspDefects({ config, indexHtml: INDEX, vercel: null }), 'csp/base-uri', 'csp.json', 'base-uri');
  });

  it('P12 rejects a malformed csp.json, naming the field', () => {
    const badMode = { ...clone(REF), mode: 'warn' };
    expectOne(findCspDefects({ config: badMode, indexHtml: INDEX, vercel: null }), 'csp/config', 'csp.json', 'mode');

    const extra = { ...clone(REF), extra: true };
    expectOne(findCspDefects({ config: extra, indexHtml: INDEX, vercel: null }), 'csp/config', 'csp.json', 'extra');

    const stringList = withDirective(REF, 'script-src', "'self'");
    expectOne(
      findCspDefects({ config: stringList, indexHtml: INDEX, vercel: null }),
      'csp/config',
      'csp.json',
      'script-src',
    );

    expectOne(findCspDefects({ config: null, indexHtml: INDEX, vercel: null }), 'csp/config', 'csp.json', 'csp.json');
  });

  it('P13 finds a hand-written meta in index.html', () => {
    const indexHtml = INDEX.replace(
      '<head>',
      `<head>\n    <meta http-equiv="Content-Security-Policy" content="default-src 'self'">`,
    );
    expectOne(
      findCspDefects({ config: REF, indexHtml, vercel: null }),
      'csp/hand-meta',
      'index.html',
      'Content-Security-Policy',
    );
  });

  it('P14 finds the report-only key in an enforce hosting header', () => {
    const vercel = vercelWith([{ key: 'Content-Security-Policy-Report-Only', value: REF_HEADER }]);
    expectOne(
      findCspDefects({ config: REF, indexHtml: INDEX, vercel }),
      'csp/hosting',
      'vercel.json',
      'Content-Security-Policy',
    );
  });

  it('P15 finds the meta value in the header, then passes the header beside another header', () => {
    const wrongValue = vercelWith([{ key: 'Content-Security-Policy', value: REF_META }]);
    expectOne(
      findCspDefects({ config: REF, indexHtml: INDEX, vercel: wrongValue }),
      'csp/hosting',
      'vercel.json',
      REF_HEADER,
    );

    const right = vercelWith([
      { key: 'Content-Security-Policy', value: REF_HEADER },
      { key: 'X-Content-Type-Options', value: 'nosniff' },
    ]);
    expect(findCspDefects({ config: REF, indexHtml: INDEX, vercel: right })).toEqual([]);
  });

  it("P16 passes 'unsafe-inline' in style-src, and counts the CSP headers of the source /(.*)", () => {
    const tgVercel = vercelWith([{ key: 'Content-Security-Policy-Report-Only', value: TG_HEADER }]);
    expect(findCspDefects({ config: TG, indexHtml: INDEX, vercel: tgVercel })).toEqual([]);

    const noCsp = vercelWith([{ key: 'X-Frame-Options', value: 'DENY' }]);
    expect(findCspDefects({ config: REF, indexHtml: INDEX, vercel: noCsp })).toHaveLength(1);
    expect(findCspDefects({ config: REF, indexHtml: INDEX, vercel: noCsp })[0]).toMatchObject({
      id: 'csp/hosting',
      file: 'vercel.json',
    });

    const bothKeys = vercelWith([
      { key: 'Content-Security-Policy', value: REF_HEADER },
      { key: 'Content-Security-Policy-Report-Only', value: REF_HEADER },
    ]);
    expect(findCspDefects({ config: REF, indexHtml: INDEX, vercel: bothKeys })).toHaveLength(1);
    expect(findCspDefects({ config: REF, indexHtml: INDEX, vercel: bothKeys })[0]).toMatchObject({
      id: 'csp/hosting',
      file: 'vercel.json',
    });
  });

  it('P17 passes a hosting header that says the same policy in another spelling, as adopt reads it', () => {
    const spellings = [
      { key: 'Content-Security-Policy-Report-Only', value: `${TG_HEADER};` },
      { key: 'content-security-policy-report-only', value: TG_HEADER },
      { key: 'Content-Security-Policy-Report-Only', value: TG_HEADER.replaceAll('; ', ';') },
    ];
    for (const header of spellings) {
      expect(findCspDefects({ config: TG, indexHtml: INDEX, vercel: vercelWith([header]) })).toEqual([]);
    }

    const otherPolicy = vercelWith([
      { key: 'Content-Security-Policy-Report-Only', value: `${TG_HEADER}; font-src https:` },
    ]);
    expect(findCspDefects({ config: TG, indexHtml: INDEX, vercel: otherPolicy })[0]).toMatchObject({
      id: 'csp/hosting',
    });
  });
});

describe('findOriginDefects', () => {
  it('P17 checks the origins of the environment against connect-src', () => {
    expectOne(
      findOriginDefects(REF, { VITE_API_URL: 'https://api.example.com' }),
      'csp/connect-src',
      'csp.json',
      'VITE_API_URL',
    );
    expectOne(
      findOriginDefects(REF, { VITE_API_URL: 'https://api.example.com' }),
      'csp/connect-src',
      'csp.json',
      'https://api.example.com',
    );

    expect(findOriginDefects(REF, { VITE_API_URL: '/api' })).toEqual([]);

    const sentryConfig = withDirective(REF, 'connect-src', ["'self'", 'https://*.ingest.sentry.io']);
    const sentryDsn = { VITE_SENTRY_DSN: 'https://key@o0.ingest.sentry.io/0' };
    expect(findOriginDefects(sentryConfig, sentryDsn)).toEqual([]);

    expectOne(findOriginDefects(REF, sentryDsn), 'csp/connect-src', 'csp.json', 'https://o0.ingest.sentry.io');

    const anyHttps = withDirective(REF, 'connect-src', ["'self'", 'https:']);
    expect(findOriginDefects(anyHttps, { VITE_SUPABASE_URL: 'https://abc.supabase.co' })).toEqual([]);
  });
});

describe('cspPlugin', () => {
  const pluginFor = (root, env = {}, command = 'build') => {
    const plugin = cspPlugin();
    plugin.configResolved({ root, command, env });
    return plugin;
  };

  it('P18 writes the meta into the build, sends the header in the preview, and checks the origins at the build start', () => {
    const refRoot = makeRoot({ 'csp.json': JSON.stringify(REF) });
    const tgRoot = makeRoot({ 'csp.json': JSON.stringify(TG) });

    const build = pluginFor(refRoot);
    expect(build.transformIndexHtml(INDEX)).toEqual([
      {
        tag: 'meta',
        attrs: { 'http-equiv': 'Content-Security-Policy', content: REF_META },
        injectTo: 'head-prepend',
      },
    ]);
    expect(build.transformIndexHtml(INDEX, { server: {} })).toEqual([]);

    expect(pluginFor(tgRoot).transformIndexHtml(INDEX)).toEqual([]);

    const headers = {};
    const use = vi.fn();
    build.configurePreviewServer({ middlewares: { use } });
    const middleware = use.mock.calls[0][0];
    const next = vi.fn();
    middleware({}, { setHeader: (key, value) => (headers[key] = value) }, next);
    expect(headers['Content-Security-Policy']).toBe(REF_HEADER);
    expect(next).toHaveBeenCalled();

    const originEnv = { VITE_API_URL: 'https://api.example.com' };
    const refContext = { error: vi.fn(), warn: vi.fn() };
    pluginFor(refRoot, originEnv).buildStart.call(refContext);
    expect(refContext.error).toHaveBeenCalledWith(expect.stringContaining('https://api.example.com'));

    const tgContext = { error: vi.fn(), warn: vi.fn() };
    pluginFor(tgRoot, originEnv).buildStart.call(tgContext);
    expect(tgContext.warn).toHaveBeenCalledWith(expect.stringContaining('https://api.example.com'));
    expect(tgContext.error).not.toHaveBeenCalled();
  });
});

describe('check-csp CLI', () => {
  const runCli = (root) => {
    const result = spawnSync(process.execPath, [CLI, '--root', root], { cwd: PACK_ROOT, encoding: 'utf8' });
    return { status: result.status, output: `${result.stdout}${result.stderr}` };
  };

  it('P19 reports the off, the clean, the violating and the malformed csp.json', () => {
    const empty = runCli(makeRoot({}));
    expect(empty.status).toBe(0);
    expect(empty.output).toContain('content security policy is off: no csp.json');

    const clean = runCli(makeRoot({ 'csp.json': JSON.stringify(REF), 'index.html': INDEX }));
    expect(clean.status).toBe(0);
    expect(clean.output).toContain('content security policy: enforce, no violation');

    const unsafe = runCli(
      makeRoot({
        'csp.json': JSON.stringify(withDirective(REF, 'script-src', ["'self'", "'unsafe-inline'"])),
        'index.html': INDEX,
      }),
    );
    const unsafeLines = unsafe.output.trim().split('\n');
    expect(unsafe.status).toBe(1);
    expect(unsafeLines.some((line) => line.startsWith('violation csp/script-src — csp.json:'))).toBe(true);
    expect(unsafeLines.at(-1)).toBe('CSP violations: 1');

    const malformed = runCli(makeRoot({ 'csp.json': '{' }));
    expect(malformed.status).toBe(1);
    expect(malformed.output.split('\n').some((line) => line.startsWith('violation csp/config'))).toBe(true);
  });
});

describe('readCspConfig', () => {
  it('returns null for a root without csp.json', () => {
    expect(readCspConfig(makeRoot({}))).toBeNull();
  });

  it('names csp.json when the file is not JSON', () => {
    expect(() => readCspConfig(makeRoot({ 'csp.json': '{' }))).toThrow(/csp\.json/);
  });
});

describe('listCspHeaders', () => {
  it('returns the policy headers of source "/(.*)" and no other header or source', () => {
    const vercel = {
      headers: [
        {
          source: '/(.*)',
          headers: [
            { key: 'X-Frame-Options', value: 'DENY' },
            { key: 'Content-Security-Policy-Report-Only', value: 'v' },
          ],
        },
        { source: '/assets/(.*)', headers: [{ key: 'Content-Security-Policy', value: 'w' }] },
      ],
    };
    expect(listCspHeaders(vercel)).toEqual([{ key: 'Content-Security-Policy-Report-Only', value: 'v' }]);
  });

  it('compares the key without case', () => {
    const vercel = vercelWith([{ key: 'content-security-policy', value: 'v' }]);
    expect(listCspHeaders(vercel)).toEqual([{ key: 'content-security-policy', value: 'v' }]);
  });

  it('returns an empty list for a vercel.json without headers', () => {
    expect(listCspHeaders({ rewrites: [] })).toEqual([]);
  });
});

describe('fromCspHeader', () => {
  const REPORT_ONLY_KEY = 'Content-Security-Policy-Report-Only';
  const REPORT_ONLY_VALUE =
    "default-src 'self'; script-src 'self' https://telegram.org; img-src 'self' data: blob:; frame-ancestors 'self' https://web.telegram.org https://*.telegram.org; report-uri https://csp.example.com/r";

  it('reads a report-only header with the directives in order and the report-uri', () => {
    const config = fromCspHeader({ key: REPORT_ONLY_KEY, value: REPORT_ONLY_VALUE });
    expect(config).toEqual({
      mode: 'report-only',
      directives: {
        'default-src': ["'self'"],
        'script-src': ["'self'", 'https://telegram.org'],
        'img-src': ["'self'", 'data:', 'blob:'],
        'frame-ancestors': ["'self'", 'https://web.telegram.org', 'https://*.telegram.org'],
      },
      reportUri: 'https://csp.example.com/r',
    });
    expect(Object.keys(config.directives)).toEqual(['default-src', 'script-src', 'img-src', 'frame-ancestors']);
    expect(cspHeader(config)).toEqual({ key: REPORT_ONLY_KEY, value: REPORT_ONLY_VALUE });
  });

  it('lowers the names and drops runs of spaces and empty parts', () => {
    const config = fromCspHeader({
      key: 'Content-Security-Policy',
      value: "Default-Src  'self' ;; object-src 'none';",
    });
    expect(config).toEqual({
      mode: 'enforce',
      directives: { 'default-src': ["'self'"], 'object-src': ["'none'"] },
    });
    expect(config).not.toHaveProperty('reportUri');
  });

  it('reads the mode from the key without case', () => {
    const header = { key: 'CONTENT-SECURITY-POLICY-REPORT-ONLY', value: "default-src 'self'" };
    expect(fromCspHeader(header)?.mode).toBe('report-only');
  });

  it.each([
    ['a directive without sources', "default-src 'self'; upgrade-insecure-requests"],
    ['a directive named twice', "default-src 'self'; default-src https:"],
    ['a report-uri that is not the last directive', "report-uri https://a.example/r; default-src 'self'"],
    ['a report-uri that is not https', "default-src 'self'; report-uri http://a.example/r"],
    ['a report-uri with two URLs', "default-src 'self'; report-uri https://a.example/r https://b.example/r"],
    ['a report-uri without a URL', "default-src 'self'; report-uri"],
    ['an empty value', ''],
    ['a value of separators only', ' ; ; '],
    ['a report-uri alone', 'report-uri https://a.example/r'],
  ])('gives null for %s', (_name, value) => {
    expect(fromCspHeader({ key: 'Content-Security-Policy', value })).toBeNull();
  });
});
