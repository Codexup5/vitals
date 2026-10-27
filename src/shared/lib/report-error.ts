import { invoke, isTauri } from '@tauri-apps/api/core';

type ErrorContext = Readonly<Record<string, unknown>>;

const describe = (error: unknown) => (error instanceof Error ? `${error.name}: ${error.message}` : String(error));

// No telemetry: an error stays on the PC, in the WebView console and in %APPDATA%\Vitals\logs
export const reportError = (error: unknown, context: ErrorContext = {}) => {
  console.error(error, context);
  if (!isTauri()) return;
  invoke('log_ui_error', { message: describe(error), context: JSON.stringify(context) }).catch((logError: unknown) => {
    console.error(logError);
  });
};
