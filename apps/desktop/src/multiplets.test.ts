import { expect, it } from 'vitest';
import type { DocumentInfo, MultipletInfo } from './api';
import {
  couplingsOf,
  describeMultiplet,
  letterOf,
  nucleiOf,
  report,
  sharedCouplings,
  simulateLines,
} from './multiplets';

const info = (over: Partial<MultipletInfo> = {}): MultipletInfo => ({
  ppm: 7.31,
  kind: 'd',
  jsHz: [7.84],
  lines: 2,
  value: 1.02,
  widthHz: 1.5,
  explained: 0.98,
  ...over,
});

it('quotes a multiplet the way a report does', () => {
  expect(describeMultiplet(info(), 1, 2)).toBe('7.31 (d, J = 7.8 Hz, 1H)');
  // A singlet has no coupling to quote.
  expect(describeMultiplet(info({ kind: 's', jsHz: [] }), 3, 2)).toBe('7.31 (s, 3H)');
  // Two couplings are both quoted, in the order they were measured.
  expect(describeMultiplet(info({ kind: 'dd', jsHz: [8.1, 1.25] }), 1, 2)).toBe(
    '7.31 (dd, J = 8.1, 1.3 Hz, 1H)',
  );
  // A signal that could not be read is quoted as the range it covers, highest
  // first, which is what a paper prints for an m.
  expect(describeMultiplet(info({ kind: 'm', jsHz: [] }), 2, 2, { from: 7.2, to: 7.42 })).toBe(
    '7.42 – 7.20 (m, 2H)',
  );
});

it('counts nuclei from the integral unless it was told otherwise', () => {
  expect(nucleiOf({ from: 7.4, to: 7.2 }, info())).toBe(1);
  expect(nucleiOf({ from: 7.4, to: 7.2 }, info({ value: 2.87 }))).toBe(3);
  expect(nucleiOf({ from: 7.4, to: 7.2, nuclei: 2 }, info({ value: 2.87 }))).toBe(2);
  // Never nothing: a multiplet that is there stands for at least one nucleus.
  expect(nucleiOf({ from: 7.4, to: 7.2 }, info({ value: 0.1 }))).toBe(1);
});

it('names them down the spectrum when they have no name', () => {
  expect([0, 1, 25, 26].map(letterOf)).toEqual(['A', 'B', 'Z', 'AA']);
});

it('writes the spectrum up as a line', () => {
  const doc = {
    nucleus: '1H',
    solvent: 'DMSO',
    frequencyMhz: 600.13,
    analysis: {
      multiplets: [
        { from: 7.4, to: 7.2 },
        { from: 2.6, to: 2.4 },
      ],
    },
    multipletInfo: [info(), info({ ppm: 2.5, kind: 't', jsHz: [7.45], value: 2.1 })],
  } as unknown as DocumentInfo;
  expect(report(doc, 2)).toBe(
    '¹H NMR (600 MHz, (CD₃)₂SO) δ 7.31 (d, J = 7.8 Hz, 1H), 2.50 (t, J = 7.5 Hz, 2H).',
  );
});

it('writes a carbon spectrum as its shifts, one place, solvent left out', () => {
  const doc = {
    nucleus: '13C',
    solvent: 'CDCl3',
    frequencyMhz: 150.9,
    analysis: {
      // The solvent's triplet, an ethyl acetate carbonyl the table names, one the
      // user renamed (so it is the sample's), a C-F doublet read as a multiplet.
      peaks: [181.23, 171.36, 171.4, 140.87, 115.52, 115.38, 77.37, 77.16, 76.95, 45.04],
      labels: [null, null, 'C=O', null, null, null, null, null, null, null],
      multiplets: [{ from: 115.6, to: 115.3 }],
    },
    multipletInfo: [info({ ppm: 115.45, kind: 'd', jsHz: [21.5] })],
  } as unknown as DocumentInfo;
  expect(report(doc, 2)).toBe(
    '¹³C NMR (151 MHz, CDCl₃) δ 181.2, 171.4, 140.9, 115.5 (d, J = 21.5 Hz), 45.0.',
  );
});

it('splits a multiplet into the lines it would have', () => {
  const obs = 600;
  // A doublet is two lines J apart, evenly.
  const d = simulateLines(7.3, 'd', [7.8], obs);
  expect(d.map((l) => l.weight)).toEqual([1, 1]);
  expect((d[0].ppm - d[1].ppm) * obs).toBeCloseTo(7.8, 6);

  // Lines that land together add up: a triplet is 1:2:1, a quartet 1:3:3:1.
  expect(simulateLines(5, 't', [7.5], obs).map((l) => l.weight)).toEqual([0.5, 1, 0.5]);
  expect(simulateLines(2, 'q', [7], obs).map((l) => l.weight)).toEqual([1 / 3, 1, 1, 1 / 3]);

  // Two unequal couplings give four lines of the same height, not a quartet.
  const dd = simulateLines(7.3, 'dd', [8, 2], obs);
  expect(dd).toHaveLength(4);
  expect(dd.map((l) => l.weight)).toEqual([1, 1, 1, 1]);

  // A singlet is one line, and nothing is invented for an m.
  expect(simulateLines(3, 's', [], obs)).toHaveLength(1);
  expect(simulateLines(3, 'm', [], obs)).toHaveLength(1);
});

it('knows how many couplings a class stands for', () => {
  expect(couplingsOf('t', [7.5])).toEqual([7.5, 7.5]);
  expect(couplingsOf('sept', [6.8])).toHaveLength(6);
  expect(couplingsOf('dd', [8, 2])).toEqual([8, 2]);
  expect(couplingsOf('s', [7])).toEqual([]);
  // A dt is one coupling once and the next twice, which a count alone cannot say.
  expect(couplingsOf('dt', [8, 2])).toEqual([8, 2, 2]);
  expect(couplingsOf('td', [7, 1.5])).toEqual([7, 7, 1.5]);
});

it('a quartet of doublets is a quartet each line of which is a doublet', () => {
  expect(couplingsOf('qd', [7.5, 1.2])).toEqual([7.5, 7.5, 7.5, 1.2]);
  // The order is what tells it from a dq: the same two numbers, split the other way.
  expect(couplingsOf('dq', [7.5, 1.2])).toEqual([7.5, 1.2, 1.2, 1.2]);
});

it('finds the couplings two multiplets share, and not those of an m', () => {
  const infos = [
    info({ kind: 'dd', jsHz: [8.3, 2.1] }),
    info({ kind: 'd', jsHz: [8.55] }),
    info({ kind: 'd', jsHz: [2.4] }),
    info({ kind: 'm', jsHz: [8.3] }),
  ];
  expect(sharedCouplings(infos, 0)).toEqual([
    { j: 0, other: 1, k: 0, hz: 8.55 },
    { j: 1, other: 2, k: 0, hz: 2.4 },
  ]);
});

it('does not invent a nucleus count when the selected area is unavailable', () => {
  const range = { from: 7.4, to: 7.2 };
  expect(nucleiOf(range, info({ value: null }))).toBeNull();
  expect(nucleiOf(range, undefined)).toBeNull();
  expect(nucleiOf({ ...range, nuclei: 2 }, info({ value: null }))).toBe(2);
  expect(describeMultiplet(info({ value: null }), nucleiOf(range, info({ value: null })), 2)).toBe(
    '7.31 (d, J = 7.8 Hz)',
  );
});
