import { Eyebrow } from '@/shared/ui/eyebrow/Eyebrow';

import { useModulePage } from '../model/use-module-page';

import styles from './ModulePage.module.scss';

export const ModulePage = () => {
  const { eyebrow, title, note } = useModulePage();

  return (
    <main className={styles.page}>
      <header className={styles.header}>
        <Eyebrow>{eyebrow}</Eyebrow>
        <h1>{title}</h1>
      </header>
      <p className={styles.note}>{note}</p>
    </main>
  );
};
