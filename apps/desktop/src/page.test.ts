import { expect, it } from 'vitest';
import { PAGE_SETUP, pixelsPerMm, plotSize, textSizesOf } from './page';

it('lays the plot out so its text prints at the size asked for', () => {
  // 11 px of text at 7 pt: 11 / 7 px a point, 2.835 points a millimetre.
  expect(pixelsPerMm(PAGE_SETUP)).toBeCloseTo((11 / 7) * (72 / 25.4), 6);
  const { width } = plotSize(PAGE_SETUP);
  expect(width).toBeCloseTo((82.5 - 4) * pixelsPerMm(PAGE_SETUP), 6);
  // Larger text is fewer pixels to the page, so everything grows with it.
  expect(pixelsPerMm({ ...PAGE_SETUP, textSize: 10 })).toBeLessThan(pixelsPerMm(PAGE_SETUP));
});

it('sizes each kind of text in points on the page', () => {
  const sizes = textSizesOf({ ...PAGE_SETUP, textSize: 8, peakSize: 6 });
  // The axis is the plot's own 11 px; the peaks, at 6 pt against 8, three quarters of that.
  expect(sizes.axis).toBe(11);
  expect(sizes.peaks).toBeCloseTo(11 * (6 / 8));
});
