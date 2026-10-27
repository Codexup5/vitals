// The content security policy: one csp.json, judged here and written into the build and the preview.

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const CONFIG_FILE = 'csp.json';
const CONFIG_FIELDS = ['mode', 'directives', 'reportUri'];
const MODES = ['enforce', 'report-only'];
const REQUIRED_DIRECTIVES = ['default-src', 'script-src', 'object-src', 'base-uri', 'frame-ancestors'];
const META_UNSUPPORTED = ['frame-ancestors', 'report-uri', 'report-to', 'sandbox'];
const UNSAFE_SCRIPT_SOURCES = ["'unsafe-inline'", "'unsafe-eval'", '*', 'data:', 'blob:', 'http:', 'https:'];
const HOSTING_SOURCE = '/(.*)';
const CSP_HEADER_KEYS = { enforce: 'Content-Security-Policy', 'report-only': 'Content-Security-Policy-Report-Only' };
const ORIGIN_VARIABLES = ['VITE_API_URL', 'VITE_SENTRY_DSN', 'VITE_VITALS_URL', 'VITE_SUPABASE_URL'];
const DEFAULT_PORTS = { 'http:': '80', 'https:': '443' };
const SCHEME_SOURCE = /^([a-z][a-z0-9+.-]*):$/i;
const HOST_SOURCE = /^(?:([a-z][a-z0-9+.-]*):\/\/)?(\*\.[^/:*]+|[^/:*]+)(?::(\d+|\*))?(?:\/.*)?$/i;
const HAND_META = /<meta\b[^>]*\bhttp-equiv\s*=\s*["']?\s*content-security-policy/i;

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);
const finding = (id, file, detail) => ({ id, file, detail });
const describeError = (error) => (error instanceof Error ? error.message : String(error));

export function readCspConfig(root) {
  const file = path.join(root, CONFIG_FILE);
  if (!existsSync(file)) return null;
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    throw new Error(`${CONFIG_FILE} is not valid JSON: ${describeError(error)}`, { cause: error });
  }
}

function sourceDefects(name, sources) {
  const label = `directives.${name}`;
  if (!Array.isArray(sources) || sources.length === 0) {
    return [`"${label}" must be a non-empty list of strings`];
  }
  const broken = sources.filter((source) => typeof source !== 'string' || !/^[^\s;,]+$/.test(source));
  return broken.length === 0 ? [] : [`"${label}" holds a source that is not a string without spaces, ";" and ","`];
}

// The shape of csp.json; the other rules read a config only after it passes.
function configDefects(config) {
  if (!isObject(config)) return [finding('csp/config', CONFIG_FILE, `${CONFIG_FILE} must be a JSON object`)];
  const problems = Object.keys(config)
    .filter((field) => !CONFIG_FIELDS.includes(field))
    .map((field) => `"${field}" is not a field of ${CONFIG_FILE}: allowed are ${CONFIG_FIELDS.join(', ')}`);
  if (!MODES.includes(config.mode)) problems.push(`"mode" must be one of ${MODES.join(', ')}`);
  if (!isObject(config.directives)) {
    problems.push('"directives" must be an object of directive name to a list of sources');
  } else {
    for (const [name, sources] of Object.entries(config.directives)) problems.push(...sourceDefects(name, sources));
  }
  if (
    config.reportUri !== undefined &&
    !(typeof config.reportUri === 'string' && /^https:\/\/\S+$/.test(config.reportUri))
  ) {
    problems.push('"reportUri" must be an https:// URL');
  }
  return problems.map((problem) => finding('csp/config', CONFIG_FILE, problem));
}

const directiveText = ([name, sources]) => `${name} ${sources.join(' ')}`;

export function toPolicy(config, delivery) {
  const entries = Object.entries(config.directives);
  const parts =
    delivery === 'meta'
      ? entries.filter(([name]) => !META_UNSUPPORTED.includes(name)).map(directiveText)
      : entries.map(directiveText);
  if (delivery === 'header' && config.reportUri) parts.push(`report-uri ${config.reportUri}`);
  return parts.join('; ');
}

export function cspHeader(config) {
  return { key: CSP_HEADER_KEYS[config.mode], value: toPolicy(config, 'header') };
}

