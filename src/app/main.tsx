import '@fontsource-variable/golos-text';
import '@fontsource-variable/jetbrains-mono';
import '@fontsource-variable/unbounded';
import './styles/global.scss';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { initPreferences } from '@/shared/lib/preferences';

import { App } from './App';

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Root element #root is missing in index.html');

initPreferences();

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
