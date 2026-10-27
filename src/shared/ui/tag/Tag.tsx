import type { ReactNode } from 'react';

import styles from './Tag.module.scss';

export type TagTone = 'accent' | 'ok' | 'warn' | 'danger';

/** A soft pill for a state or a total: it always carries its word, the tone only repeats it */
export const Tag = ({ tone = 'accent', children }: { tone?: TagTone; children: ReactNode }) => (
  <span className={styles.tag} data-tone={tone}>
    {children}
  </span>
);
