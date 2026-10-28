import { expect, test } from 'vitest';
import { contrast, contrastWarnings, DEFAULT_CUSTOM } from './palettes';

test('black on white is 21:1 and the default scheme passes', () => {
  expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 5);
  expect(contrastWarnings(DEFAULT_CUSTOM)).toEqual([]);
});

test('grey text on a grey background is reported', () => {
  const warnings = contrastWarnings({ ...DEFAULT_CUSTOM, '--fg': '#202020', '--bg': '#1c1c1c' });
  expect(warnings).toEqual(['Text is under 4.5:1 against the background']);
});
