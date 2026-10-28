import { expect, it } from 'vitest';
import { IMPURITIES, isSolvent, matchImpurity, suggestName } from './impurities';
import { findSolvent } from './solvents';

it('names what a peak is likely to be, in the solvent at hand', () => {
  // Water moves with the solvent, which is the whole point of the table.
  expect(matchImpurity(1.56, 'CDCl3')?.name).toBe('H₂O');
  expect(matchImpurity(3.33, '(CD3)2SO')?.name).toBe('H₂O');
  expect(matchImpurity(0.4, 'C6D6')?.name).toBe('H₂O');
  expect(matchImpurity(0.07, 'CDCl3')?.name).toBe('silicone grease');
  expect(matchImpurity(5.3, 'CDCl3')?.name).toBe('CH₂Cl₂');
});

it('stays quiet when nothing is near, or the solvent is not known', () => {
  // 0.5 ppm is clear of grease at 0.07 and hexane at 0.88.
  expect(matchImpurity(0.5, 'CDCl3')).toBeUndefined();
  expect(matchImpurity(1.56, 'CS2')).toBeUndefined();
  expect(matchImpurity(1.56, undefined)).toBeUndefined();
});

it('takes the nearest when two are close', () => {
  // Hexane CH2 at 1.26 sits beside ethanol CH3 at 1.25 in chloroform.
  expect(matchImpurity(1.262, 'CDCl3')?.shift).toBe(1.26);
  expect(matchImpurity(1.248, 'CDCl3')?.shift).toBe(1.25);
});

it('quotes a shift for every impurity it lists', () => {
  const all = Object.values(IMPURITIES).flat();
  expect(all.length).toBeGreaterThan(100);
  expect(all.every((entry) => Number.isFinite(entry.shift) && entry.name.length > 0)).toBe(true);
});

it('names the solvent itself, not only what is dissolved in it', () => {
  const dmso = findSolvent('DMSO');
  const chloroform = findSolvent('CDCl3');
  // The spelling a spectrometer writes reads better above a peak than the full
  // name, and the solvent is tried before the table, so its own peak wins.
  expect(suggestName(2.5, dmso)).toBe('DMSO');
  // Chloroform-d is listed without a short spelling, so it keeps its name.
  expect(suggestName(7.26, chloroform)).toBe('Chloroform-d');
  // Anything else still falls through to the impurities tabulated for it.
  expect(suggestName(3.33, dmso)).toBe('H₂O');
  expect(suggestName(0.5, chloroform)).toBeUndefined();
  expect(suggestName(7.26, undefined)).toBeUndefined();
});

it('reads a carbon spectrum against the carbon tables', () => {
  const chloroform = findSolvent('CDCl3', '13C');
  expect(chloroform?.shift).toBe(77.16);
  // Every line of the solvent's triplet is the solvent, at 151 MHz as at 75.
  expect(suggestName(77.37, chloroform)).toBe('Chloroform-d');
  expect(suggestName(76.95, chloroform)).toBe('Chloroform-d');
  expect(suggestName(171.36, chloroform)).toBe('EtOAc CO');
  // A proton shift means nothing on a carbon spectrum.
  expect(suggestName(1.56, chloroform)).toBeUndefined();
  // Acetone-d6 has two carbon lines, and D2O none at all.
  expect(suggestName(206.26, findSolvent('Acetone', '13C'))).toBe('Acetone');
  expect(findSolvent('D2O', '13C')).toBeUndefined();
  expect(isSolvent(40, 39, findSolvent('DMSO', '13C'))).toBe(true);
});

it('excludes the solvent without treating impurity suggestions as assignments', () => {
  const dmso = findSolvent('DMSO');
  expect(isSolvent(2.55, 2.45, dmso)).toBe(true);
  expect(suggestName(3.33, dmso)).toBe('H₂O');
  expect(isSolvent(3.4, 3.3, dmso)).toBe(false);
  // A signal of the sample is left alone.
  expect(isSolvent(8.2, 8.1, dmso)).toBe(false);
  expect(isSolvent(2.55, 2.45, undefined)).toBe(false);
});

it('does not put benzene forward: an aromatic sample is not benzene', () => {
  expect(
    Object.values(IMPURITIES)
      .flat()
      .some((e) => e.name === 'benzene'),
  ).toBe(false);
});
