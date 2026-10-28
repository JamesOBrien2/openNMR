/** Shifts of the impurities a sample commonly picks up, by solvent.
 *
 * From Fulmer et al., "NMR Chemical Shifts of Trace Impurities", Organometallics
 * 29 (2010) 2176. A peak near one of these is worth a second look; it is a
 * suggestion, never an assignment, so the tolerance is deliberately tight.
 */
import type { Solvent } from './solvents';

export interface Impurity {
  name: string;
  shift: number;
}

/** Keyed by the formula the solvent table uses. */
export const IMPURITIES: Record<string, Impurity[]> = {
  CDCl3: [
    { name: 'TMS', shift: 0.0 },
    { name: 'silicone grease', shift: 0.07 },
    { name: 'hexane CH₃', shift: 0.88 },
    { name: 'Et₂O CH₃', shift: 1.21 },
    { name: 'EtOH CH₃', shift: 1.25 },
    { name: 'hexane CH₂', shift: 1.26 },
    { name: 'cyclohexane', shift: 1.43 },
    { name: 'H₂O', shift: 1.56 },
    { name: 'EtOAc CH₃CO', shift: 2.05 },
    { name: 'acetic acid', shift: 2.1 },
    { name: 'acetone', shift: 2.17 },
    { name: 'toluene CH₃', shift: 2.36 },
    { name: 'methanol', shift: 3.49 },
    { name: 'Et₂O CH₂', shift: 3.48 },
    { name: 'dioxane', shift: 3.71 },
    { name: 'EtOH CH₂', shift: 3.72 },
    { name: 'THF OCH₂', shift: 3.76 },
    { name: 'EtOAc OCH₂', shift: 4.12 },
    { name: 'CH₂Cl₂', shift: 5.3 },
    { name: 'CHCl₃', shift: 7.26 },
  ],
  '(CD3)2SO': [
    { name: 'silicone grease', shift: 0.29 },
    { name: 'hexane CH₃', shift: 0.86 },
    { name: 'EtOH CH₃', shift: 1.06 },
    { name: 'Et₂O CH₃', shift: 1.09 },
    { name: 'hexane CH₂', shift: 1.25 },
    { name: 'cyclohexane', shift: 1.4 },
    { name: 'acetic acid', shift: 1.91 },
    { name: 'EtOAc CH₃CO', shift: 1.99 },
    { name: 'acetone', shift: 2.09 },
    { name: 'toluene CH₃', shift: 2.3 },
    { name: 'methanol CH₃', shift: 3.16 },
    { name: 'H₂O', shift: 3.33 },
    { name: 'Et₂O CH₂', shift: 3.38 },
    { name: 'EtOH CH₂', shift: 3.44 },
    { name: 'dioxane', shift: 3.57 },
    { name: 'THF OCH₂', shift: 3.6 },
    { name: 'EtOAc OCH₂', shift: 4.03 },
    { name: 'CH₂Cl₂', shift: 5.76 },
    { name: 'CHCl₃', shift: 8.32 },
  ],
  D2O: [
    { name: 'EtOH CH₃', shift: 1.17 },
    { name: 'tert-butanol', shift: 1.24 },
    { name: 'EtOAc CH₃', shift: 1.24 },
    { name: 'THF CH₂', shift: 1.88 },
    { name: 'acetonitrile', shift: 2.06 },
    { name: 'EtOAc CH₃CO', shift: 2.07 },
    { name: 'acetone', shift: 2.22 },
    { name: 'DMSO', shift: 2.71 },
    { name: 'methanol', shift: 3.34 },
    { name: 'EtOH CH₂', shift: 3.65 },
    { name: 'THF OCH₂', shift: 3.74 },
    { name: 'dioxane', shift: 3.75 },
    { name: 'EtOAc OCH₂', shift: 4.14 },
    { name: 'HDO', shift: 4.79 },
  ],
  CD3OD: [
    { name: 'hexane CH₃', shift: 0.9 },
    { name: 'Et₂O CH₃', shift: 1.18 },
    { name: 'EtOH CH₃', shift: 1.19 },
    { name: 'EtOAc CH₃', shift: 1.24 },
    { name: 'hexane CH₂', shift: 1.29 },
    { name: 'THF CH₂', shift: 1.87 },
    { name: 'EtOAc CH₃CO', shift: 2.01 },
    { name: 'acetone', shift: 2.15 },
    { name: 'toluene CH₃', shift: 2.32 },
    { name: 'DMSO', shift: 2.65 },
    { name: 'methanol CH₃', shift: 3.34 },
    { name: 'Et₂O CH₂', shift: 3.49 },
    { name: 'EtOH CH₂', shift: 3.6 },
    { name: 'dioxane', shift: 3.66 },
    { name: 'THF OCH₂', shift: 3.71 },
    { name: 'EtOAc OCH₂', shift: 4.09 },
    { name: 'H₂O', shift: 4.87 },
    { name: 'CH₂Cl₂', shift: 5.49 },
    { name: 'CHCl₃', shift: 7.9 },
  ],
  C6D6: [
    { name: 'silicone grease', shift: 0.29 },
    { name: 'H₂O', shift: 0.4 },
    { name: 'hexane CH₃', shift: 0.89 },
    { name: 'EtOAc CH₃', shift: 0.92 },
    { name: 'EtOH CH₃', shift: 0.97 },
    { name: 'Et₂O CH₃', shift: 1.11 },
    { name: 'hexane CH₂', shift: 1.24 },
    { name: 'cyclohexane', shift: 1.4 },
    { name: 'THF CH₂', shift: 1.4 },
    { name: 'acetone', shift: 1.55 },
    { name: 'DMSO', shift: 1.68 },
    { name: 'toluene CH₃', shift: 2.11 },
    { name: 'methanol', shift: 3.07 },
    { name: 'Et₂O CH₂', shift: 3.26 },
    { name: 'EtOH CH₂', shift: 3.34 },
    { name: 'dioxane', shift: 3.35 },
    { name: 'THF OCH₂', shift: 3.57 },
    { name: 'EtOAc OCH₂', shift: 3.89 },
    { name: 'CH₂Cl₂', shift: 4.27 },
    { name: 'CHCl₃', shift: 6.15 },
  ],
  CD3CN: [
    { name: 'Et₂O CH₃', shift: 1.12 },
    { name: 'EtOH CH₃', shift: 1.12 },
    { name: 'EtOAc CH₃', shift: 1.2 },
    { name: 'THF CH₂', shift: 1.8 },
    { name: 'acetonitrile', shift: 1.94 },
    { name: 'EtOAc CH₃CO', shift: 1.97 },
    { name: 'acetone', shift: 2.08 },
    { name: 'H₂O', shift: 2.13 },
    { name: 'toluene CH₃', shift: 2.33 },
    { name: 'DMSO', shift: 2.5 },
    { name: 'Et₂O CH₂', shift: 3.42 },
    { name: 'EtOH CH₂', shift: 3.54 },
    { name: 'dioxane', shift: 3.6 },
    { name: 'THF OCH₂', shift: 3.64 },
    { name: 'EtOAc OCH₂', shift: 4.06 },
    { name: 'CH₂Cl₂', shift: 5.44 },
    { name: 'CHCl₃', shift: 7.58 },
  ],
  '(CD3)2CO': [
    { name: 'Et₂O CH₃', shift: 1.11 },
    { name: 'EtOH CH₃', shift: 1.12 },
    { name: 'EtOAc CH₃', shift: 1.2 },
    { name: 'THF CH₂', shift: 1.79 },
    { name: 'EtOAc CH₃CO', shift: 1.97 },
    { name: 'acetone', shift: 2.09 },
    { name: 'toluene CH₃', shift: 2.32 },
    { name: 'DMSO', shift: 2.52 },
    { name: 'H₂O', shift: 2.84 },
    { name: 'methanol', shift: 3.31 },
    { name: 'Et₂O CH₂', shift: 3.41 },
    { name: 'EtOH CH₂', shift: 3.57 },
    { name: 'dioxane', shift: 3.59 },
    { name: 'THF OCH₂', shift: 3.63 },
    { name: 'EtOAc OCH₂', shift: 4.05 },
    { name: 'CH₂Cl₂', shift: 5.63 },
    { name: 'CHCl₃', shift: 8.02 },
  ],
};

