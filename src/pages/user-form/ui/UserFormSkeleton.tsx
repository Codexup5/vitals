import styles from './UserFormSkeleton.module.scss';

const SKELETON_FIELDS = ['firstName', 'lastName', 'email', 'role'];

export const UserFormSkeleton = () => (
  <main className={styles.page} aria-busy="true">
    <span className={styles.title} data-skeleton />
    {SKELETON_FIELDS.map((field) => (
      <div key={field} className={styles.field}>
        <span className={styles.label} data-skeleton />
        <span className={styles.control} data-skeleton />
      </div>
    ))}
    <span className={styles.button} data-skeleton />
  </main>
);