const isPolicyKey = (key) =>
  Object.values(CSP_HEADER_KEYS).some((policyKey) => policyKey.toLowerCase() === String(key).toLowerCase());

// the content security policy headers of the source every page matches, whatever the case of the key
export function listCspHeaders(vercel) {
  const entries = isObject(vercel) && Array.isArray(vercel.headers) ? vercel.headers : [];
  return entries
    .filter((entry) => isObject(entry) && entry.source === HOSTING_SOURCE && Array.isArray(entry.headers))
    .flatMap((entry) => entry.headers)
    .filter((header) => isObject(header) && isPolicyKey(header.key));
}

// the inverse of cspHeader: null unless the header is the normalized text of a csp.json that passes its checks
export function fromCspHeader(header) {
  const mode = Object.keys(CSP_HEADER_KEYS).find(
    (name) => CSP_HEADER_KEYS[name].toLowerCase() === String(header.key).toLowerCase(),
  );
  if (mode === undefined || typeof header.value !== 'string') return null;
  const parts = header.value
    .split(';')
    .map((part) => part.trim().split(/\s+/))
    .filter(([name]) => name !== '')
    .map(([name, ...sources]) => ({ name: name.toLowerCase(), sources }));
  const last = parts.length - 1;
  if (parts.some(({ name, sources }, index) => sources.length === 0 || (name === 'report-uri' && index !== last))) {
    return null;
  }
  const report = parts[last]?.name === 'report-uri' ? parts[last] : undefined;
  const directives = parts.filter((part) => part !== report);
  if (
    directives.length === 0 ||
    new Set(directives.map(({ name }) => name)).size !== directives.length ||
    (report !== undefined && !(report.sources.length === 1 && /^https:\/\/\S+$/.test(report.sources[0])))
  ) {
    return null;
  }
  const config = {
    mode,
    directives: Object.fromEntries(directives.map(({ name, sources }) => [name, sources])),
    ...(report === undefined ? {} : { reportUri: report.sources[0] }),
  };
  const normalized = parts.map(({ name, sources }) => `${name} ${sources.join(' ')}`).join('; ');
  return configDefects(config).length === 0 && toPolicy(config, 'header') === normalized ? config : null;
}

function scriptDefects(directives) {
  const defects = [];
  for (const name of ['script-src', 'default-src']) {
    const unsafe = (directives[name] ?? []).filter(
      (source) => UNSAFE_SCRIPT_SOURCES.includes(source.toLowerCase()) || /^http:\/\//i.test(source),
    );
    if (unsafe.length > 0) {
      defects.push(
        finding(
          'csp/script-src',
          CONFIG_FILE,
          `${name} allows ${unsafe.join(' ')}: scripts may come from 'self' or a pinned https:// host only`,
        ),
      );
    }
  }
  return defects;
}

function hostingDefects(config, vercel) {
  const expected = cspHeader(config);
  const cspHeaders = listCspHeaders(vercel);
  const wanted = `"${expected.key}: ${expected.value}"`;
  const where = `the headers of source "${HOSTING_SOURCE}"`;
  if (cspHeaders.length !== 1) {
    return [
      finding(
        'csp/hosting',
        'vercel.json',
        `${where} must hold exactly one content security policy header, found ${cspHeaders.length}: set ${wanted}`,
      ),
    ];
  }
  const [header] = cspHeaders;
  // the header is read as adopt reads it, so a key in another case, a trailing ; or no space after ; is the same policy
  const parsed = fromCspHeader(header);
  const actual = parsed === null ? null : cspHeader(parsed);
  if (actual !== null && actual.key === expected.key && actual.value === expected.value) return [];
  return [finding('csp/hosting', 'vercel.json', `${where} holds "${header.key}: ${header.value}": set ${wanted}`)];
}

