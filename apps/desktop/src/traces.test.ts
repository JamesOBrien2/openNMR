import { describe, expect, it } from 'vitest';
import type { DocumentInfo } from './api';
import {
  colorAt,
  fixed,
  indexOf,
  lastPpm,
  nucleusLabel,
  rowRange,
  scaleFor,
  scaleOf,
  stackOffset,
} from './traces';

const doc = { firstPpm: 12, deltaPpm: -0.01, points: 1001, min: -50, max: 20 } as DocumentInfo;

it('maps ppm to point indices and back', () => {
  expect(lastPpm(doc)).toBeCloseTo(2);
  expect(indexOf(doc, 7)).toBeCloseTo(500);
});

it('shares one scale across a set unless each is auto scaled', () => {
  const taller = { ...doc, min: -200, max: 30 } as DocumentInfo;
  expect(scaleOf(doc)).toBe(50);
  // The whole set is drawn against the tallest, so their heights stay comparable.
  expect(scaleFor(doc, [doc, taller], false)).toBe(200);
  expect(scaleFor(doc, [doc, taller], true)).toBe(50);
});

it('offsets a stack along the stacking angle', () => {
  expect(stackOffset(0, 72)).toEqual({ dx: 0, dy: 0 });
  // Straight up is the classic stack; flat lays them side by side.
  expect(stackOffset(2, 90).dy).toBeCloseTo(2.2);
  expect(stackOffset(2, 90).dx).toBeCloseTo(0);
  expect(stackOffset(2, 0).dy).toBeCloseTo(0);
  expect(stackOffset(2, 0).dx).toBeCloseTo(0.14);
});

it('gives every spectrum of a long series its own colour', () => {
  const palette = ['#111111', '#222222'];
  expect(colorAt(1, 2, palette)).toBe('#222222');
  const series = Array.from({ length: 17 }, (_, i) => colorAt(i, 17, palette));
  expect(new Set(series).size).toBe(17);
  expect(series.every((c) => /^#[0-9a-f]{6}$/.test(c))).toBe(true);
});

it('pins each baseline of a stack to its own row', () => {
  const fraction = (value: number, [y0, y1]: [number, number]) => (value - y0) / (y1 - y0);
  const rows = 4;
  // The baseline of every spectrum sits at its share of the height, whatever the gain.
  for (const gain of [0.5, 1, 4]) {
    for (let k = 0; k < rows; k++) {
      expect(fraction(0, rowRange(k, rows, 10, gain))).toBeCloseTo(k / rows);
    }
  }
  // Gain 1 fills exactly one row; doubling it fills two.
  expect(fraction(10, rowRange(0, rows, 10, 1))).toBeCloseTo(1 / rows);
  expect(fraction(10, rowRange(0, rows, 10, 2))).toBeCloseTo(2 / rows);
});

it('raises the mass number of a nucleus', () => {
  expect(nucleusLabel('1H')).toBe('\u00b9H');
  expect(nucleusLabel('13C')).toBe('\u00b9\u00b3C');
  expect(nucleusLabel('31P')).toBe('\u00b3\u00b9P');
});

describe('fixed', () => {
  it('never writes minus zero', () => {
    expect(fixed(-0.0004, 2)).toBe('0.00');
    expect(fixed(-0.006, 2)).toBe('-0.01');
    expect(fixed(7.256, 2)).toBe('7.26');
  });
});
