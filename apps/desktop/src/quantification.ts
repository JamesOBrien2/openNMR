import type { PeakFit } from './api';

/** A partial total would disguise an unavailable fitted area as zero. */
export const integralTotal = (values: (number | null)[]): number | null =>
  values.reduce<number | null>(
    (sum, value) => (sum === null || value === null ? null : sum + value),
    0,
  );

/** Fitting outcomes are reported by the core; the interface only counts them. */
export function peakFitSummary(fit: PeakFit | null | undefined): string {
  if (!fit) return 'No peak fit yet';
  const count = fit.peaks.filter((peak) =>
    fit.regions.some(
      (region) =>
        region.status === 'converged' &&
        peak.ppm >= Math.min(region.from, region.to) &&
        peak.ppm <= Math.max(region.from, region.to),
    ),
  ).length;
  const parts = [`${count} fitted ${count === 1 ? 'component' : 'components'}`];
  for (const status of ['unresolved', 'failed'] as const) {
    const count = fit.regions.filter((region) => region.status === status).length;
    if (count) parts.push(`${count} ${status} ${count === 1 ? 'region' : 'regions'}`);
  }
  const reasons = {
    invalidInput: 'invalid input',
    belowThreshold: 'below threshold',
    componentLimit: 'too many components',
    insufficientSamples: 'insufficient sampling',
    optimizer: 'optimizer did not converge',
    degenerate: 'ambiguous components',
    residual: 'residual too large',
  } as const;
  for (const [reason, label] of Object.entries(reasons)) {
    const count = fit.regions.filter((region) => region.reason === reason).length;
    if (count) parts.push(`${count} ${label}`);
  }
  return parts.join(' · ');
}
