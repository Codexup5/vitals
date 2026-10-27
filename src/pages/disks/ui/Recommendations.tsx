import { Button } from '@/shared/ui/button/Button';
import { Tag } from '@/shared/ui/tag/Tag';

import type { RecommendationGroup } from '../lib/cleanup-view';

import styles from './DisksPage.module.scss';

interface RecommendationsProps {
  title: string;
  revealLabel: string;
  nothingLabel: string;
  revealFailed: string | null;
  groups: readonly RecommendationGroup[];
  onReveal: (path: string) => void;
}

export const Recommendations = ({
  title,
  revealLabel,
  nothingLabel,
  revealFailed,
  groups,
  onReveal,
}: RecommendationsProps) => (
  <section className={styles.section} aria-labelledby="recommendations-title">
    <h2 id="recommendations-title">{title}</h2>
    {revealFailed ? (
      <p role="alert" className={styles.muted}>
        {revealFailed}
      </p>
    ) : null}
    <div className={styles.groups}>
      {groups.map((group) => (
        <section key={group.group} className={styles.group} data-group={group.group} aria-label={group.title}>
          <header className={styles.groupHeader}>
            <h3>{group.title}</h3>
            <Tag tone={group.group === 'safe' ? 'ok' : 'warn'}>{group.totalLabel}</Tag>
          </header>
          <p className={styles.muted}>{group.hint}</p>
          {group.rows.length === 0 ? <p className={styles.muted}>{nothingLabel}</p> : null}
          <ul className={styles.rows}>
            {group.rows.map((row) => (
              <li key={row.id} className={styles.row}>
                <div className={styles.rowMain}>
                  <span className={styles.rowTitle}>{row.title}</span>
                  <span className={`${styles.mono} ${styles.rowSize}`}>{row.sizeLabel}</span>
                </div>
                <span className={styles.muted}>{row.hint}</span>
                {row.path ? <span className={`${styles.mono} ${styles.path}`}>{row.path}</span> : null}
                {row.notes.length > 0 ? <span className={styles.muted}>{row.notes.join(' · ')}</span> : null}
                {row.path ? (
                  <Button type="button" className={styles.reveal} onClick={() => onReveal(row.path ?? '')}>
                    {revealLabel}
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  </section>
);
