import { Button } from '@/shared/ui/button/Button';
import { Eyebrow } from '@/shared/ui/eyebrow/Eyebrow';

import { useErrorsPage } from '../model/use-errors-page';
import { CrashChart } from './CrashChart';
import { EventFeed } from './EventFeed';
import { MinidumpList } from './MinidumpList';

import styles from './ErrorsPage.module.scss';

export const ErrorsPage = () => {
  const page = useErrorsPage();
  const { texts, overviewView: overview } = page;

  return (
    <main className={styles.page}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <Eyebrow>{texts.eyebrow}</Eyebrow>
          <h1>{texts.title}</h1>
        </div>
        <div className={styles.ranges} role="group" aria-label={texts.ranges.label}>
          {page.ranges.map((item) => (
            <Button
              type="button"
              key={item.range}
              aria-pressed={item.isActive}
              onClick={() => page.selectRange(item.range)}
            >
              {item.label}
            </Button>
          ))}
        </div>
      </header>

      {overview.status === 'loading' ? <div data-skeleton className={styles.skeleton} aria-busy="true" /> : null}
      {overview.status === 'error' ? (
        <div role="alert" className={styles.notice}>
          <p>{overview.message}</p>
          <Button type="button" onClick={page.retryOverview}>
            {page.common.actions.retry}
          </Button>
        </div>
      ) : null}
      {overview.status === 'empty' ? (
        <div className={styles.notice}>
          <h2>{texts.empty.title}</h2>
          <p>{texts.empty.hint}</p>
        </div>
      ) : null}
      {overview.status === 'ready' ? (
        <>
          <ul className={styles.kpis} aria-busy={overview.isRefreshing}>
            {overview.kpis.map((kpi) => (
              <li key={kpi.key} className={styles.kpi} data-alarming={kpi.isAlarming || undefined}>
                <span className={styles.kpiValue}>{kpi.value}</span>
                <span>{kpi.label}</span>
                <span className={styles.kpiHint}>{kpi.hint}</span>
              </li>
            ))}
          </ul>
          <CrashChart
            title={overview.chartTitle}
            axisLabel={texts.chart.axis}
            legendLabel={texts.chart.legend}
            tableLabel={texts.chart.table}
            periodLabel={texts.chart.period}
            totalLabel={texts.chart.total}
            calmLabel={texts.chart.calm}
            chart={overview.chart}
            legend={overview.legend}
            tableRows={overview.tableRows}
          />
        </>
      ) : null}

      <EventFeed
        texts={texts.feed}
        view={page.feedView}
        kindFilters={page.kindFilters}
        isAllKinds={page.isAllKinds}
        retryLabel={page.common.actions.retry}
        previousLabel={page.common.actions.previous}
        nextLabel={page.common.actions.next}
        paginationLabel={page.common.pagination.label}
        onToggleKind={page.toggleKind}
        onShowAll={page.showAllKinds}
        onPage={page.goToPage}
        onRetry={page.retryEvents}
      />

      <MinidumpList
        texts={texts.dumps}
        view={page.minidumpView}
        revealFailed={page.revealFailed}
        retryLabel={page.common.actions.retry}
        onReveal={page.revealMinidump}
        onRetry={page.retryMinidumps}
      />
    </main>
  );
};
