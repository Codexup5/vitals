import { useQueryErrorResetBoundary } from '@tanstack/react-query';
import { useRouter, type ErrorComponentProps } from '@tanstack/react-router';

import { COMMON_TEXTS } from '@/shared/config/texts';
import { useTexts } from '@/shared/model/use-preferences';
import { Button } from '@/shared/ui/button/Button';

export const RouteErrorFallback = ({ reset }: ErrorComponentProps) => {
  const router = useRouter();
  const queryErrorBoundary = useQueryErrorResetBoundary();
  const texts = useTexts(COMMON_TEXTS);

  const handleRetry = () => {
    queryErrorBoundary.reset();
    reset();
    void router.invalidate();
  };

  return (
    <main role="alert">
      <h1>{texts.errors.screenTitle}</h1>
      <p>{texts.errors.unexpected}</p>
      <Button type="button" onClick={handleRetry}>
        {texts.actions.retry}
      </Button>
    </main>
  );
};