export function findCspDefects({ config, indexHtml, vercel }) {
  const invalid = configDefects(config);
  if (invalid.length > 0) return invalid;

  const { directives } = config;
  const defects = REQUIRED_DIRECTIVES.filter((name) => !(name in directives)).map((name) =>
    finding('csp/required', CONFIG_FILE, `${name} is missing: add it to directives of ${CONFIG_FILE}`),
  );
  defects.push(...scriptDefects(directives));
  const objectSrc = directives['object-src'];
  if (objectSrc && !(objectSrc.length === 1 && objectSrc[0] === "'none'")) {
    defects.push(
      finding('csp/object-src', CONFIG_FILE, `object-src is ${objectSrc.join(' ')}: it must be exactly 'none'`),
    );
  }
  const baseUri = directives['base-uri'];
  if (baseUri && !(baseUri.length === 1 && ["'self'", "'none'"].includes(baseUri[0]))) {
    defects.push(
      finding('csp/base-uri', CONFIG_FILE, `base-uri is ${baseUri.join(' ')}: it must be exactly 'self' or 'none'`),
    );
  }
  if (indexHtml !== null && HAND_META.test(indexHtml)) {
    defects.push(
      finding(
        'csp/hand-meta',
        'index.html',
        'index.html holds a <meta http-equiv="Content-Security-Policy">: remove it, the build writes the meta from csp.json',
      ),
    );
  }
  if (vercel !== null) defects.push(...hostingDefects(config, vercel));
  return defects;
}

function matchesSource(source, origin) {
  const target = new URL(origin);
  if (source === '*') return true;
  const scheme = SCHEME_SOURCE.exec(source);
  if (scheme) {
    const named = `${scheme[1]?.toLowerCase()}:`;
    return named === target.protocol || (named === 'http:' && target.protocol === 'https:');
  }
  const host = HOST_SOURCE.exec(source);
  if (!host) return false;
  const [, sourceScheme, sourceHost = '', sourcePort] = host;
  if (sourceScheme && `${sourceScheme.toLowerCase()}:` !== target.protocol) return false;
  const port = target.port || DEFAULT_PORTS[target.protocol];
  if (sourcePort !== '*' && (sourcePort ?? DEFAULT_PORTS[target.protocol]) !== port) return false;
  const name = sourceHost.toLowerCase();
  return name.startsWith('*.') ? target.hostname.endsWith(name.slice(1)) : target.hostname === name;
}

export function findOriginDefects(config, env) {
  const sources = config.directives['connect-src'] ?? config.directives['default-src'] ?? [];
  const defects = [];
  for (const variable of ORIGIN_VARIABLES) {
    const value = env[variable];
    if (typeof value !== 'string' || !/^https?:\/\//i.test(value) || !URL.canParse(value)) continue;
    const { origin } = new URL(value);
    if (sources.some((source) => matchesSource(source, origin))) continue;
    defects.push(
      finding(
        'csp/connect-src',
        CONFIG_FILE,
        `${variable} points to ${origin}, which connect-src does not allow: add "${origin}" to connect-src of ${CONFIG_FILE}`,
      ),
    );
  }
  return defects;
}

export function cspPlugin() {
  let state = null;
  let shapeDefects = [];
  let command = null;
  return {
    name: 'kit-csp',
    configResolved(resolved) {
      const config = readCspConfig(resolved.root);
      shapeDefects = config === null ? [] : configDefects(config);
      // An invalid shape fails the build below; dev and preview then run without a policy, as with no csp.json.
      const isUsable = config !== null && shapeDefects.length === 0;
      state = isUsable ? { config, command: resolved.command, env: resolved.env } : null;
      command = resolved.command;
    },
    buildStart() {
      if (command === 'build' && shapeDefects.length > 0) {
        this.error(shapeDefects.map((defect) => defect.detail).join('\n'));
      }
      if (state === null || state.command !== 'build') return;
      const defects = findOriginDefects(state.config, state.env);
      if (defects.length === 0) return;
      const message = defects.map((defect) => defect.detail).join('\n');
      if (state.config.mode === 'enforce') this.error(message);
      else this.warn(message);
    },
    transformIndexHtml(_html, context) {
      if (state === null || state.config.mode !== 'enforce' || context?.server) return [];
      return [
        {
          tag: 'meta',
          attrs: { 'http-equiv': 'Content-Security-Policy', content: toPolicy(state.config, 'meta') },
          injectTo: 'head-prepend',
        },
      ];
    },
    configurePreviewServer(server) {
      if (state === null) return;
      const { key, value } = cspHeader(state.config);
      server.middlewares.use((_request, response, next) => {
        response.setHeader(key, value);
        next();
      });
    },
  };
}
