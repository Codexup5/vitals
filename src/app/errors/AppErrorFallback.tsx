import { COMMON_TEXTS } from '@/shared/config/texts';
import { Button } from '@/shared/ui/button/Button';

import { APP_TEXTS } from '../config/texts';

export const AppErrorFallback = () => (
  <main role="alert">
    <h1>{APP_TEXTS.fatalTitle}</h1>
    <p>{COMMON_TEXTS.errors.unexpected}</p>
    <Button type="button" onClick={() => window.location.reload()}>
      {APP_TEXTS.reload}
    </Button>
  </main>
);
