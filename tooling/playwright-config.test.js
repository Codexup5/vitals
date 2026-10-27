import { afterEach, describe, expect, it, vi } from 'vitest';

// the environment of a run that names no port, no deployed URL and no visual run
const UNSET = { E2E_PORT: '', E2E_PREVIEW_PORT: '', E2E_BASE_URL: '', E2E_VISUAL: '' };
const PORT_ERROR = /^E2E_PORT must be a whole number from 1 to 65535$/;

// one import per case: the config reads the environment when it is evaluated
const loadConfig = async (env) => {
  vi.resetModules();
  for (const [name, value] of Object.entries({ ...UNSET, ...env })) {
    vi.stubEnv(name, value);
  }
  return (await import('../playwright.config.ts')).default;
};

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('playwright config ports', () => {
  it('uses port 4173 for the base URL and the dev server when no port is set', async () => {
    const config = await loadConfig({});

    expect(config.use.baseURL).toBe('http://localhost:4173');
    expect(config.webServer[0].url).toBe('http://localhost:4173');
    expect(config.webServer[0].command).toContain('--port 4173');
  });

  it('moves the base URL and the dev server to E2E_PORT', async () => {
    const config = await loadConfig({ E2E_PORT: '4180' });

    expect(config.use.baseURL).toBe('http://localhost:4180');
    expect(config.webServer[0].url).toBe('http://localhost:4180');
    expect(config.webServer[0].command).toContain('--port 4180');
  });

  it.each(['abc', '0', '65536', '4.5'])('rejects E2E_PORT %s', async (port) => {
    await expect(loadConfig({ E2E_PORT: port })).rejects.toThrow(PORT_ERROR);
  });
});

describe('playwright config servers', () => {
  it('starts no web server when the run targets a deployed URL', async () => {
    const config = await loadConfig({ E2E_BASE_URL: 'https://preview.example.com' });

    expect(config).not.toHaveProperty('webServer');
  });

  it('starts the dev server and the built app preview when no URL is given', async () => {
    const config = await loadConfig({});

    expect(config.webServer).toHaveLength(2);
    const [dev, prod] = config.webServer;
    expect(dev.url).toBe('http://localhost:4173');
    expect(dev.command).toContain('--port 4173');
    expect(prod.url).toBe('http://localhost:4174');
    expect(prod.command).toContain('vite.js preview');
    expect(prod.command).toContain('--port 4174');
  });
});
