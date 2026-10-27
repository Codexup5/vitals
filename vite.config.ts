import { existsSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { resolve } from 'node:path';
import { Readable } from 'node:stream';
import { buffer } from 'node:stream/consumers';
import { fileURLToPath, URL } from 'node:url';
import { sentryVitePlugin } from '@sentry/vite-plugin';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, type Plugin, type ViteDevServer } from 'vite';

import { cspPlugin } from './tooling/security/csp.mjs';

const MSW_WORKER_FILE = 'mockServiceWorker.js';
const SENTRY_AUTH_TOKEN = process.env.SENTRY_AUTH_TOKEN;
const RELEASE = process.env.VITE_RELEASE;
const API_PREFIX = '/api/';
const API_ENTRY = '/api/hub.ts';
const CLIENT_ENV_PREFIX = 'VITE_';

type FetchHandler = (request: Request) => Promise<Response>;

const isFetchHandler = (value: unknown): value is FetchHandler => typeof value === 'function';

// Source maps are built only for the Sentry upload: without a token they would end up in the public dist
const sentryPlugins = () =>
  SENTRY_AUTH_TOKEN
    ? sentryVitePlugin({
        authToken: SENTRY_AUTH_TOKEN,
        org: process.env.SENTRY_ORG ?? '',
        project: process.env.SENTRY_PROJECT ?? '',
        telemetry: false,
        ...(RELEASE && { release: { name: RELEASE } }),
        sourcemaps: { filesToDeleteAfterUpload: ['dist/**/*.map'] },
      })
    : [];

function dropMswWorker(): Plugin {
  let outDir = '';
  return {
    name: 'drop-msw-worker',
    apply: 'build',
    configResolved(config) {
      outDir = resolve(config.root, config.build.outDir);
    },
    async closeBundle() {
      await rm(resolve(outDir, MSW_WORKER_FILE), { force: true });
    },
  };
}

function toHeaders(incoming: IncomingMessage['headers']): Headers {
  const headers = new Headers();
  for (const [name, value] of Object.entries(incoming)) {
    if (value === undefined) continue;
    for (const item of Array.isArray(value) ? value : [value]) headers.append(name, item);
  }
  return headers;
}

async function serveApi(server: ViteDevServer, req: IncomingMessage, res: ServerResponse): Promise<void> {
  // On every request: ssrLoadModule caches the module and drops the cache itself when server code changes
  const hub: Record<string, unknown> = await server.ssrLoadModule(API_ENTRY);
  const method = req.method ?? 'GET';
  const handler = hub[method];
  if (!isFetchHandler(handler)) {
    res.statusCode = 405;
    res.end();
    return;
  }

  const hasBody = method !== 'GET' && method !== 'HEAD';
  const response = await handler(
    new Request(new URL(req.url ?? API_PREFIX, `http://${req.headers.host ?? 'localhost'}`), {
      method,
      headers: toHeaders(req.headers),
      body: hasBody ? new Uint8Array(await buffer(req)) : undefined,
    }),
  );

  res.statusCode = response.status;
  response.headers.forEach((value, name) => res.setHeader(name, value));
  if (!response.body) {
    res.end();
    return;
  }
  Readable.fromWeb(response.body).pipe(res);
}

// Serves api/hub.ts under /api in dev and e2e; a no-op in a project without the file
function devApi(): Plugin {
  return {
    name: 'dev-api',
    apply: 'serve',
    configureServer(server) {
      const { root, mode } = server.config;
      if (!existsSync(resolve(root, 'api/hub.ts'))) return;

      const env = loadEnv(mode, root, '');
      for (const [name, value] of Object.entries(env)) {
        if (!name.startsWith(CLIENT_ENV_PREFIX)) process.env[name] ??= value;
      }

      server.middlewares.use((req, res, next) => {
        if (!req.url?.startsWith(API_PREFIX)) {
          next();
          return;
        }
        serveApi(server, req, res).catch(next);
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), cspPlugin(), dropMswWorker(), devApi(), sentryPlugins()],
  resolve: {
    // The domain alias goes first: '@' would otherwise take '@/domain/…' into src/
    alias: {
      '@/domain': fileURLToPath(new URL('./domain', import.meta.url)),
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  css: {
    modules: { localsConvention: 'camelCaseOnly' },
  },
  build: { manifest: true, sourcemap: SENTRY_AUTH_TOKEN ? 'hidden' : false },
  // 127.0.0.1 explicitly: a Docker container (Playwright MCP) reaches the host loopback through host.docker.internal, not ::1
  server: { host: '127.0.0.1', port: 3000 },
});
