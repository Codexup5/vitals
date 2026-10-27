import { defineConfig, devices } from '@playwright/test';

// E2E_PORT and E2E_PREVIEW_PORT give each server its own port: parallel CI jobs on one PC would meet on a fixed one
const parsePort = (name: string, fallback: number, raw: string | undefined): number => {
  if (raw === undefined || raw === '') return fallback;
  const port = /^\d+$/.test(raw) ? Number(raw) : Number.NaN;
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`${name} must be a whole number from 1 to 65535`);
  }
  return port;
};

const PORT = parsePort('E2E_PORT', 4173, process.env.E2E_PORT);
// the built app, served by vite preview with the header of csp.json: the prod project runs against it
const PREVIEW_PORT = parsePort('E2E_PREVIEW_PORT', 4174, process.env.E2E_PREVIEW_PORT);
const PREVIEW_URL = `http://localhost:${PREVIEW_PORT}`;
// pnpm is not in the Playwright image: the web servers run the vite binary with node
const VITE_BIN = 'node node_modules/vite/bin/vite.js';

// E2E_BASE_URL points the run at a deployed URL (then no dev server starts), E2E_HEADERS is a JSON object of headers
// every request carries: the signed init data, the deployment protection bypass
const deployedUrl = process.env.E2E_BASE_URL;

const parseHeaders = (raw: string | undefined): Record<string, string> => {
  if (raw === undefined || raw === '') return {};
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error('E2E_HEADERS must be a JSON object of header names and string values', { cause: error });
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throw new Error('E2E_HEADERS must be a JSON object of header names and string values');
  }
  const headers: Record<string, string> = {};
  for (const [name, value] of Object.entries(parsed)) {
    if (typeof value !== 'string') throw new Error(`E2E_HEADERS: the value of "${name}" must be a string`);
    headers[name] = value;
  }
  return headers;
};

// E2E_VISUAL is set by the Playwright image only (pnpm test:visual): the visual baseline is rendered on Linux alone
const isVisual = Boolean(process.env.E2E_VISUAL);
if (isVisual && process.platform !== 'linux') {
  throw new Error(
    'E2E_VISUAL renders the visual baseline on Linux only: run pnpm test:visual, which starts the Playwright image in Docker',
  );
}

const visualProject = (name: string, device: (typeof devices)[string]) => ({
  name,
  use: { ...device, locale: 'ru-RU', timezoneId: 'UTC' },
  testMatch: 'visual/**/*.spec.ts',
});

const devServer = {
  command: `${VITE_BIN} --port ${PORT} --strictPort`,
  url: `http://localhost:${PORT}`,
  reuseExistingServer: !process.env.CI,
};

const prodServer = {
  command: `${VITE_BIN} build && ${VITE_BIN} preview --port ${PREVIEW_PORT} --strictPort`,
  url: PREVIEW_URL,
  env: { VITE_API_URL: '/api' },
  reuseExistingServer: !process.env.CI,
  timeout: 120_000,
};

const webServerList = () => {
  if (isVisual) return [devServer];
  return deployedUrl ? undefined : [devServer, prodServer];
};

// no server to start: the webServer key is left out, never passed as undefined
const servers = webServerList();

export default defineConfig({
  testDir: './e2e',
  snapshotPathTemplate: '{testDir}/__screenshots__/{testFilePath}/{arg}-{projectName}{ext}',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  expect: { toHaveScreenshot: { animations: 'disabled', caret: 'hide' } },
  use: {
    baseURL: deployedUrl && !isVisual ? deployedUrl : `http://localhost:${PORT}`,
    extraHTTPHeaders: parseHeaders(process.env.E2E_HEADERS),
    trace: 'retain-on-failure',
  },
  projects: isVisual
    ? [visualProject('visual-mobile', devices['Pixel 7']), visualProject('visual-desktop', devices['Desktop Chrome'])]
    : [
        { name: 'mobile', use: { ...devices['Pixel 7'] }, testIgnore: ['prod/**', 'visual/**'] },
        { name: 'desktop', use: { ...devices['Desktop Chrome'] }, testIgnore: ['prod/**', 'visual/**'] },
        {
          name: 'prod',
          use: { ...devices['Pixel 7'], baseURL: deployedUrl ?? PREVIEW_URL },
          testMatch: 'prod/**/*.spec.ts',
        },
      ],
  ...(servers === undefined ? {} : { webServer: servers }),
});
