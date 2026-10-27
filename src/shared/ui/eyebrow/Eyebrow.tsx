import type { ReactNode } from 'react';

import styles from './Eyebrow.module.scss';

/** The mono label above a heading: what the section is and where its data comes from */
export const Eyebrow = ({ children }: { children: ReactNode }) => <p className={styles.eyebrow}>{children}</p>;
