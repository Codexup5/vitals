import { useQueryErrorResetBoundary } from '@tanstack/react-query';
import { useRouter, type ErrorComponentProps } from '@tanstack/react-router';

import { NotFoundPage } from '@/pages/not-found';
import { isApiError } from '@/shared/api/api-error';
import { getErrorMessage } from '@/shared/api/error-message';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { Button } from '@/shared/ui/button/Button';

export const RouteErrorFallback = ({ error, reset }: ErrorComponentProps) => {
  const router = useRouter();
  const queryErrorBoundary = useQueryErrorResetBoundary();

  const handleRetry = () => {
    queryErrorBoundary.reset();
    reset();
    void router.invalidate();
  };

  if (isApiError(error) && error.isNotFound) return <NotFoundPage />;

  return (
    <main role="alert">
      <h1>{COMMON_TEXTS.errors.screenTitle}</h1>
      <p>{getErrorMessage(error)}</p>
      <Button type="button" onClick={handleRetry}>
        {COMMON_TEXTS.actions.retry}
      </Button>
    </main>
  );
};
