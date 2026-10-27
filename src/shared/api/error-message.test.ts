import { describe, expect, it } from 'vitest';

import { COMMON_TEXTS } from '../config/texts';
import { ApiError, toHttpApiError } from './api-error';
import { getErrorMessage } from './error-message';

const { errors } = COMMON_TEXTS;

describe('getErrorMessage', () => {
  it.each([
    [401, errors.unauthorized],
    [403, errors.forbidden],
    [404, errors.notFound],
    [409, errors.conflict],
    [422, errors.validation],
    [429, errors.tooManyRequests],
    [500, errors.server],
    [503, errors.server],
    [418, errors.unexpected],
  ])('explains HTTP %i', (status, message) => {
    expect(getErrorMessage(toHttpApiError(status, undefined))).toBe(message);
  });

  it.each([
    ['network', errors.network],
    ['timeout', errors.timeout],
    ['auth', errors.unauthorized],
  ] as const)('explains a %s failure', (kind, message) => {
    expect(getErrorMessage(new ApiError({ kind, status: kind === 'auth' ? 401 : 0, code: 'X', message: '' }))).toBe(
      message,
    );
  });

  it('falls back for an unknown error', () => {
    expect(getErrorMessage(new Error('boom'))).toBe(errors.unexpected);
  });
});
