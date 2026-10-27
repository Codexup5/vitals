import { delay, http, HttpResponse } from 'msw';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

import { server } from '../../../mocks/node';
import { isApiError } from './api-error';
import { configureAuth, httpClient } from './http-client';

const pingSchema = z.object({ ok: z.boolean() });

afterEach(() => configureAuth(null));

describe('httpClient', () => {
  it('parses a response through the schema', async () => {
    server.use(http.get('*/api/ping', () => HttpResponse.json({ ok: true })));
    await expect(httpClient.get('/ping', { schema: pingSchema })).resolves.toEqual({ ok: true });
  });

  it('serializes query params and skips empty values', async () => {
    let requestedUrl = '';
    server.use(
      http.get('*/api/ping', ({ request }) => {
        requestedUrl = request.url;
        return HttpResponse.json({ ok: true });
      }),
    );
    await httpClient.get('/ping', { schema: pingSchema, query: { page: 2, search: '', tag: ['a', 'b'] } });
    expect(new URL(requestedUrl).search).toBe('?page=2&tag=a&tag=b');
  });

  it('normalizes an HTTP error with field errors', async () => {
    server.use(
      http.post('*/api/ping', () =>
        HttpResponse.json({ code: 'VALIDATION', message: 'bad', fields: { email: 'taken' } }, { status: 422 }),
      ),
    );
    const error: unknown = await httpClient
      .post('/ping', { schema: pingSchema, body: {} })
      .catch((caught: unknown) => caught);
    expect(isApiError(error) && { status: error.status, code: error.code, fields: error.fields }).toEqual({
      status: 422,
      code: 'VALIDATION',
      fields: { email: 'taken' },
    });
  });

  it('fails loudly in development when the backend breaks the contract', async () => {
    server.use(http.get('*/api/ping', () => HttpResponse.json({ ok: 'yes' })));
    const error: unknown = await httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught);
    expect(isApiError(error) && error.kind).toBe('schema');
  });

  describe('in production', () => {
    beforeEach(() => {
      vi.stubEnv('DEV', false);
    });

    afterEach(() => {
      vi.unstubAllEnvs();
    });

    it('rejects a response that breaks the contract too', async () => {
      server.use(http.get('*/api/ping', () => HttpResponse.json({ ok: 'yes' })));
      const error: unknown = await httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught);
      expect(isApiError(error) && error.kind).toBe('schema');
    });
  });

  it('turns a network failure into ApiError', async () => {
    server.use(http.get('*/api/ping', () => HttpResponse.error()));
    const error: unknown = await httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught);
    expect(isApiError(error) && error.kind).toBe('network');
  });

  it('turns a cut-off response body into a network ApiError', async () => {
    server.use(
      http.get(
        '*/api/ping',
        () =>
          new HttpResponse(
            new ReadableStream({
              pull: (controller) => controller.error(new Error('connection reset')),
            }),
          ),
      ),
    );
    const error: unknown = await httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught);
    expect(isApiError(error) && { kind: error.kind, code: error.code }).toEqual({ kind: 'network', code: 'BODY_READ' });
  });

  it('reports a timeout that hits while the body is still being read', async () => {
    const timeoutController = new AbortController();
    vi.spyOn(AbortSignal, 'timeout').mockReturnValue(timeoutController.signal);
    server.use(
      http.get(
        '*/api/ping',
        () =>
          new HttpResponse(
            new ReadableStream({
              pull: (controller) => {
                timeoutController.abort();
                controller.error(new Error('aborted by timeout'));
              },
            }),
          ),
      ),
    );
    const error: unknown = await httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught);
    expect(isApiError(error) && { kind: error.kind, code: error.code }).toEqual({ kind: 'timeout', code: 'TIMEOUT' });
  });

  describe('without AbortSignal.any', () => {
    const nativeAny = Object.getOwnPropertyDescriptor(AbortSignal, 'any');

    beforeEach(() => {
      Reflect.deleteProperty(AbortSignal, 'any');
    });
    afterEach(() => {
      if (nativeAny) Object.defineProperty(AbortSignal, 'any', nativeAny);
    });

    it('still cancels the request with the outer signal', async () => {
      const requestSignalReached = new Promise<AbortSignal>((resolve) => {
        server.use(
          http.get('*/api/ping', async ({ request }) => {
            resolve(request.signal);
            await delay('infinite');
            return HttpResponse.json({ ok: true });
          }),
        );
      });
      const controller = new AbortController();
      const request = httpClient.get('/ping', { schema: pingSchema, signal: controller.signal });
      const requestSignal = await requestSignalReached;
      controller.abort();
      const error: unknown = await request.catch((caught: unknown) => caught);
      expect({ isApiError: isApiError(error), isRequestAborted: requestSignal.aborted }).toEqual({
        isApiError: false,
        isRequestAborted: true,
      });
    });
  });

  it('refreshes the token once for parallel 401 responses', async () => {
    const refreshAccessToken = vi.fn(() => Promise.resolve('fresh'));
    server.use(
      http.get('*/api/ping', ({ request }) =>
        request.headers.get('Authorization') === 'Bearer stale'
          ? new HttpResponse(null, { status: 401 })
          : HttpResponse.json({ ok: true }),
      ),
    );
    configureAuth({
      getAccessToken: () => (refreshAccessToken.mock.calls.length > 0 ? 'fresh' : 'stale'),
      refreshAccessToken,
    });

    await Promise.all([
      httpClient.get('/ping', { schema: pingSchema }),
      httpClient.get('/ping', { schema: pingSchema }),
    ]);
    expect(refreshAccessToken).toHaveBeenCalledTimes(1);
  });

  it('retries with the current token when a 401 arrives after the refresh', async () => {
    let accessToken = 'stale';
    const refreshAccessToken = vi.fn(() => {
      accessToken = 'fresh';
      return Promise.resolve('fresh');
    });
    let releaseLateRequest: (() => void) | undefined;
    const lateRequestReleased = new Promise<void>((resolve) => {
      releaseLateRequest = resolve;
    });
    let markLateRequestHeld: (() => void) | undefined;
    const lateRequestHeld = new Promise<void>((resolve) => {
      markLateRequestHeld = resolve;
    });
    server.use(
      http.get('*/api/late', async ({ request }) => {
        if (request.headers.get('Authorization') === 'Bearer fresh') return HttpResponse.json({ ok: true });
        markLateRequestHeld?.();
        await lateRequestReleased;
        return new HttpResponse(null, { status: 401 });
      }),
      http.get('*/api/ping', ({ request }) =>
        request.headers.get('Authorization') === 'Bearer fresh'
          ? HttpResponse.json({ ok: true })
          : new HttpResponse(null, { status: 401 }),
      ),
    );
    configureAuth({ getAccessToken: () => accessToken, refreshAccessToken });

    const lateRequest = httpClient.get('/late', { schema: pingSchema });
    await lateRequestHeld;
    await httpClient.get('/ping', { schema: pingSchema });
    releaseLateRequest?.();

    await expect(lateRequest).resolves.toEqual({ ok: true });
    expect(refreshAccessToken).toHaveBeenCalledTimes(1);
  });

  it('reports a lost session when the server rejects the refreshed token too', async () => {
    let accessToken = 'stale';
    const refreshAccessToken = vi.fn(() => {
      accessToken = 'fresh';
      return Promise.resolve(accessToken);
    });
    const onUnauthorized = vi.fn();
    server.use(http.get('*/api/ping', () => new HttpResponse(null, { status: 401 })));
    configureAuth({ getAccessToken: () => accessToken, refreshAccessToken, onUnauthorized });

    const error = await httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught);

    expect(isApiError(error) && { kind: error.kind, status: error.status }).toEqual({ kind: 'auth', status: 401 });
    expect({ refreshes: refreshAccessToken.mock.calls.length, losses: onUnauthorized.mock.calls.length }).toEqual({
      refreshes: 1,
      losses: 1,
    });
  });

  it('reports a lost session once when the refresh is rejected', async () => {
    let rejectRefresh: ((reason: Error) => void) | undefined;
    const refreshOutcome = new Promise<string | null>((_resolve, reject) => {
      rejectRefresh = reject;
    });
    const refreshAccessToken = vi.fn(() => refreshOutcome);
    const getAccessToken = vi.fn(() => 'stale');
    const onUnauthorized = vi.fn();
    server.use(http.get('*/api/ping', () => new HttpResponse(null, { status: 401 })));
    configureAuth({ getAccessToken, refreshAccessToken, onUnauthorized });

    const requests = [
      httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught),
      httpClient.get('/ping', { schema: pingSchema }).catch((caught: unknown) => caught),
    ];
    await vi.waitFor(() => expect(getAccessToken).toHaveBeenCalledTimes(4));
    rejectRefresh?.(new Error('refresh token expired'));
    const errors = await Promise.all(requests);

    expect(errors.map((error) => isApiError(error) && { kind: error.kind, status: error.status })).toEqual([
      { kind: 'auth', status: 401 },
      { kind: 'auth', status: 401 },
    ]);
    expect({ refreshes: refreshAccessToken.mock.calls.length, losses: onUnauthorized.mock.calls.length }).toEqual({
      refreshes: 1,
      losses: 1,
    });
  });
});
