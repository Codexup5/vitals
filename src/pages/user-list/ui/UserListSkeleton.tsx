import { COMMON_TEXTS } from '@/shared/config/texts';

import styles from './UserListSkeleton.module.scss';

const SKELETON_ROWS = ['first', 'second', 'third', 'fourth', 'fifth'];

export const UserListSkeleton = () => (
  <div className={styles.table} aria-busy="true">
    <span className={styles.hidden}>{COMMON_TEXTS.loading}</span>
    <span className={styles.header} data-skeleton />
    {SKELETON_ROWS.map((row) => (
      <span key={row} className={styles.row} data-skeleton />
    ))}
  </div>
);
