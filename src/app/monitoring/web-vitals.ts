import type { Metric } from 'web-vitals';

import { ENV } from '@/shared/config/env';
import { reportError } from '@/shared/lib/report-error';

interface VitalMetric {
  name: Metric['name'];
  value: number;
  rating: Metric['rating'];
  id: string;
  navigationType: Metric['navigationType'];
}

type VitalsReport = (metric: VitalMetric) => void;

const toVitalMetric = ({ name, value, rating, id, navigationType }: Metric): VitalMetric => ({
  name,
  value,
  rating,
  id,
  navigationType,
});

const startWebVitals = async (report: VitalsReport) => {
  const { onCLS, onINP, onLCP, onFCP, onTTFB } = await import('web-vitals');
  const forward = (metric: Metric) => {
    report(toVitalMetric(metric));
  };
  onCLS(forward);
  onINP(forward);
  onLCP(forward);
  onFCP(forward);
  onTTFB(forward);
};

const beaconReport =
  (url: string): VitalsReport =>
  (metric) => {
    navigator.sendBeacon(url, JSON.stringify(metric));
  };

export const initWebVitals = () => {
  if (!ENV.vitalsUrl) return;
  startWebVitals(beaconReport(ENV.vitalsUrl)).catch((error: unknown) => {
    reportError(error, { scope: 'web-vitals' });
  });
};
