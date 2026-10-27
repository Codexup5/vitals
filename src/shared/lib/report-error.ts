type ErrorContext = Readonly<Record<string, unknown>>;

// No telemetry: an error stays on the PC, in the WebView console; the log file in %APPDATA%\Vitals comes with stage 1
export const reportError = (error: unknown, context: ErrorContext = {}) => {
  console.error(error, context);
};