/** The solvents of `CARBON_ROWS`, in the order of their columns. */
const CARBON_SOLVENTS = ['CDCl3', '(CD3)2SO', 'D2O', 'CD3OD', 'C6D6', 'CD3CN', '(CD3)2CO'];

/** ¹³C shifts, row by row as Fulmer et al. Table 2 prints them, in the columns of
 * `CARBON_SOLVENTS`; null where the paper has none, or where it is the solvent
 * itself. DMSO is not in that table and comes from Gottlieb, Kotlyar & Nudelman,
 * J. Org. Chem. 62 (1997) 7512, Table 2. */
const CARBON_ROWS: [string, (number | null)[]][] = [
  ['TMS', [0.0, null, null, null, null, null, null]],
  ['acetic acid CO', [175.99, 171.93, 177.21, 175.11, 175.82, 173.21, 172.31]],
  ['acetic acid CH₃', [20.81, 20.95, 21.03, 20.56, 20.37, 20.73, 20.51]],
  ['acetone CO', [207.07, 206.31, 215.94, 209.67, 204.43, 207.43, 205.87]],
  ['acetone CH₃', [30.92, 30.56, 30.89, 30.67, 30.14, 30.91, 30.6]],
  ['acetonitrile CN', [116.43, 117.91, 119.68, 118.06, 116.02, null, 117.6]],
  ['acetonitrile CH₃', [1.89, 1.03, 1.47, 0.85, 0.2, null, 1.12]],
  ['tert-butanol C', [69.15, 66.88, 70.36, 69.4, 68.19, 68.74, 68.13]],
  ['tert-butanol CH₃', [31.25, 30.38, 30.29, 30.91, 30.47, 30.68, 30.72]],
  ['CHCl₃', [null, 79.16, null, 79.44, 77.79, 79.17, 79.19]],
  ['cyclohexane', [26.94, 26.33, null, 27.96, 27.23, 27.63, 27.51]],
  ['CH₂Cl₂', [53.52, 54.84, null, 54.78, 53.46, 55.32, 54.95]],
  ['Et₂O CH₃', [15.2, 15.12, 14.77, 15.46, 15.46, 15.63, 15.78]],
  ['Et₂O CH₂', [65.91, 62.05, 66.42, 66.88, 65.94, 66.32, 66.12]],
  ['DMSO', [40.76, null, 39.39, 40.45, 40.03, 41.31, 41.23]],
  ['dioxane', [67.14, 66.36, 67.19, 68.11, 67.16, 67.72, 67.6]],
  ['EtOH CH₃', [18.41, 18.51, 17.47, 18.4, 18.72, 18.8, 18.89]],
  ['EtOH CH₂', [58.28, 56.07, 58.05, 58.26, 57.86, 57.96, 57.72]],
  ['EtOAc CH₃CO', [21.04, 20.68, 21.15, 20.88, 20.56, 21.16, 20.83]],
  ['EtOAc CO', [171.36, 170.31, 175.26, 172.89, 170.44, 171.68, 170.96]],
  ['EtOAc OCH₂', [60.49, 59.74, 62.32, 61.5, 60.21, 60.98, 60.56]],
  ['EtOAc CH₃', [14.19, 14.4, 13.92, 14.49, 14.19, 14.54, 14.5]],
  ['hexane CH₃', [14.14, 13.88, null, 14.45, 14.32, 14.43, 14.34]],
  ['hexane CH₂', [22.7, 22.05, null, 23.68, 23.04, 23.4, 23.28]],
  ['hexane CH₂', [31.64, 30.95, null, 32.73, 31.96, 32.36, 32.3]],
  ['methanol', [50.41, 48.59, 49.5, null, 49.97, 49.9, 49.77]],
  ['THF OCH₂', [67.97, 67.03, 68.68, 68.83, 67.8, 68.33, 68.07]],
  ['THF CH₂', [25.62, 25.14, 25.67, 26.48, 25.72, 26.27, 26.15]],
  ['toluene CH₃', [21.46, 20.99, null, 21.5, 21.1, 21.5, 21.46]],
];

