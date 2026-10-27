import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
import { z } from 'zod';

import { REQUEST_TIMEOUT_MS } from '../config/query';
import { IpcError } from './ipc-error';

// zod probes eval with Function('') when a z.object is built, and script-src without 'unsafe-eval' reports it as a CSP
// violation; this module is the first to load zod in every chunk that calls a command
z.config({ jitless: true });

const commandErrorSchema = z.object({ kind: z.enum(['storage', 'invalid', 'io']), message: z.string() });

const toIpcError = (command: string, error: unknown) => {
  if (error instanceof IpcError) return error;
  const reported = commandErrorSchema.safeParse(error);
  if (reported.success) return new IpcError({ ...reported.data, cause: error });
  // Outside the Tauri window (a plain browser, a test without a fake back end) invoke has nothing to call
  return new IpcError({ kind: 'unavailable', message: `${command}: ${String(error)}`, cause: error });
};

const withTimeout = <T>(command: string, call: Promise<T>, signal?: AbortSignal) =>
  new Promise<T>((resolve, reject) => {
    const settle = () => {
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
    };
    const abort = () => {
      settle();
      const reason: unknown = signal?.reason;
      reject(
        reason instanceof Error || reason instanceof DOMException ? reason : new DOMException('Aborted', 'AbortError'),
      );
    };
    const timer = setTimeout(() => {
      settle();
      reject(new IpcError({ kind: 'timeout', message: `${command}: no answer in ${REQUEST_TIMEOUT_MS} ms` }));
    }, REQUEST_TIMEOUT_MS);
    signal?.addEventListener('abort', abort, { once: true });
    call.then(
      (value) => {
        settle();
        resolve(value);
      },
      (callError: unknown) => {
        settle();
        reject(toIpcError(command, callError));
      },
    );
  });

/** Calls a Rust command and checks its answer against the wire schema; every failure is an `IpcError`. */
export const callCommand = async <Schema extends z.ZodType>(
  command: string,
  args: InvokeArgs,
  schema: Schema,
  signal?: AbortSignal,
): Promise<z.output<Schema>> => {
  let answer: unknown;
  try {
    answer = await withTimeout(command, invoke(command, args), signal);
  } catch (error) {
    if (signal?.aborted && error === signal.reason) throw error;
    throw toIpcError(command, error);
  }
  const parsed = schema.safeParse(answer);
  if (!parsed.success) {
    throw new IpcError({
      kind: 'schema',
      message: `${command}: ${z.prettifyError(parsed.error)}`,
      cause: parsed.error,
    });
  }
  return parsed.data;
};
