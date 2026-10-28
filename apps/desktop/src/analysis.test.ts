import { describe, expect, it } from 'vitest';
import type { Analysis, Molecule } from './api';
import {
  assignCrossPeak,
  addIntegral,
  mergePeaks,
  normalizeIntegral,
  removeIntegral,
  removePeak,
  setIntegral,
  shiftAnalysis,
  setPeak,
  setPeakLabel,
  couplingsBetween,
  mergeMultiplets,
  splitMultiplet,
  classOf,
  clearAssignments,
  toggleAssignment,
} from './analysis';

const empty: Analysis = {
  integrals: [],
  reference: null,
  peaks: [],
  labels: [],
  multiplets: [],
};

describe('integrals', () => {
  it('stores ranges high ppm first', () => {
    expect(addIntegral(empty, 1.1, 1.4).integrals).toEqual([[1.4, 1.1]]);
  });

  it('does not duplicate repeated regions or move their normalisation', () => {
    const a = normalizeIntegral(addIntegral(addIntegral(empty, 4, 3), 8, 7), 1, 2);
    expect(addIntegral(a, 3, 4)).toBe(a);
    expect(addIntegral(a, 4, 3)).toBe(a);
    const repeated = [
      [4, 3],
      [7, 8],
      [3, 4],
    ].reduce((analysis, [from, to]) => addIntegral(analysis, from, to), a);
    expect(repeated).toBe(a);
    expect(repeated.reference).toEqual({ index: 1, value: 2 });
    expect(addIntegral(a, 4.001, 3).integrals).toHaveLength(3);
    expect(addIntegral(a, 4.1, 3.1).integrals).toHaveLength(3);
    expect(addIntegral({ ...a, integrals: [[3, 4]] }, 4, 3).integrals).toEqual([[3, 4]]);
  });

  it('keeps them in the order the spectrum reads, wherever they were drawn', () => {
    let a = addIntegral(addIntegral(empty, 4, 3), 8, 7);
    a = normalizeIntegral(a, 1, 2);
    a = addIntegral(a, 6, 5);
    expect(a.integrals).toEqual([
      [8, 7],
      [6, 5],
      [4, 3],
    ]);
    // The normalisation still means the integral it was set on, at 4-3 ppm.
    expect(a.reference).toEqual({ index: 2, value: 2 });
  });

  it('keeps the normalisation pointing at the same integral after removal', () => {
    let a = addIntegral(addIntegral(addIntegral(empty, 4, 3), 2, 1), 6, 5);
    // They are held as the spectrum reads them, 6-5, 4-3, 2-1, whatever order
    // they were drawn in; the normalisation here is set on 4-3.
    a = normalizeIntegral(a, 1, 3);
    // Taking away the one above it leaves it where it is, one place up.
    expect(removeIntegral(a, 0).reference).toEqual({ index: 0, value: 3 });
    // Taking away the one it names leaves nothing to point at.
    expect(removeIntegral(a, 1).reference).toBeNull();
    expect(removeIntegral(a, 2).integrals).toEqual([
      [6, 5],
      [4, 3],
    ]);
  });
});

describe('peaks', () => {
  it('merges without duplicates and sorts by ppm', () => {
    const start: Analysis = { ...empty, peaks: [7.26, 1.2], labels: [] };
    expect(mergePeaks(start, [1.2001, 3.7, 7.5], 0.01).peaks).toEqual([7.5, 7.26, 3.7, 1.2]);
  });

  it('removes a peak by index', () => {
    expect(removePeak({ ...empty, peaks: [3, 2, 1] }, 1).peaks).toEqual([3, 1]);
  });
});

describe('dragging', () => {
  const analysis: Analysis = {
    integrals: [[2, 1]],
    reference: null,
    peaks: [3, 1],
    labels: [],
    multiplets: [],
  };

  it('keeps peaks ordered when one is moved', () => {
    expect(setPeak(analysis, 1, 4).peaks).toEqual([4, 3]);
  });

  it('orders the edges of a moved integral', () => {
    expect(setIntegral(analysis, 0, 1.5, 2.5).integrals).toEqual([[2.5, 1.5]]);
  });
});

