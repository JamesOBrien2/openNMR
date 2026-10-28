import { expect, it } from 'vitest';
import { findSolvent, formatSolvent, solventShift, SOLVENTS } from './solvents';

it('finds the solvent a dataset names, however it is spelled', () => {
  expect(solventShift('CDCl3')).toBe(7.26);
  expect(solventShift('cdcl3')).toBe(7.26);
  // The datasets here carry the short form.
  expect(solventShift('DMSO')).toBe(2.5);
  expect(solventShift('Acetone')).toBe(2.05);
  expect(findSolvent('C6D6')?.name).toBe('Benzene-d6');
  expect(solventShift(null)).toBeUndefined();
  expect(solventShift('CS2')).toBeUndefined();
});

it('lowers the counts in a formula', () => {
  expect(formatSolvent('CDCl3')).toBe('CDCl₃');
  expect(formatSolvent('C6D6')).toBe('C₆D₆');
  expect(formatSolvent('(CD3)2SO')).toBe('(CD₃)₂SO');
  expect(formatSolvent('Si(CH3)4')).toBe('Si(CH₃)₄');
});

it('quotes a shift for every solvent it lists', () => {
  expect(SOLVENTS.every((s) => Number.isFinite(s.shift))).toBe(true);
  expect(SOLVENTS.find((s) => s.formula === 'D2O')?.shift).toBe(4.79);
});
