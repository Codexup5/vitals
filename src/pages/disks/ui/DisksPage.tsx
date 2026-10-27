import type { CSSProperties } from 'react';

import { Button } from '@/shared/ui/button/Button';

import { useDisksPage } from '../model/use-disks-page';
import { Recommendations } from './Recommendations';
import { Treemap } from './Treemap';

import styles from './DisksPage.module.scss';

export const DisksPage = () => {
  const page = useDisksPage();
  const { texts, view } = page;

  return (
    <main className={styles.page}>
      <header className={styles.header}>
        <h1>{texts.title}</h1>
        <div className={styles.scan}>
          <Button type="button" onClick={page.startScan} disabled={page.isScanning} aria-busy={page.isScanning}>
            {texts.scan.start}
          </Button>
          {view.status === 'ready' ? (
            <p className={styles.muted} role="status">
              {view.scanLabel}
            </p>
          ) : null}
        </div>
      </header>
      {page.scanFailed ? <p role="alert">{page.scanFailed}</p> : null}

      {view.status === 'loading' ? <div data-skeleton className={styles.skeleton} aria-busy="true" /> : null}
      {view.status === 'error' ? (
        <div role="alert" className={styles.notice}>
          <p>{view.message}</p>
          <Button type="button" onClick={page.retry}>
            {page.common.actions.retry}
          </Button>
        </div>
      ) : null}

      {view.status === 'ready' ? (
        <>
          <ul className={styles.kpis}>
            {view.kpis.map((kpi) => (
              <li key={kpi.key} className={styles.kpi}>
                <span className={styles.kpiValue}>{kpi.value}</span>
                <span>{kpi.label}</span>
                {kpi.hint ? <span className={styles.muted}>{kpi.hint}</span> : null}
              </li>
            ))}
          </ul>

          <section className={styles.section} aria-labelledby="volumes-title">
            <h2 id="volumes-title">{texts.volumes.title}</h2>
            <ul className={styles.volumes}>
              {view.volumes.map((volume) => (
                <li key={volume.letter} className={styles.volume}>
                  <span className={styles.volumeName}>{volume.name}</span>
                  <span
                    className={styles.bar}
                    role="meter"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={volume.usedPercent}
                    aria-label={volume.name}
                    style={{ '--used': `${volume.usedPercent}%` } as CSSProperties}
                  />
                  <span className={styles.mono}>{volume.usedLabel}</span>
                  <span className={styles.muted}>{volume.freeLabel}</span>
                </li>
              ))}
            </ul>
          </section>

          <section className={styles.section} aria-labelledby="drives-title">
            <h2 id="drives-title">{texts.drives.title}</h2>
            <div className={styles.scroll}>
              <table className={styles.table}>
                <thead>
                  <tr>
                    <th scope="col">{texts.drives.name}</th>
                    <th scope="col">{texts.drives.health}</th>
                    <th scope="col">{texts.drives.temperature}</th>
                    <th scope="col">{texts.drives.wear}</th>
                    <th scope="col">{texts.drives.hours}</th>
                  </tr>
                </thead>
                <tbody>
                  {view.drives.map((drive) => (
                    <tr key={drive.key}>
                      <td>{drive.name}</td>
                      <td>
                        <span className={styles.health} data-health={drive.health}>
                          {drive.healthLabel}
                        </span>
                      </td>
                      <td className={styles.mono}>{drive.temperature}</td>
                      <td className={styles.mono}>{drive.wear}</td>
                      <td className={styles.mono}>{drive.hours}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          {page.treemap ? (
            <section className={styles.section} aria-labelledby="treemap-title">
              <header className={styles.groupHeader}>
                <h2 id="treemap-title">{texts.treemap.title}</h2>
                <div className={styles.modes} role="group" aria-label={texts.treemap.modes}>
                  <Button
                    type="button"
                    aria-pressed={page.mode === 'category'}
                    onClick={() => page.selectMode('category')}
                  >
                    {texts.treemap.byCategory}
                  </Button>
                  <Button
                    type="button"
                    aria-pressed={page.mode === 'cleanup'}
                    onClick={() => page.selectMode('cleanup')}
                  >
                    {texts.treemap.byCleanup}
                  </Button>
                </div>
              </header>
              <Treemap
                title={texts.treemap.title}
                description={view.scanLabel}
                legendLabel={texts.treemap.legend}
                tiles={page.treemap.tiles}
                legend={page.treemap.legend}
              />
            </section>
          ) : null}

          {view.hasScan ? (
            <Recommendations
              title={texts.recommendations.title}
              revealLabel={texts.recommendations.reveal}
              nothingLabel={texts.recommendations.nothing}
              revealFailed={page.revealFailed}
              groups={view.recommendations}
              onReveal={page.reveal}
            />
          ) : null}
        </>
      ) : null}
    </main>
  );
};