describe('labels', () => {
  const two: Analysis = {
    integrals: [],
    reference: null,
    peaks: [3, 1],
    labels: ['water', null],
    multiplets: [],
  };

  it('adds peaks not already there, highest first, unlabelled', () => {
    const merged = mergePeaks(two, [2, 3.0005], 0.01);
    expect(merged.peaks).toEqual([3, 2, 1]);
    // The label stays with the peak it was given to, not with its old position.
    expect(merged.labels).toEqual(['water', null, null]);
  });

  it('carries a label when its peak is moved past another', () => {
    const moved = setPeak(two, 0, 0.5);
    expect(moved.peaks).toEqual([1, 0.5]);
    expect(moved.labels).toEqual([null, 'water']);
  });

  it('drops the label of a peak that is removed', () => {
    expect(removePeak(two, 0).labels).toEqual([null]);
  });

  it('names a peak, and clears it again', () => {
    expect(setPeakLabel(two, 1, 'grease').labels).toEqual(['water', 'grease']);
    expect(setPeakLabel(two, 0, '').labels).toEqual(['', null]);
    expect(setPeakLabel(two, 0, null).labels).toEqual([null, null]);
  });
});

describe('referencing', () => {
  it('carries the peaks, integrals and multiplets with the axis', () => {
    const a: Analysis = {
      integrals: [[7.4, 7.2]],
      reference: null,
      peaks: [7.26, 1.56],
      labels: [null, 'water'],
      multiplets: [
        { from: 7.4, to: 7.2, ppm: 7.3, jsHz: [7.8] },
        { from: 1.6, to: 1.5 },
      ],
    };
    const moved = shiftAnalysis(a, -0.1);
    expect(moved.peaks[0]).toBeCloseTo(7.16);
    expect(moved.peaks[1]).toBeCloseTo(1.46);
    expect(moved.integrals[0][0]).toBeCloseTo(7.3);
    expect(moved.integrals[0][1]).toBeCloseTo(7.1);
    // What a peak was called is its own, and does not move with the numbers.
    expect(moved.labels).toEqual([null, 'water']);
    const [m, n] = moved.multiplets;
    expect(m.from).toBeCloseTo(7.3);
    expect(m.to).toBeCloseTo(7.1);
    expect(m.ppm).toBeCloseTo(7.2);
    // Couplings are in Hz, which referencing does not touch.
    expect(m.jsHz).toEqual([7.8]);
    // A shift read off the lines follows them by itself.
    expect(n.ppm).toBeUndefined();
    expect(n.from).toBeCloseTo(1.5);
  });
});

describe('multiplets', () => {
  it('reads couplings off the gaps between the lines picked', () => {
    // Two lines 0.013 ppm apart at 600 MHz are 7.8 Hz apart, clicked either way.
    expect(couplingsBetween([7.3, 7.313], 600)[0]).toBeCloseTo(7.8, 6);
    expect(couplingsBetween([7.313, 7.3], 600)[0]).toBeCloseTo(7.8, 6);

    // Three evenly spaced lines are two equal gaps, which is a triplet.
    const triplet = couplingsBetween([5.0, 5.0125, 5.025], 600);
    expect(triplet).toHaveLength(2);
    expect(triplet[0]).toBeCloseTo(7.5, 6);
    expect(triplet[1]).toBeCloseTo(7.5, 6);

    // One line has nothing to couple with, and two at the same shift no gap.
    expect(couplingsBetween([3.2], 600)).toEqual([]);
    expect(couplingsBetween([3.2, 3.2], 600)).toEqual([]);
  });
});

