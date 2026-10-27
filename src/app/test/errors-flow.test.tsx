import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { updatePreferences } from '@/shared/lib/preferences';

import { renderApp } from './render-app';

const counts = { bsod: 0, gpuReset: 0, nvContainer: 940, whea: 0, iommu: 8, appCrashes: 1, errors: 1916 };

const OVERVIEW = {
  summary: { daysWithoutCrash: 0, systemDriveFreeGb: 267.7 },
  firstDay: '2026-07-13',
  days: [
    { day: '2026-10-26', ...counts, crashes: 0, topApps: [] },
    { day: '2026-10-27', ...counts, crashes: 2, topApps: ['Steam.exe ×1'] },
  ],
  months: [{ day: '2026-10', ...counts, crashes: 2, topApps: [] }],
};

const event = (kind: string, detail: string | null) => ({
  time: 1_793_097_600,
  log: 'System',
  provider: kind === 'bsod' ? 'Microsoft-Windows-WER-SystemErrorReporting' : 'Microsoft-Windows-Kernel-Power',
  eventId: kind === 'bsod' ? 1001 : 41,
  level: 2,
  kind,
  detail,
});

type Handler = (command: string, args: Record<string, unknown>) => unknown;

const fakeBackend = (overrides: Partial<Record<string, Handler>> = {}) => {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ command, args });
    const override = overrides[command];
    if (override) return override(command, args);
    if (command === 'health_overview') return OVERVIEW;
    if (command === 'health_events') {
      const kinds = args.kinds as string[];
      return kinds.includes('bsod')
        ? { items: [event('bsod', '0x0000004e')], total: 1 }
        : { items: [event('crash', null), event('bsod', '0x0000004e')], total: 2 };
    }
    if (command === 'minidumps') return [];
    return null;
  });
  return calls;
};

afterEach(() => {
  clearMocks();
});

describe('errors screen', () => {
  it('shows the counts, the chart and the feed of the last 30 days', async () => {
    updatePreferences({ locale: 'ru' });
    fakeBackend();
    renderApp('/errors');

    expect(await screen.findByRole('heading', { level: 1, name: 'Ошибки и падения' })).toBeInTheDocument();
    expect(await screen.findByRole('img', { name: 'Падения по дням' })).toBeInTheDocument();
    const restarts = screen.getByText('Аварийных перезагрузок').closest('li');
    expect(within(restarts as HTMLElement).getByText('2')).toBeInTheDocument();
    expect(await screen.findByText('Найдено: 2')).toBeInTheDocument();
    expect(await screen.findByText(/Дампов нет/)).toBeInTheDocument();
  });

  it('filters the feed by kind and keeps the filter in the address', async () => {
    updatePreferences({ locale: 'ru' });
    const calls = fakeBackend();
    const { router } = renderApp('/errors');
    const filters = await screen.findByRole('group', { name: 'Виды событий' });

    await userEvent.click(within(filters).getByRole('button', { name: 'Синий экран (BSOD)' }));

    expect(await screen.findByText('Найдено: 1')).toBeInTheDocument();
    expect(router.state.location.search).toMatchObject({ kinds: ['bsod'] });
    expect(calls.some((call) => call.command === 'health_events' && (call.args.kinds as string[])[0] === 'bsod')).toBe(
      true,
    );
  });

  it('says what to do when the database cannot be read, and retries', async () => {
    updatePreferences({ locale: 'en' });
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    let isBroken = true;
    fakeBackend({
      health_overview: () => {
        if (isBroken) throw Object.assign(new Error('database is locked'), { kind: 'storage' });
        return OVERVIEW;
      },
    });
    renderApp('/errors?range=90d');

    const alert = await screen.findByRole('alert', {}, { timeout: 10_000 });
    expect(alert).toHaveTextContent('Could not read the Vitals database');
    isBroken = false;
    await userEvent.click(within(alert).getByRole('button', { name: 'Retry' }));
    expect(await screen.findByRole('img', { name: 'Crashes by day' })).toBeInTheDocument();
  });
});
