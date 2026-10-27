import { Link } from '@tanstack/react-router';

import { COMMON_TEXTS } from '@/shared/config/texts';
import { useTexts } from '@/shared/model/use-preferences';

import { NOT_FOUND_TEXTS } from '../config/texts';

export const NotFoundPage = () => {
  const texts = useTexts(NOT_FOUND_TEXTS);
  const commonTexts = useTexts(COMMON_TEXTS);

  return (
    <main>
      <h1>{texts.title}</h1>
      <p>{commonTexts.errors.notFound}</p>
      <Link to="/">{texts.home}</Link>
    </main>
  );
};