describe('multiplets', () => {
  const one: Analysis = { ...empty, multiplets: [{ from: 7.5, to: 7.3, kind: 'd', name: 'X' }] };

  it('splits one in two that read their lines again, and merges them back', () => {
    const cut = splitMultiplet(one, 0, 7.4);
    expect(cut.multiplets).toEqual([
      { from: 7.5, to: 7.4 },
      { from: 7.4, to: 7.3 },
    ]);
    expect(mergeMultiplets(cut, 0).multiplets).toEqual([{ from: 7.5, to: 7.3, name: undefined }]);
  });

  it('leaves a cut outside the range, or a last one merged, as it was', () => {
    expect(splitMultiplet(one, 0, 8)).toBe(one);
    expect(mergeMultiplets(one, 0)).toBe(one);
  });
});

describe('assignment', () => {
  // Two methyls sharing a class, and a methylene.
  const molecule: Molecule = {
    formula: 'C3H8O',
    atoms: [
      { symbol: 'C', x: 0, y: 0, charge: 0, h: 3, class: 0 },
      { symbol: 'C', x: 0, y: 0, charge: 0, h: 3, class: 0 },
      { symbol: 'C', x: 0, y: 0, charge: 0, h: 2, class: 1 },
    ],
    bonds: [],
  };
  const two = (): Analysis => ({
    ...empty,
    multiplets: [
      { from: 2, to: 1 },
      { from: 1, to: 0 },
    ],
  });

  it('picks the whole class and counts its hydrogens', () => {
    expect(classOf(molecule, 0)).toEqual([0, 1]);
    const a = toggleAssignment(two(), 0, molecule, classOf(molecule, 0));
    expect(a.multiplets[0]).toMatchObject({ atoms: [0, 1], nuclei: 6 });
  });

  it('gives a hydrogen to one signal only, and toggles off', () => {
    let a = toggleAssignment(two(), 0, molecule, [0, 1]);
    a = toggleAssignment(a, 1, molecule, [1]);
    expect(a.multiplets[0]).toMatchObject({ atoms: [0], nuclei: 3 });
    expect(a.multiplets[1]).toMatchObject({ atoms: [1], nuclei: 3 });
    a = toggleAssignment(a, 1, molecule, [1]);
    expect(a.multiplets[1]).toMatchObject({ atoms: [], nuclei: null });
  });

  it('keeps the assignments of the two halves when multiplets merge', () => {
    let a = toggleAssignment(two(), 0, molecule, [0]);
    a = toggleAssignment(a, 1, molecule, [2]);
    expect(mergeMultiplets(a, 0).multiplets[0].atoms).toEqual([0, 2]);
  });

  it('only gives when told to add, so a second click on a signal keeps them', () => {
    const a = toggleAssignment(two(), 0, molecule, [0, 1]);
    expect(toggleAssignment(a, 0, molecule, [0, 1], true).multiplets[0].atoms).toEqual([0, 1]);
    expect(toggleAssignment(a, 0, molecule, [0, 1]).multiplets[0].atoms).toEqual([]);
  });

  it('is all taken away when the structure goes', () => {
    const a = toggleAssignment(two(), 0, molecule, [0, 1]);
    expect(clearAssignments(a).multiplets[0]).toMatchObject({ atoms: [], nuclei: null });
    expect(clearAssignments(empty)).toBe(empty);
  });
});

describe('assignCrossPeak', () => {
  const peaks = [
    { at: [1.2, 20] as [number, number], atoms: [] },
    { at: [1.5, 20] as [number, number], atoms: [3] },
  ];
  it('gives atoms to one cross peak, and takes them back the second time', () => {
    const given = assignCrossPeak(peaks, 0, [3]);
    expect(given.map((p) => p.atoms)).toEqual([[3], [3]]);
    expect(assignCrossPeak(given, 0, [3])[0].atoms).toEqual([]);
    expect(assignCrossPeak(peaks, 1, [4, 5])[1].atoms).toEqual([4, 5]);
  });
});
