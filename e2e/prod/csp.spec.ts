import path from 'node:path';
import { expect, test } from '@playwright/test';

import { cspHeader, readCspConfig, toPolicy, type CspConfig } from '../../tooling/security/csp.mjs';

// the pack root: the csp.json the build and the preview read
const PACK_ROOT = path.resolve(import.meta.dirname, '../..');

const requireConfig = (): CspConfig => {
  const config = readCspConfig(PACK_ROOT);
  if (config === null) throw new Error('csp.json is missing at the pack root');
  return config;
};

// the e2e project has no DOM lib, so the page code is a string the browser runs
const COLLECT_VIOLATIONS = `
  window.cspViolations = [];
  document.addEventListener('securitypolicyviolation', (event) => {
    window.cspViolations.push({ violatedDirective: event.violatedDirective, blockedURI: event.blockedURI });
  });
`;

test('the built app loads under its content security policy', async ({ page }) => {
  const config = requireConfig();
  const consoleMessages: string[] = [];
  page.on('console', (message) => {
    if (message.text().includes('Content Security Policy')) consoleMessages.push(message.text());
  });
  // the listener is in place before any script of the page runs, so no violation is missed
  await page.addInitScript({ content: COLLECT_VIOLATIONS });

  const response = await page.goto('/');
  expect(response, 'the document response of /').not.toBeNull();
  await expect(page.getByRole('main')).toBeVisible();

  const violations = await page.evaluate<unknown>('window.cspViolations');
  expect(violations).toEqual([]);
  expect(consoleMessages).toEqual([]);

  // a deployed URL is served by its host, which sends the header; the preview sends it from csp.json
  if (process.env.E2E_BASE_URL === undefined) {
    const { key, value } = cspHeader(config);
    expect(response?.headers()[key.toLowerCase()]).toBe(value);
  }

  const meta = page.locator('meta[http-equiv="Content-Security-Policy"]');
  if (config.mode === 'enforce') {
    await expect(meta).toHaveAttribute('content', toPolicy(config, 'meta'));
  } else {
    await expect(meta).toHaveCount(0);
  }
});
