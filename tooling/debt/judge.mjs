import { findKitFindings, KIT_FILES_LINE } from '../../scripts/lib/kit-files.mjs';
import { findNewErrors, pruneBaseline, toFingerprints } from '../../scripts/lib/tsc-baseline.mjs';

const sumCounts = (counts) => Object.values(counts).reduce((sum, count) => sum + count, 0);

const describeFinding = ({ file, code, message }) => `${file}: ${code} — ${message}`;

const refuseKitFindings = (inKitFiles, labels) => ({
  exitCode: 1,
  lines: [
    `${labels.file} was not written, it would freeze kit files:`,
    ...inKitFiles.map(describeFinding),
    KIT_FILES_LINE,
  ],
});

const judgeCheck = ({ findings, baseline, labels }) => {
  const fresh = findNewErrors(findings, baseline ?? {});
  const lines = fresh.map(describeFinding);
  if (baseline !== null) {
    const known = findings.length - fresh.length;
    const fixed = sumCounts(baseline) - sumCounts(pruneBaseline(findings, baseline));
    if (fixed > 0) {
      lines.push(`${fixed} fixed ${labels.noun} still counted in ${labels.file}: run pnpm ${labels.freeze}`);
    } else if (known > 0) {
      lines.push(`${known} known ${labels.noun} (${labels.file})`);
    }
    if (fresh.length > 0) lines.push(`${fresh.length} new ${labels.noun} past ${labels.file}`);
  }
  return { exitCode: fresh.length > 0 ? 1 : 0, lines };
};

const judgeBaseline = ({ findings, baseline, kitFiles, labels }) => {
  if (baseline === null) {
    const inKitFiles = findKitFindings(findings, kitFiles);
    if (inKitFiles.length > 0) return refuseKitFindings(inKitFiles, labels);
  }
  const write = baseline === null ? toFingerprints(findings) : pruneBaseline(findings, baseline);
  return { exitCode: 0, lines: [`${labels.file}: ${sumCounts(write)} known ${labels.noun}`], write };
};

const judgeSync = ({ findings, baseline, kitFiles, labels }) => {
  const inKitFiles = findKitFindings(findings, kitFiles);
  if (inKitFiles.length > 0) return refuseKitFindings(inKitFiles, labels);
  if (baseline === null) return { exitCode: 0, lines: [`no ${labels.file}: nothing to sync`] };
  const write = toFingerprints(findings);
  const added = Object.entries(write).reduce((sum, [key, count]) => sum + Math.max(0, count - (baseline[key] ?? 0)), 0);
  return { exitCode: 0, lines: [`${labels.file}: ${added} findings added by sync`], write };
};

const JUDGES = { check: judgeCheck, baseline: judgeBaseline, sync: judgeSync };

/**
 * The rule of every debt baseline (architecture.md §9): a freeze lowers, a check fails on a finding past the file,
 * a sync adds only the debt a kit update brings.
 *
 * @param {{ mode: 'check' | 'baseline' | 'sync', findings: { file: string, code: string, message: string }[], baseline: Record<string, number> | null, kitFiles: Set<string>, labels: { file: string, freeze: string, noun: string } }} options
 * @returns {{ exitCode: number, lines: string[], write?: Record<string, number> }}
 */
export const judgeDebt = (options) => {
  const judge = JUDGES[options.mode];
  if (judge === undefined) throw new Error(`unknown debt mode ${options.mode}: use check, baseline or sync`);
  return judge(options);
};
