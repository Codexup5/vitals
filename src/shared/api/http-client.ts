import { type z } from 'zod';

import { ENV } from '../config/env';
import { REQUEST_TIMEOUT_MS } from '../config/query';
import { ApiError, toHttpApiError } from './api-error';

type QueryValue = string | number | boolean | null | undefined | readonly (string | number)[];
export type QueryParams = Readonly<Record<string, QueryValue>>;

type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

interface RequestOptions {
  query?: QueryParams;
  body?: unknown;
  signal?: AbortSignal | undefined;
}

interface SchemaRequestOptions<Schema extends z.ZodType> extends RequestOptions {
  schema: Schema;
}

export interface AuthAdapter {
  getAccessToken: () => Promise<string | null> | string | null;
  refreshAccessToken: () => Promise<string | null>;
  onUnauthorized?: () => void;
}

let authAdapter: AuthAdapter | null = null;
let refreshInFlight: Promise<string> | null = null;

export const configureAuth = (adapter: AuthAdapter | null) => {
  authAdapter = adapter;
};

const loseSession = (adapter: AuthAdapter, cause?: unknown) => {
  adapter.onUnauthorized?.();
  return new ApiError({
    kind: 'auth',
    status: 401,
    code: 'SESSION_EXPIRED',
    message: 'Access token refresh failed',
    cause,
  });
};

const refreshSession = async (adapter: AuthAdapter) => {
  let token: string | null;
  try {
    token = await adapter.refreshAccessToken();
  } catch (error) {
    throw loseSession(adapter, error);
  }
  if (!token) throw loseSession(adapter);
  return token;
};

const refreshAccessTokenOnce = (adapter: AuthAdapter) => {
  refreshInFlight ??= refreshSession(adapter).finally(() => {
    refreshInFlight = null;
  });
  return refreshInFlight;
};

const buildUrl = (path: string, query: QueryParams = {}) => {
  const url = new URL(`${ENV.apiUrl}${path}`, window.location.origin);
  for (const [key, queryValue] of Object.entries(query)) {
    if (queryValue === null || queryValue === undefined || queryValue === '') continue;
    const values = Array.isArray(queryValue) ? queryValue : [queryValue];
    for (const single of values) url.searchParams.append(key, String(single));
  }
  return url;
};

const combineSignals = (signals: AbortSignal[]) => {
  // Safari < 17.4 has no AbortSignal.any: without a fallback a request with an external signal throws TypeError.
  if ('any' in AbortSignal) return AbortSignal.any(signals);
  const controller = new AbortController();
  for (const source of signals) {
    if (source.aborted) {
      controller.abort(source.reason);
      break;
    }
    source.addEventListener('abort', () => controller.abort(source.reason), {
      once: true,
      signal: controller.signal,
    });
  }
  return controller.signal;
};

interface Sent {
  response: Response;
  timeout: AbortSignal;
}

const readText = async ({ response, timeout }: Sent, signal: AbortSignal | undefined, source: string) => {
  try {
    return await response.text();
  } catch (error) {
    if (signal?.aborted) throw error;
    const isTimeout = timeout.aborted;
    throw new ApiError({
      kind: isTimeout ? 'timeout' : 'network',
      status: 0,
      code: isTimeout ? 'TIMEOUT' : 'BODY_READ',
      message: `${source}: response body was ${isTimeout ? 'not read in time' : 'cut off'}`,
      cause: error,
    });
  }
};

const readBody = async (sent: Sent, signal: AbortSignal | undefined, source: string) => {
  const text = await readText(sent, signal, source);
  if (!text) return undefined;
  try {
    const parsed: unknown = JSON.parse(text);
    return parsed;
  } catch (error) {
    if (error instanceof SyntaxError) return text;
    throw error;
  }
};

const send = async (method: HttpMethod, path: string, options: RequestOptions, isRetry = false): Promise<Sent> => {
  const { query, body, signal } = options;
  const token = await authAdapter?.getAccessToken();
  const headers = new Headers({ Accept: 'application/json' });
  if (body !== undefined) headers.set('Content-Type', 'application/json');
  if (token) headers.set('Authorization', `Bearer ${token}`);

  const timeout = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
  let response: Response;
  try {
    response = await fetch(buildUrl(path, query), {
      method,
      headers,
      signal: signal ? combineSignals([signal, timeout]) : timeout,
      ...(body !== undefined && { body: JSON.stringify(body) }),
    });
  } catch (error) {
    if (signal?.aborted) throw error;
    const kind = timeout.aborted ? 'timeout' : 'network';
    throw new ApiError({
      kind,
      status: 0,
      code: kind.toUpperCase(),
      message: `${method} ${path} failed`,
      cause: error,
    });
  }

  if (response.status === 401 && authAdapter && !isRetry) {
    const currentToken = await authAdapter.getAccessToken();
    if (!currentToken || currentToken === token) await refreshAccessTokenOnce(authAdapter);
    return send(method, path, options, true);
  }
  if (response.status === 401 && authAdapter) throw loseSession(authAdapter);
  if (!response.ok)
    throw toHttpApiError(response.status, await readBody({ response, timeout }, signal, `${method} ${path}`));
  return { response, timeout };
};

const parseBody = <Schema extends z.ZodType>(schema: Schema, body: unknown, source: string): z.output<Schema> => {
  const result = schema.safeParse(body);
  if (result.success) return result.data;

  throw new ApiError({
    kind: 'schema',
    status: 200,
    code: 'SCHEMA_MISMATCH',
    message: `${source}: response does not match schema`,
    cause: result.error,
  });
};

const requestJson = async <Schema extends z.ZodType>(
  method: HttpMethod,
  path: string,
  { schema, ...options }: SchemaRequestOptions<Schema>,
) => {
  const source = `${method} ${path}`;
  const sent = await send(method, path, options);
  return parseBody(schema, await readBody(sent, options.signal, source), source);
};

const requestVoid = async (method: HttpMethod, path: string, options: RequestOptions = {}) => {
  await send(method, path, options);
};

export const httpClient = {
  get: <Schema extends z.ZodType>(path: string, options: SchemaRequestOptions<Schema>) =>
    requestJson('GET', path, options),
  post: <Schema extends z.ZodType>(path: string, options: SchemaRequestOptions<Schema>) =>
    requestJson('POST', path, options),
  put: <Schema extends z.ZodType>(path: string, options: SchemaRequestOptions<Schema>) =>
    requestJson('PUT', path, options),
  patch: <Schema extends z.ZodType>(path: string, options: SchemaRequestOptions<Schema>) =>
    requestJson('PATCH', path, options),
  delete: (path: string, options?: RequestOptions) => requestVoid('DELETE', path, options),
  send: (method: Exclude<HttpMethod, 'GET'>, path: string, options?: RequestOptions) =>
    requestVoid(method, path, options),
};
