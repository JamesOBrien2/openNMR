import { describe, expect, it } from 'vitest';
import {
  contourLevels,
  covers,
  extent,
  fitY,
  fullX,
  intensityAt,
  niceTicks,
  pan,
  ppmAt,
  pxOfPpm,
  traceRequest,
  zoomAt,
  zoomBox,
  zoomX,
  zoomY,
  amend,
  historyOf,
  record,
  step,
  type Limits,
  type View,
} from './view';

const v: View = { x0: 8, x1: 4, y0: -10, y1: 90 };
const limits: Limits = { left: 12, right: -2, minSpan: 0.01 };

describe('a vertical axis in ppm', () => {
  // A 2D's F1, drawn high at the foot: y0 is the bottom edge and the higher shift.
  const f1: Limits = { ...limits, y: { left: 160, right: 0, minSpan: 0.1 } };
  const map: View = { x0: 8, x1: 4, y0: 120, y1: 40 };
  it('pans no further than the data reaches', () => {
    // Dragged down, what lies above comes into view: the low shifts, up to 0.
    const down = pan(map, 0, 10_000, 100, 100, f1);
    expect([down.y0, down.y1]).toEqual([80, 0]);
    const up = pan(map, 0, -10_000, 100, 100, f1);
    expect([up.y0, up.y1]).toEqual([160, 80]);
  });
  it('zooms out no wider than the data', () => {
    const out = zoomAt(map, 50, 50, 100, 100, 10, 'y', f1);
    expect([out.y0, out.y1]).toEqual([160, 0]);
  });
  it('leaves intensity alone when there are no vertical limits', () => {
    expect(zoomY(v, 0, 100).y1).toBe(9000);
  });
});

describe('zoom history', () => {
  const [a, b, c] = [v, { ...v, x0: 7 }, { ...v, x0: 6 }];
  it('walks back and forward, no further than it goes', () => {
    const h = record(record(historyOf(a), b), c);
    expect(step(h, -1).views[step(h, -1).at]).toBe(b);
    expect(step(h, -5).at).toBe(0);
    expect(step(h, 5).at).toBe(2);
  });
  it('drops what was stepped back over when a new zoom is made', () => {
    const h = record(step(record(record(historyOf(a), b), c), -1), a);
    expect(h.views).toEqual([a, b, a]);
    expect(h.at).toBe(2);
  });
  it('changes the one on screen in place', () => {
    const h = amend(record(historyOf(a), b), c);
    expect(h.views).toEqual([a, c]);
  });
});

