import type { EventKind } from '@/entities/health';
import { Button } from '@/shared/ui/button/Button';

import type { FeedView } from '../lib/event-feed-view';

import styles from './EventFeed.module.scss';

interface KindFilter {
  kind: EventKind;
  label: string;
  isActive: boolean;
}

interface EventFeedTexts {
  title: string;
  filter: string;
  all: string;
  time: string;
  kind: string;
  source: string;
  detail: string;
}

interface EventFeedProps {
  texts: EventFeedTexts;
  view: FeedView;
  kindFilters: readonly KindFilter[];
  isAllKinds: boolean;
  retryLabel: string;
  previousLabel: string;
  nextLabel: string;
  paginationLabel: string;
  onToggleKind: (kind: EventKind) => void;
  onShowAll: () => void;
  onPage: (page: number) => void;
  onRetry: () => void;
}

export const EventFeed = ({
  texts,
  view,
  kindFilters,
  isAllKinds,
  retryLabel,
  previousLabel,
  nextLabel,
  paginationLabel,
  onToggleKind,
  onShowAll,
  onPage,
  onRetry,
}: EventFeedProps) => (
  <section className={styles.feed} aria-labelledby="event-feed-title" aria-busy={view.status === 'loading'}>
    <h2 id="event-feed-title">{texts.title}</h2>
    <div className={styles.filters} role="group" aria-label={texts.filter}>
      <Button type="button" aria-pressed={isAllKinds} onClick={onShowAll}>
        {texts.all}
      </Button>
      {kindFilters.map((filter) => (
        <Button
          type="button"
          key={filter.kind}
          aria-pressed={filter.isActive}
          onClick={() => onToggleKind(filter.kind)}
        >
          {filter.label}
        </Button>
      ))}
    </div>
    {view.status === 'loading' ? <div data-skeleton className={styles.skeleton} /> : null}
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
      <>
        <p className={styles.found}>{view.found}</p>
        <div className={styles.scroll}>
          <table className={styles.table} aria-busy={view.isRefreshing}>
            <thead>
              <tr>
                <th scope="col">{texts.time}</th>
                <th scope="col">{texts.kind}</th>
                <th scope="col">{texts.source}</th>
                <th scope="col">{texts.detail}</th>
              </tr>
            </thead>
            <tbody>
              {view.rows.map((row) => (
                <tr key={row.id}>
                  <td className={styles.mono}>{row.timeLabel}</td>
                  <td>
                    <span className={styles.kind} data-kind={row.chartKind ?? undefined}>
                      {row.kindLabel}
                    </span>
                  </td>
                  <td className={styles.mono}>{row.source}</td>
                  <td className={styles.mono}>{row.detail}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <nav className={styles.pages} aria-label={paginationLabel}>
          <Button type="button" disabled={view.page <= 1} onClick={() => onPage(view.page - 1)}>
            {previousLabel}
          </Button>
          <span className={styles.mono}>{view.position}</span>
          <Button type="button" disabled={view.page >= view.pageCount} onClick={() => onPage(view.page + 1)}>
            {nextLabel}
          </Button>
        </nav>
      </>
    ) : null}
  </section>
);
