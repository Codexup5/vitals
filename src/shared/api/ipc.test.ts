import { invoke } from '@tauri-apps/api/core';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

import { callCommand } from './ipc';
import { IpcError } from './ipc-error';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const answerSchema = z.object({ total: z.number() });

afterEach(() => {
  vi.useRealTimers();
});

describe('callCommand', () => {
  it('returns the checked answer', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ total: 3 });
    await expect(callCommand('health_events', { page: 0 }, answerSchema)).resolves.toEqual({ total: 3 });
    expect(invoke).toHaveBeenCalledWith('health_events', { page: 0 });
  });

  it('turns a Rust error into its kind', async () => {
    vi.mocked(invoke).mockRejectedValueOnce({ kind: 'storage', message: 'the database could not be read' });
    await expect(callCommand('health_events', {}, answerSchema)).rejects.toMatchObject({ kind: 'storage' });
  });

  it('rejects an answer of another shape as a schema error', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ total: 'many' });
    const call = callCommand('health_events', {}, answerSchema);
    await expect(call).rejects.toBeInstanceOf(IpcError);
    await expect(call).rejects.toMatchObject({ kind: 'schema' });
  });

  it('reports a missing back end as unavailable', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new TypeError('window.__TAURI_INTERNALS__ is undefined'));
    await expect(callCommand('health_events', {}, answerSchema)).rejects.toMatchObject({ kind: 'unavailable' });
  });

  it('gives up after the timeout', async () => {
    vi.useFakeTimers();
    vi.mocked(invoke).mockReturnValueOnce(new Promise(() => undefined));
    const call = callCommand('health_events', {}, answerSchema);
    const expectation = expect(call).rejects.toMatchObject({ kind: 'timeout' });
    await vi.advanceTimersByTimeAsync(16_000);
    await expectation;
  });

  it('passes an abort through untouched', async () => {
    vi.mocked(invoke).mockReturnValueOnce(new Promise(() => undefined));
    const controller = new AbortController();
    const call = callCommand('health_events', {}, answerSchema, controller.signal);
    const reason = new DOMException('left the screen', 'AbortError');
    controller.abort(reason);
    await expect(call).rejects.toBe(reason);
  });
});
