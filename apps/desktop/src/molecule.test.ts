import { describe, expect, it } from 'vitest';
import type { Molecule } from './api';
import { layout, looksLikeMolfile, molfileText, molfileUrl } from './molecule';

const ethanol: Molecule = {
  formula: 'C2H6O',
  atoms: [
    { symbol: 'C', x: 0, y: 0, charge: 0, h: 3, class: 0 },
    { symbol: 'C', x: 2, y: 1, charge: 0, h: 2, class: 1 },
    { symbol: 'O', x: 4, y: 0, charge: 0, h: 1, class: 2 },
  ],
  bonds: [
    { a: 0, b: 1, order: 1, stereo: 0 },
    { a: 1, b: 2, order: 1, stereo: 0 },
  ],
};

describe('molfile as a data URL', () => {
  it('comes back as it went in', () => {
    const text = '\n  x\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\nM  END\n';
    expect(molfileText(molfileUrl(text))).toBe(text);
    expect(molfileText('data:image/png;base64,AAAA')).toBeUndefined();
  });

  it('is told from other text by its counts line', () => {
    expect(looksLikeMolfile('\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n')).toBe(true);
    expect(looksLikeMolfile('CCO')).toBe(false);
  });
});

describe('layout', () => {
  it('measures in bond lengths and flips y', () => {
    const d = layout(ethanol);
    expect(d.unit).toBeCloseTo(Math.hypot(2, 1));
    expect(d.points[1].y).toBeLessThan(d.points[0].y);
  });
});
