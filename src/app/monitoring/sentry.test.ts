import type { Breadcrumb, ErrorEvent } from '@sentry/react';
import { describe, expect, it } from 'vitest';

import { scrubEvent } from './sentry';

const PERSONAL_TEXT = 'anna@example.com +7 (912) 345-67-89';
const SCRUBBED_TEXT = '[email] [phone]';

const toEvent = (patch: Partial<ErrorEvent>): ErrorEvent => ({ type: undefined, ...patch });

const toCrumb = (category: string, payload: Record<string, unknown>): Breadcrumb => ({ category, ['data']: payload });

describe('scrubEvent', () => {
  it('scrubs the event message and drops request cookies and query', () => {
    const event = scrubEvent(
      toEvent({ message: PERSONAL_TEXT, request: { url: '/users', cookies: { id: '1' }, query_string: 'q=a' } }),
    );

    expect(event.message).toBe(SCRUBBED_TEXT);
    expect(event.request).toEqual({ url: '/users' });
  });

  it('scrubs exception values', () => {
    const event = scrubEvent(
      toEvent({ exception: { values: [{ type: 'Error', value: PERSONAL_TEXT }, { type: 'TypeError' }] } }),
    );

    expect(event.exception?.values).toEqual([{ type: 'Error', value: SCRUBBED_TEXT }, { type: 'TypeError' }]);
  });

  it('scrubs breadcrumb messages', () => {
    const event = scrubEvent(toEvent({ breadcrumbs: [{ message: PERSONAL_TEXT }, { category: 'ui.click' }] }));

    expect(event.breadcrumbs).toEqual([{ message: SCRUBBED_TEXT }, { category: 'ui.click' }]);
  });

  it('scrubs strings nested in extra, arrays included', () => {
    const event = scrubEvent(
      toEvent({ extra: { scope: ['users', { search: 'anna@example.com', tags: [PERSONAL_TEXT] }], page: 2 } }),
    );

    expect(event.extra).toEqual({ scope: ['users', { search: '[email]', tags: [SCRUBBED_TEXT] }], page: 2 });
  });

  it('scrubs strings nested in contexts', () => {
    const event = scrubEvent(toEvent({ contexts: { form: { contact: { phone: '+7 (912) 345-67-89' } } } }));

    expect(event.contexts).toEqual({ form: { contact: { phone: '[phone]' } } });
  });

  it('cuts values nested deeper than the depth cap', () => {
    const deep = { a: { b: { c: { d: { e: { f: { g: { h: { i: { j: { k: 'anna@example.com' } } } } } } } } } } };
    const event = scrubEvent(toEvent({ extra: { deep } }));

    expect(JSON.stringify(event.extra)).not.toContain('anna@example.com');
  });

  it('drops the query string from the request url', () => {
    const event = scrubEvent(toEvent({ request: { url: 'https://app.test/users?search=anna' } }));

    expect(event.request?.url).toBe('https://app.test/users');
  });

  it('drops the query string from breadcrumb url, from and to', () => {
    const event = scrubEvent(
      toEvent({
        breadcrumbs: [
          toCrumb('fetch', { url: '/api/users?search=anna', status: 200 }),
          toCrumb('navigation', { from: '/users?search=anna', to: '/users/1?tab=a' }),
        ],
      }),
    );

    expect(event.breadcrumbs).toEqual([
      toCrumb('fetch', { url: '/api/users', status: 200 }),
      toCrumb('navigation', { from: '/users', to: '/users/1' }),
    ]);
  });

  it('keeps the trace context and ids made of digits', () => {
    const trace = { trace_id: 'c0ffee1234567890dead1234567890ab', span_id: '1234567890abcdef' };
    const event = scrubEvent(
      toEvent({
        contexts: { trace },
        extra: { scope: ['user', 'detail', 'c0ffee12-3456-7890-dead-beef12345678'] },
      }),
    );

    expect(event.contexts).toEqual({ trace });
    expect(event.extra).toEqual({ scope: ['user', 'detail', 'c0ffee12-3456-7890-dead-beef12345678'] });
  });

  it('keeps iso dates in a message', () => {
    const message = 'created 2026-01-12, updated 2026-01-12T09:00:00.000Z, seen 2026-01-12 09:00:00';

    expect(scrubEvent(toEvent({ message })).message).toBe(message);
  });

  it.each(['+7 (912) 345-67-89', '8 912 345 67 89', '89123456789'])('scrubs the phone %s', (phone) => {
    expect(scrubEvent(toEvent({ message: `call ${phone} now` })).message).toBe('call [phone] now');
  });
});
