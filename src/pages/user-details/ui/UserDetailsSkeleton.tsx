import styles from './UserDetailsSkeleton.module.scss';

export const UserDetailsSkeleton = () => (
  <main className={styles.page} aria-busy="true">
    <span className={styles.title} data-skeleton />
    <span className={styles.line} data-skeleton />
    <span className={styles.line} data-skeleton />
    <span className={styles.line} data-skeleton />
  </main>
);
