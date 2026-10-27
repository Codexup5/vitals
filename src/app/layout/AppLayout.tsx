import { Link, Outlet } from '@tanstack/react-router';

import { MODULE_IDS } from '@/shared/config/modules';
import { MODULE_TEXTS } from '@/shared/config/modules-texts';
import { usePreferences, useTexts } from '@/shared/model/use-preferences';
import { Button } from '@/shared/ui/button/Button';

import { APP_TEXTS } from '../config/texts';

import styles from './AppLayout.module.scss';

export const AppLayout = () => {
  const { locale, theme, updatePreferences } = usePreferences();
  const texts = useTexts(APP_TEXTS);
  const moduleTexts = useTexts(MODULE_TEXTS);

  return (
    <div className={styles.layout}>
      <aside className={styles.sidebar}>
        <p className={styles.brand}>{texts.brand}</p>
        <nav aria-label={texts.modules}>
          <ul className={styles.nav}>
            {MODULE_IDS.map((moduleId) => (
              <li key={moduleId}>
                <Link to="/$moduleId" params={{ moduleId }} className={styles.link}>
                  {moduleTexts[moduleId]}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
        <section className={styles.preferences} aria-label={texts.preferences}>
          <Button
            type="button"
            lang={locale === 'ru' ? 'en' : 'ru'}
            aria-label={texts.switchLocaleLabel}
            onClick={() => updatePreferences({ locale: locale === 'ru' ? 'en' : 'ru' })}
          >
            {texts.switchLocale}
          </Button>
          <Button type="button" onClick={() => updatePreferences({ theme: theme === 'dark' ? 'light' : 'dark' })}>
            {theme === 'dark' ? texts.themeLight : texts.themeDark}
          </Button>
        </section>
      </aside>
      <div className={styles.content}>
        <Outlet />
      </div>
    </div>
  );
};
