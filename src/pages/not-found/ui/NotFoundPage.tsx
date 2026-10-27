import { Link } from '@tanstack/react-router';

import { COMMON_TEXTS } from '@/shared/config/texts';

import { NOT_FOUND_TEXTS } from '../config/texts';

export const NotFoundPage = () => (
  <main>
    <h1>{NOT_FOUND_TEXTS.title}</h1>
    <p>{COMMON_TEXTS.errors.notFound}</p>
    <Link to="/">{NOT_FOUND_TEXTS.home}</Link>
  </main>
);
