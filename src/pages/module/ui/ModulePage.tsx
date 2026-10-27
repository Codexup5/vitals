import { useModulePage } from '../model/use-module-page';

import styles from './ModulePage.module.scss';

export const ModulePage = () => {
  const { title, note } = useModulePage();

  return (
    <main className={styles.page}>
      <h1>{title}</h1>
      <p className={styles.note}>{note}</p>
    </main>
  );
};
