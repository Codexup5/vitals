import { APP_TEXTS } from '../config/texts';

interface ConfigErrorFallbackProps {
  issues: string;
}

export const ConfigErrorFallback = ({ issues }: ConfigErrorFallbackProps) => (
  <main role="alert">
    <h1>{APP_TEXTS.configTitle}</h1>
    <p>{APP_TEXTS.configHint}</p>
    {import.meta.env.DEV && <pre>{issues}</pre>}
  </main>
);
