import { expect, it } from 'vitest';
import { integralTotal, peakFitSummary } from './quantification';

it('does not turn unavailable fitted areas into a partial total', () => {
  expect(integralTotal([1, 2, 3])).toBe(6);
  expect(integralTotal([1, null, 3])).toBeNull();
  expect(integralTotal([null])).toBeNull();
  expect(integralTotal([])).toBe(0);
});

it('reports unsuccessful fitting separately from fitted components', () => {
  expect(peakFitSummary(null)).toBe('No peak fit yet');
  expect(
    peakFitSummary({
      peaks: [
        { ppm: 2.3, height: 4, widthHz: 1, area: 0.3 },
        { ppm: 1.3, height: 2, widthHz: 1, area: 0.2 },
      ],
      regions: [
        { from: 2.4, to: 2.2, status: 'converged', rms: 0.01 },
        { from: 1.4, to: 1.2, status: 'unresolved', rms: 0.04, reason: 'insufficientSamples' },
        { from: 0.4, to: 0.2, status: 'failed', rms: null },
      ],
    }),
  ).toBe('1 fitted component · 1 unresolved region · 1 failed region · 1 insufficient sampling');
});
