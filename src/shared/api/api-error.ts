import { z } from 'zod';

export type ApiErrorKind = 'network' | 'timeout' | 'http' | 'schema' | 'auth';

export type FieldErrors = Readonly<Record<string, string>>;

interface ApiErrorInit {
  kind: ApiErrorKind;
  status: number;
  code: string;
  message: string;
  fields?: FieldErrors;
  cause?: unknown;
}

export class ApiError extends Error {
  readonly kind: ApiErrorKind;
  readonly status: number;
  readonly code: string;
  readonly fields: FieldErrors;

  constructor({ kind, status, code, message, fields = {}, cause }: ApiErrorInit) {
    super(message, { cause });
    this.name = 'ApiError';
    this.kind = kind;
    this.status = status;
    this.code = code;
    this.fields = fields;
  }

  get isNotFound() {
    return this.status === 404;
  }

  get isUnauthorized() {
    return this.status === 401;
  }

  get isForbidden() {
    return this.status === 403;
  }

  get isClientError() {
    return this.status >= 400 && this.status < 500;
  }
}

export const isApiError = (error: unknown): error is ApiError => error instanceof ApiError;

const errorBodySchema = z.object({
  code: z.string().optional(),
  message: z.string().optional(),
  fields: z.record(z.string(), z.string()).optional(),
});

export const toHttpApiError = (status: number, body: unknown) => {
  const parsed = errorBodySchema.safeParse(body);
  const details = parsed.success ? parsed.data : {};
  return new ApiError({
    kind: 'http',
    status,
    code: details.code ?? `HTTP_${status}`,
    message: details.message ?? `Request failed with status ${status}`,
    ...(details.fields && { fields: details.fields }),
  });
};
