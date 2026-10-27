import { COMMON_TEXTS } from '@/shared/config/texts';
import { useTexts } from '@/shared/model/use-preferences';
import { Button } from '@/shared/ui/button/Button';

import { APP_TEXTS } from '../config/texts';

export const AppErrorFallback = () => {
  const texts = useTexts(APP_TEXTS);
  const commonTexts = useTexts(COMMON_TEXTS);

  return (
    <main role="alert">
      <h1>{texts.fatalTitle}</h1>
      <p>{commonTexts.errors.unexpected}</p>
      <Button type="button" onClick={() => window.location.reload()}>
        {texts.reload}
      </Button>
    </main>
  );
};