/** ¹³C impurities, keyed and sorted as `IMPURITIES` is. */
export const CARBON_IMPURITIES: Record<string, Impurity[]> = Object.fromEntries(
  CARBON_SOLVENTS.map((formula, column) => [
    formula,
    CARBON_ROWS.flatMap(([name, shifts]) => {
      const shift = shifts[column];
      return shift === null || shift === undefined ? [] : [{ name, shift }];
    }).sort((a, b) => a.shift - b.shift),
  ]),
);

/** How near a peak has to be, in ppm, before an impurity is suggested. */
export const IMPURITY_TOLERANCE = 0.03;
/** The same for ¹³C, whose shifts Gottlieb et al. found to vary by up to
 * ±0.13 ppm from sample to sample (acetone-d6's carbonyl). */
export const CARBON_TOLERANCE = 0.15;
/** How far either side of a ¹³C solvent line its own multiplet reaches: the
 * deuterium splits it by 20-32 Hz (Gottlieb et al., note 3), so chloroform-d's
 * triplet spans ±32 Hz, 0.43 ppm at 75 MHz, and DMSO-d6's septet ±63 Hz, 0.42
 * ppm at 151 MHz. */
export const CARBON_SOLVENT_REACH = 0.5;

const tablesFor = (solvent: Solvent | undefined) =>
  solvent?.nucleus === '13C' ? CARBON_IMPURITIES : IMPURITIES;

