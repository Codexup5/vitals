import './styles/global.scss';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { ENV, ENV_ISSUES } from '@/shared/config/env';

import { App } from './App';
import { ConfigErrorFallback } from './errors/ConfigErrorFallback';
import { initMonitoring } from './monitoring/sentry';
import { initWebVitals } from './monitoring/web-vitals';

const startApiMocks = async () => {
  if (!import.meta.env.DEV || !ENV.shouldMockApi) return;
  const { worker } = await import('../../mocks/browser');
  await worker.start({ onUnhandledRequest: 'bypass' });
};

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Root element #root is missing in index.html');

const root = createRoot(rootElement);

if (ENV_ISSUES) {
  root.render(
    <StrictMode>
      <ConfigErrorFallback issues={ENV_ISSUES} />
    </StrictMode>,
  );
} else {
  initMonitoring();
  initWebVitals();
  await startApiMocks();

  root.render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
