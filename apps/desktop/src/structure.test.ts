import { describe, expect, it } from 'vitest';
import type { Molecule } from './api';
import { ACS_1996, draw, labelled, type Item } from './structure';

const atom = (symbol: string, x: number, y: number, h = 0, charge = 0) => ({
  symbol,
  x,
  y,
  charge,
  h,
  class: 0,
});
const bond = (a: number, b: number, order = 1, stereo = 0) => ({ a, b, order, stereo });

/** A hexagon of side 1 with a methyl on its first corner: toluene as Kekulé draws it. */
const ring = (): Molecule => {
  const corners = Array.from({ length: 6 }, (_, k) => {
    const angle = Math.PI / 2 + (k * Math.PI) / 3;
    return atom('C', Math.cos(angle), Math.sin(angle), k === 0 ? 0 : 1);
  });
  return {
    formula: 'C7H8',
    atoms: [...corners, atom('C', 0, 2, 3)],
    bonds: [
      ...Array.from({ length: 6 }, (_, k) => bond(k, (k + 1) % 6, k % 2 === 0 ? 2 : 1)),
      bond(0, 6),
    ],
  };
};

const lines = (items: Item[]) => items.filter((i) => i.kind === 'line');
const texts = (items: Item[]) => items.filter((i) => i.kind === 'text');

describe('draw', () => {
  it('puts a ring double bond inside the ring, shorter than the bond it stands beside', () => {
    const molecule = ring();
    const { items, points } = draw(molecule, ACS_1996);
    // Bond 2-3 is a double one away from the methyl; the lines nearest its middle.
    const mid = { x: (points[2].x + points[3].x) / 2, y: (points[2].y + points[3].y) / 2 };
    const near = lines(items).filter((l) => {
      const cx = (l.x1 + l.x2) / 2;
      const cy = (l.y1 + l.y2) / 2;
      return Math.hypot(cx - mid.x, cy - mid.y) < 0.3;
    });
    expect(near).toHaveLength(2);
    const [outer, inner] = near.sort(
      (p, q) => Math.hypot(q.x1 - q.x2, q.y1 - q.y2) - Math.hypot(p.x1 - p.x2, p.y1 - p.y2),
    );
    // The ring's centre is the origin: the inner line is the one nearer it.
    const away = (l: (typeof near)[0]) => Math.hypot((l.x1 + l.x2) / 2, (l.y1 + l.y2) / 2);
    expect(away(inner)).toBeLessThan(away(outer));
    expect(Math.hypot(inner.x1 - inner.x2, inner.y1 - inner.y2)).toBeLessThan(
      Math.hypot(outer.x1 - outer.x2, outer.y1 - outer.y2),
    );
  });

  it('writes only the heteroatoms, with the hydrogens away from the bonds', () => {
    const ethanol: Molecule = {
      formula: 'C2H6O',
      atoms: [atom('C', 0, 0, 3), atom('C', 1, 0.5, 2), atom('O', 2, 0, 1)],
      bonds: [bond(0, 1), bond(1, 2)],
    };
    expect([0, 1, 2].map((i) => labelled(ethanol, i, ACS_1996))).toEqual([false, false, true]);
    const words = texts(draw(ethanol, ACS_1996).items).map((t) => t.text);
    expect(words).toEqual(['O', 'H']);
    // The bond comes from the left, so the H is written on the right: "OH".
    const [o, h] = texts(draw(ethanol, ACS_1996).items);
    expect(h.x).toBeGreaterThan(o.x);
    // Bonds from the right put it on the left: "HO".
    const mirrored = { ...ethanol, atoms: ethanol.atoms.map((a) => ({ ...a, x: -a.x })) };
    const [o2, h2] = texts(draw(mirrored, ACS_1996).items);
    expect(h2.x).toBeLessThan(o2.x);
  });

  it('puts the hydrogen of a ring nitrogen below it, under the symbol, and takes it from there', () => {
    // Both bonds go up, as the NH of a carbazole or a pyrrole is drawn.
    const molecule: Molecule = {
      formula: 'C2H3N',
      atoms: [atom('N', 0, 0, 1), atom('C', -0.8, 0.6, 1), atom('C', 0.8, 0.6, 1)],
      bonds: [bond(0, 1), bond(0, 2)],
    };
    const { items, hits } = draw(molecule, ACS_1996);
    const [n, h] = texts(items);
    expect(n.text + h.text).toBe('NH');
    expect(h.y).toBeGreaterThan(n.y);
    expect(h.x).toBeCloseTo(n.x);
    // The click lands on the H that is written, not on the N above it.
    expect(hits[0].y).toBeCloseTo(h.y);
  });

  it('counts hydrogens as a subscript after the H', () => {
    const amine: Molecule = {
      formula: 'CH5N',
      atoms: [atom('C', 0, 0, 3), atom('N', 1, 0, 2)],
      bonds: [bond(0, 1)],
    };
    expect(texts(draw(amine, ACS_1996).items).map((t) => t.text)).toEqual(['N', 'H', '2']);
  });

  it('draws a hashed wedge as strokes across the bond that widen', () => {
    const molecule: Molecule = {
      formula: 'C2H6',
      atoms: [atom('C', 0, 0, 3), atom('C', 1, 0, 3)],
      bonds: [bond(0, 1, 1, 6)],
    };
    const strokes = lines(draw(molecule, ACS_1996).items);
    expect(strokes.length).toBeGreaterThan(4);
    const width = (l: (typeof strokes)[0]) => Math.hypot(l.x1 - l.x2, l.y1 - l.y2);
    expect(width(strokes[strokes.length - 1])).toBeGreaterThan(width(strokes[0]));
  });

  it('stops a bond short of the symbol it ends at', () => {
    const molecule: Molecule = {
      formula: 'CH4O',
      atoms: [atom('C', 0, 0, 3), atom('O', 1, 0, 1)],
      bonds: [bond(0, 1)],
    };
    const [l] = lines(draw(molecule, ACS_1996).items);
    expect(l.x2).toBeLessThan(1);
  });

  it('writes the hydrogens of a terminal carbon when asked', () => {
    const molecule: Molecule = {
      formula: 'C2H6',
      atoms: [atom('C', 0, 0, 3), atom('C', 1, 0, 3)],
      bonds: [bond(0, 1)],
    };
    expect(texts(draw(molecule, ACS_1996).items)).toHaveLength(0);
    const words = texts(draw(molecule, { ...ACS_1996, terminalCarbons: true }).items);
    // The left one has its bond on the right, so its hydrogens stand on the left.
    expect(words.map((t) => t.text)).toEqual(['C', '3', 'H', 'C', 'H', '3']);
  });
});
