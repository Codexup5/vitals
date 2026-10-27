import { COMMON_TEXTS } from '../config/texts';
import { isApiError } from './api-error';

export const getErrorMessage = (error: unknown) => {
  if (!isApiError(error)) return COMMON_TEXTS.errors.unexpected;
  if (error.kind === 'network') return COMMON_TEXTS.errors.network;
  if (error.kind === 'timeout') return COMMON_TEXTS.errors.timeout;
  if (error.kind === 'auth' || error.isUnauthorized) return COMMON_TEXTS.errors.unauthorized;
  if (error.isNotFound) return COMMON_TEXTS.errors.notFound;
  if (error.isForbidden) return COMMON_TEXTS.errors.forbidden;
  if (error.status === 409) return COMMON_TEXTS.errors.conflict;
  if (error.status === 422) return COMMON_TEXTS.errors.validation;
  if (error.status === 429) return COMMON_TEXTS.errors.tooManyRequests;
  if (error.status >= 500) return COMMON_TEXTS.errors.server;
  return COMMON_TEXTS.errors.unexpected;
};
