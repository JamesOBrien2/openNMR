import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { DocumentInfo, Opened } from '../api';

beforeEach(() => {
  vi.resetModules();
  vi.stubGlobal('location', { search: '?fixture=deconvolution' });
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => [] }));
});
afterEach(() => vi.unstubAllGlobals());

async function fixture() {
  const { handle } = await import('./backend');
  const opened = (await handle('open', { path: '/demo/deconvolution', baseline: true })) as Opened;
  return { handle, doc: opened.documents[0] };
}

it('demonstrates overlapping components and a failed fit without a sum fallback', async () => {
  const { handle, doc } = await fixture();
  expect(doc.integralValues).toEqual([null, null]);
  const fitted = handle('fit_peaks', { id: doc.id, threshold: 0.01 }) as DocumentInfo;
  expect(fitted.analysis.peaks).toEqual(doc.analysis.peaks);
  expect(fitted.analysis.peakFit?.peaks.map((peak) => peak.ppm)).toEqual([6.1, 6.096]);
  expect(fitted.analysis.peakFit?.regions.map((region) => region.status)).toEqual([
    'converged',
    'converged',
    'failed',
  ]);
  expect(fitted.integralValues).toEqual([1, null]);
  const summed = handle('set_analysis', {
    id: doc.id,
    analysis: { ...fitted.analysis, integrationMethod: 'sum' },
  }) as DocumentInfo;
  expect(summed.integralValues.every((value) => value !== null)).toBe(true);
  const restored = handle('undo', { id: doc.id }) as DocumentInfo;
  expect(restored.analysis.integrationMethod).toBe('fitted');
  expect(restored.integralValues).toEqual([1, null]);
});

it('refits only accepted peaks and invalidates a fit after a manual edit', async () => {
  const { handle, doc } = await fixture();
  const fitted = handle('fit_peaks', { id: doc.id, threshold: 0.01 }) as DocumentInfo;
  const edited = handle('set_analysis', {
    id: doc.id,
    analysis: { ...fitted.analysis, peaks: [6.1], labels: ['kept'] },
  }) as DocumentInfo;
  expect(edited.analysis.peakFit).toBeNull();
  const refitted = handle('fit_peaks', { id: doc.id, threshold: 0.01 }) as DocumentInfo;
  expect(refitted.analysis.peaks).toEqual([6.1]);
  expect(refitted.analysis.labels).toEqual(['kept']);
  expect(refitted.analysis.peakFit?.peaks).toHaveLength(1);
  const regions = handle('signal_regions', {
    id: doc.id,
    from: 10,
    to: 0,
    threshold: 1e10,
  }) as [number, number][];
  expect(regions).toHaveLength(1);
});

it('keeps automatic pick, fit and integration together for one undo', async () => {
  const { handle } = await import('./backend');
  const opened = (await handle('open', { path: '/demo/plain', baseline: true })) as Opened;
  const original = opened.documents[0];
  const picked = handle('auto_peaks', {
    id: original.id,
    threshold: 0.05,
    method: 'standard',
  }) as DocumentInfo;
  const fitted = handle('fit_peaks', {
    id: original.id,
    threshold: 0.05,
    amend: true,
  }) as DocumentInfo;
  expect(fitted.analysis.peaks).toEqual(picked.analysis.peaks);
  handle('set_analysis', {
    id: original.id,
    analysis: { ...fitted.analysis, integrals: [[6.2, 6]] },
    amend: true,
  });
  const undone = handle('undo', { id: original.id }) as DocumentInfo;
  expect(undone.analysis).toEqual(original.analysis);
});

it('exposes overlap discovery only through the explicit automatic method', async () => {
  const { handle, doc } = await fixture();
  const picked = handle('auto_peaks', {
    id: doc.id,
    threshold: 0.05,
    method: 'deconvolution',
  }) as DocumentInfo;
  expect(picked.analysis.peaks).toEqual(expect.arrayContaining([6.1, 6.096]));
  expect(picked.analysis.peakFit?.peaks).toHaveLength(2);
  const belowThreshold = handle('auto_peaks', {
    id: doc.id,
    threshold: 100,
    method: 'deconvolution',
  }) as DocumentInfo;
  expect(belowThreshold.analysis.peaks).toEqual([]);
});
