import { init, type ErrorEvent } from '@sentry/react';

import { ENV } from '@/shared/config/env';

const EMAIL_PATTERN = /[\w.+-]+@[\w-]+\.[\w.-]+/g;
const PHONE_PATTERN = /(?<!\w)\+?\d(?:[\s().-]{0,2}\d){9,14}(?![\w:])/g;
const URL_KEYS = ['url', 'from', 'to'];
const MAX_SCRUB_DEPTH = 6;
const TRUNCATED = '[truncated]';

const scrubText = (text: string) => text.replace(EMAIL_PATTERN, '[email]').replace(PHONE_PATTERN, '[phone]');

const dropQuery = (url: string) => {
  const queryStart = url.indexOf('?');
  return queryStart === -1 ? url : url.slice(0, queryStart);
};

const isPlainObject = (value: unknown): value is Record<string, unknown> => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
};

const scrubValue = (value: unknown, depth: number): unknown => {
  if (typeof value === 'string') return scrubText(value);
  if (!Array.isArray(value) && !isPlainObject(value)) return value;
  if (depth >= MAX_SCRUB_DEPTH) return TRUNCATED;
  if (Array.isArray(value)) return value.map((item: unknown) => scrubValue(item, depth + 1));
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, scrubValue(item, depth + 1)]));
};

const scrubRecord = (record: Record<string, unknown>) => {
  for (const [key, value] of Object.entries(record)) {
    record[key] = scrubValue(value, 1);
  }
};

const dropUrlQueries = (record: Record<string, unknown>) => {
  for (const key of URL_KEYS) {
    const value = record[key];
    if (typeof value === 'string') record[key] = dropQuery(value);
  }
};

export const scrubEvent = (event: ErrorEvent) => {
  if (event.request) {
    delete event.request.data;
    delete event.request.cookies;
    delete event.request.query_string;
    if (event.request.url) event.request.url = scrubText(dropQuery(event.request.url));
  }
  if (event.message) event.message = scrubText(event.message);
  if (event.extra) scrubRecord(event.extra);
  if (event.contexts) {
    const { trace, ...rest } = event.contexts;
    scrubRecord(rest);
    event.contexts = trace ? { ...rest, trace } : rest;
  }
  for (const exception of event.exception?.values ?? []) {
    if (exception.value) exception.value = scrubText(exception.value);
  }
  for (const breadcrumb of event.breadcrumbs ?? []) {
    if (breadcrumb.message) breadcrumb.message = scrubText(breadcrumb.message);
    if (breadcrumb.data) {
      dropUrlQueries(breadcrumb.data);
      scrubRecord(breadcrumb.data);
    }
  }
  return event;
};

export const initMonitoring = () => {
  if (!ENV.sentryDsn) return;
  init({
    dsn: ENV.sentryDsn,
    environment: ENV.mode,
    ...(ENV.release && { release: ENV.release }),
    dataCollection: {
      userInfo: false,
      cookies: false,
      httpHeaders: false,
      httpBodies: [],
      urlQueryParams: false,
    },
    beforeSend: scrubEvent,
  });
};