const toleranceFor = (solvent: Solvent | undefined) =>
  solvent?.nucleus === '13C' ? CARBON_TOLERANCE : IMPURITY_TOLERANCE;

/** Every line of the solvent's own nucleus, and how far each reaches. */
function solventLines(solvent: Solvent): [number[], number] {
  const lines = [solvent.shift, ...(solvent.others ?? [])];
  return [lines, solvent.nucleus === '13C' ? CARBON_SOLVENT_REACH : IMPURITY_TOLERANCE];
}

/** Whether a range covers a line of the selected solvent. */
export function isSolvent(from: number, to: number, solvent: Solvent | undefined): boolean {
  const [high, low] = [Math.max(from, to), Math.min(from, to)];
  const within = (shift: number) => shift <= high && shift >= low;
  return !!solvent && solventLines(solvent)[0].some(within);
}

/** What a peak at `ppm` is likely to be: the residual solvent itself, or one of
 * the trace impurities tabulated for it, in the solvent's nucleus. The solvent is
 * tried first, since its own peak is usually the tallest thing in the spectrum
 * that is not the sample. A suggestion is never an assignment; it can be typed
 * over or emptied. */
export function suggestName(ppm: number, solvent: Solvent | undefined): string | undefined {
  if (solvent) {
    const [lines, reach] = solventLines(solvent);
    if (lines.some((line) => Math.abs(line - ppm) <= reach)) {
      // The short spelling a spectrometer writes reads better above a peak than
      // the full name does.
      return solvent.aliases?.[0] ?? solvent.name;
    }
  }
  return matchImpurity(ppm, solvent?.formula, toleranceFor(solvent), tablesFor(solvent))?.name;
}

/** The impurity nearest `ppm` in the solvent's table, if one is near enough. */
export function matchImpurity(
  ppm: number,
  formula: string | undefined,
  tolerance = IMPURITY_TOLERANCE,
  impurities = IMPURITIES,
): Impurity | undefined {
  const table = formula ? impurities[formula] : undefined;
  if (!table) return undefined;
  let nearest: Impurity | undefined;
  let best = tolerance;
  for (const entry of table) {
    const distance = Math.abs(entry.shift - ppm);
    if (distance <= best) {
      best = distance;
      nearest = entry;
    }
  }
  return nearest;
}
