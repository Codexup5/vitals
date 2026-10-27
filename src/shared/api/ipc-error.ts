/** The kinds the Rust commands report, plus what can go wrong on the way to them. */
export type IpcErrorKind = 'storage' | 'invalid' | 'io' | 'schema' | 'timeout' | 'unavailable';

interface IpcErrorInit {
  kind: IpcErrorKind;
  message: string;
  cause?: unknown;
}

export class IpcError extends Error {
  readonly kind: IpcErrorKind;

  constructor({ kind, message, cause }: IpcErrorInit) {
    super(message, { cause });
    this.name = 'IpcError';
    this.kind = kind;
  }
}

export const isIpcError = (error: unknown): error is IpcError => error instanceof IpcError;