describe('coordinate transforms', () => {
  it('maps pixels to ppm and intensity and back', () => {
    expect(ppmAt(v, 50, 200)).toBe(7);
    expect(pxOfPpm(v, 7, 200)).toBe(50);
    expect(intensityAt(v, 0, 100)).toBe(90);
    expect(intensityAt(v, 100, 100)).toBe(-10);
  });

  it('zooms around the anchor', () => {
    const z = zoomX(v, 7, 0.5, limits);
    expect(z.x0 - z.x1).toBe(2);
    expect(ppmAt(z, 50, 200)).toBe(7);
    expect(zoomY(v, 0, 2)).toEqual({ ...v, y0: -20, y1: 180 });
  });

  it('keeps the view inside the data', () => {
    expect(zoomX(v, 7, 100, limits)).toMatchObject({ x0: 12, x1: -2 });
    expect(pan(v, -1000, 0, 200, 100, limits)).toMatchObject({ x0: 2, x1: -2 });
    expect(zoomX(v, 6, 1e-6, limits).x0 - zoomX(v, 6, 1e-6, limits).x1).toBeCloseTo(0.01);
  });

  it('pans content with the pointer', () => {
    expect(pan(v, 50, 10, 200, 100, limits)).toEqual({ x0: 9, x1: 5, y0: 0, y1: 100 });
  });

  it('zooms to a dragged region along the chosen axis', () => {
    const box = { x0: 100, y0: 50, x1: 50, y1: 0 };
    expect(zoomBox(v, box, 200, 100, 'xy', limits)).toEqual({ x0: 7, x1: 6, y0: 40, y1: 90 });
    expect(zoomBox(v, box, 200, 100, 'x', limits)).toEqual({ ...v, x0: 7, x1: 6 });
    expect(zoomBox(v, box, 200, 100, 'y', limits)).toEqual({ ...v, y0: 40, y1: 90 });
  });

  it('zooms a lens around a click point', () => {
    const z = zoomAt(v, 50, 0, 200, 100, 0.8, 'x', limits);
    expect(z.x0 - z.x1).toBeCloseTo(3.2);
    expect(ppmAt(z, 50, 200)).toBeCloseTo(7);
    expect(z.y0).toBe(v.y0);
  });

  it('leaves the peak labels their band when fitting', () => {
    const fitted = fitY(v, -5, 40, 0.25);
    // The highest point lands exactly under the band, whatever the intensities.
    expect((fitted.y1 - 40) / (fitted.y1 - fitted.y0)).toBeCloseTo(0.25);
    expect(fitted.y0).toBeCloseTo(fitY(v, -5, 40).y0);
    // An unlabelled spectrum keeps the whole plot.
    expect(fitY(v, -5, 40, 0)).toEqual(fitY(v, -5, 40));
    // A label band can never take more than half of it.
    expect((fitY(v, -5, 40, 0.9).y1 - 40) / (fitY(v, -5, 40, 0.9).y1 - fitted.y0)).toBeCloseTo(0.5);
  });

  it('shows the full spectrum and fits intensity to visible points', () => {
    expect(fullX(v, limits)).toEqual({ ...v, x0: 12, x1: -2 });
    const xy = new Float32Array([-10, 500, 10, -5, 20, 40, 30, 10]);
    const { lo, hi } = extent(xy, 100, 100, 300);
    expect([lo, hi]).toEqual([-5, 40]);
    // The baseline keeps a wider margin below it than the peaks get above.
    const fitted = fitY(v, lo, hi);
    expect(fitted.y0).toBeCloseTo(-14.9);
    expect(fitted.y1).toBeCloseTo(42.25);
  });
});

describe('niceTicks', () => {
  it('uses 1-2-5 steps and handles reversed ranges', () => {
    expect(niceTicks(10, -0.5, 5)).toEqual({ values: [0, 5, 10], decimals: 0 });
    expect(niceTicks(1.23, 1.31, 4).values).toEqual([1.24, 1.26, 1.28, 1.3]);
  });

  it('returns nothing for empty ranges', () => {
    expect(niceTicks(1, 1, 5).values).toEqual([]);
  });
});

describe('trace requests', () => {
  it('pads one screen each side and scales columns', () => {
    expect(traceRequest(100, 300, 1000, 400)).toEqual({ start: 0, end: 501, columns: 1002 });
  });

  it('is empty when the view is outside the data', () => {
    const r = traceRequest(2000, 2100, 1000, 400);
    expect(r.end - r.start).toBe(0);
  });

  it('reuses data while the resolution is similar', () => {
    const have = traceRequest(0, 10000, 100000, 1000);
    expect(covers(have, 0, 11000, traceRequest(0, 11000, 100000, 1000))).toBe(true);
    expect(covers(have, 0, 2000, traceRequest(0, 2000, 100000, 1000))).toBe(false);
  });
});

it('puts the baseline where it was asked to sit', () => {
  const v = { x0: 10, x1: 0, y0: 0, y1: 1 };
  const fitted = fitY(v, -1, 40, 0.25, 0.2);
  const at = (value: number) => (value - fitted.y0) / (fitted.y1 - fitted.y0);
  expect(at(0)).toBeCloseTo(0.2);
  // The highest line still stands just under the labels.
  expect(at(40)).toBeCloseTo(0.75);
});

describe('contourLevels', () => {
  it('climbs geometrically and stops at the top of the data', () => {
    expect(contourLevels(1, 10, 2, 20)).toEqual([1, 2, 4, 8]);
    expect(contourLevels(1, 1e9, 2, 3)).toEqual([1, 2, 4]);
    expect(contourLevels(20, 10, 2, 20)).toEqual([]);
  });
});
