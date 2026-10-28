import { expect, it } from 'vitest';
import type { DocumentInfo } from './api';
import { integralsCsv, peaksCsv } from './csv';

const doc = {
  name: 'ethanol, "dry" / 1',
  analysis: { integrals: [[1.4, 1.1]], reference: null, peaks: [3.7, 1.2] },
  integralValues: [3],
  peakIntensities: [10.5, 20],
} as DocumentInfo;

it('writes peaks with quoted spectrum names', () => {
  expect(peaksCsv(doc)).toBe(
    'spectrum,ppm,intensity\n"ethanol, ""dry"" / 1",3.7,10.5\n"ethanol, ""dry"" / 1",1.2,20\n',
  );
});

it('writes integrals', () => {
  expect(integralsCsv(doc).split('\n')[1]).toBe('"ethanol, ""dry"" / 1",1.4,1.1,3');
});

it('leaves unavailable fitted integrals empty instead of exporting zero', () => {
  expect(integralsCsv({ ...doc, integralValues: [null] }).split('\n')[1]).toBe(
    '"ethanol, ""dry"" / 1",1.4,1.1,',
  );
});
