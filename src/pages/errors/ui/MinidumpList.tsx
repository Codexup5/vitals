import { Button } from '@/shared/ui/button/Button';

import type { MinidumpView } from '../lib/minidump-view';

import styles from './EventFeed.module.scss';

interface MinidumpTexts {
  title: string;
  folder: string;
  name: string;
  time: string;
  code: string;
  size: string;
  reveal: string;
}

interface MinidumpListProps {
  texts: MinidumpTexts;
  view: MinidumpView;
  revealFailed: string | null;
  retryLabel: string;
  onReveal: (name: string) => void;
  onRetry: () => void;
}

export const MinidumpList = ({ texts, view, revealFailed, retryLabel, onReveal, onRetry }: MinidumpListProps) => (
  <section className={styles.feed} aria-labelledby="minidumps-title" aria-busy={view.status === 'loading'}>
    <h2 id="minidumps-title">{texts.title}</h2>
    <p className={`${styles.found} ${styles.mono}`}>{texts.folder}</p>
    {revealFailed ? (
      <p role="alert" className={styles.message}>
        {revealFailed}
      </p>
    ) : null}
    {view.status === 'error' ? (
      <div role="alert" className={styles.message}>
        <p>{view.message}</p>
        <Button type="button" onClick={onRetry}>
          {retryLabel}
        </Button>
      </div>
    ) : null}
    {view.status === 'empty' ? <p className={styles.message}>{view.message}</p> : null}
    {view.status === 'ready' ? (
      <div className={styles.scroll}>
        <table>
          <thead>
            <tr>
              <th scope="col">{texts.name}</th>
              <th scope="col">{texts.time}</th>
              <th scope="col">{texts.code}</th>
              <th scope="col">{texts.size}</th>
              <th scope="col">
                <span className="visually-hidden">{texts.reveal}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {view.rows.map((row) => (
              <tr key={row.name}>
                <td className={styles.mono}>{row.name}</td>
                <td className={styles.mono}>{row.timeLabel}</td>
                <td className={styles.mono}>{row.code}</td>
                <td className={styles.mono}>{row.sizeLabel}</td>
                <td>
                  <Button type="button" onClick={() => onReveal(row.name)}>
                    {texts.reveal}
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    ) : null}
  </section>
);
