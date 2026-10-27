import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

import { WEB_VITALS_BUDGETS } from './web-vitals-budgets';

// the IIFE build of the installed web-vitals sits beside its main file and declares var webVitals
const IIFE_PATH = path.join(path.dirname(fileURLToPath(import.meta.resolve('web-vitals'))), 'web-vitals.iife.js');

// the e2e project has no DOM lib, so the page code is a string the browser runs; the latest value of each metric
// goes into window.reportedVitals, and reportAllChanges keeps the last change of LCP and CLS
const REGISTER_VITALS = `
  window.reportedVitals = {};
  const keep = (metric) => {
    window.reportedVitals[metric.name] = metric.value;
  };
  webVitals.onLCP(keep, { reportAllChanges: true });
  webVitals.onCLS(keep, { reportAllChanges: true });
  webVitals.onINP(keep, { reportAllChanges: true });
`;

// web-vitals finalizes its metrics when the page becomes hidden: the visibility is overridden and the event sent
const HIDE_PAGE = `(() => {
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
  document.dispatchEvent(new Event('visibilitychange', { bubbles: true }));
})()`;

interface ReportedVitals {
  LCP?: number;
  CLS?: number;
  INP?: number;
}

test('the built app holds the web vitals budgets', async ({ page }) => {
  // injected by the test, not by the app: the page CSP does not apply, and the app's own seam is not the judge
  // Playwright runs each init script in its own function scope, so the var of the IIFE is seen only by code in the same script
  await page.addInitScript({
    content: `${fs.readFileSync(IIFE_PATH, 'utf8')}
${REGISTER_VITALS}`,
  });

  await page.goto('/');
  await expect(page.getByRole('main')).toBeVisible();
  // the click is the interaction INP measures; the filter form renders in every state
  await page.getByRole('button', { name: 'Найти' }).click();
  await page.evaluate(HIDE_PAGE);

  const reported = await page.evaluate<ReportedVitals>('window.reportedVitals');
  expect(reported.LCP, 'LCP is reported').toBeDefined();
  expect(reported.LCP).toBeLessThanOrEqual(WEB_VITALS_BUDGETS.LCP);
  // CLS is 0 when no layout shift is reported
  expect(reported.CLS ?? 0).toBeLessThanOrEqual(WEB_VITALS_BUDGETS.CLS);
  // an interaction under the Event Timing threshold reports no INP
  if (reported.INP !== undefined) expect(reported.INP).toBeLessThanOrEqual(WEB_VITALS_BUDGETS.INP);
});
