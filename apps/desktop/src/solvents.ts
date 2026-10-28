/** Residual shifts of the usual deuterated solvents.
 *
 * Values as tabulated by Fulmer et al., Organometallics 29 (2010) 2176 (Tables 1
 * and 2), which is the table commonly used for referencing.
 */
export interface Solvent {
  /** What the solvent is called. */
  name: string;
  /** How it is written. */
  formula: string;
  /** Residual shift in ppm, of the nucleus it was found for (¹H in `SOLVENTS`). */
  shift: number;
  /** Its other lines of that nucleus: acetone-d6's carbonyl beside its methyl. */
  others?: number[];
  /** ¹³C lines, the one referenced to first; none for D2O, which has no carbon. */
  carbon?: number[];
  /** The nucleus `shift` belongs to. */
  nucleus?: string;
  /** What a dataset may call it: it may write `DMSO`, not `DMSO-d6`. */
  aliases?: string[];
}

export const SOLVENTS: Solvent[] = [
  { name: 'Chloroform-d', formula: 'CDCl3', shift: 7.26, carbon: [77.16] },
  {
    name: 'Dimethyl sulfoxide-d6',
    formula: '(CD3)2SO',
    shift: 2.5,
    carbon: [39.52],
    aliases: ['DMSO', 'DMSO-d6'],
  },
  { name: 'Water-d2', formula: 'D2O', shift: 4.79 },
  {
    name: 'Acetone-d6',
    formula: '(CD3)2CO',
    shift: 2.05,
    carbon: [29.84, 206.26],
    aliases: ['Acetone'],
  },
  {
    name: 'Methanol-d4',
    formula: 'CD3OD',
    shift: 3.31,
    carbon: [49.0],
    aliases: ['MeOD', 'Methanol'],
  },
  { name: 'Benzene-d6', formula: 'C6D6', shift: 7.16, carbon: [128.06], aliases: ['Benzene'] },
  {
    name: 'Acetonitrile-d3',
    formula: 'CD3CN',
    shift: 1.94,
    carbon: [1.32, 118.26],
    aliases: ['MeCN', 'Acetonitrile'],
  },
  {
    name: 'Tetrahydrofuran-d8',
    formula: 'C4D8O',
    shift: 3.58,
    carbon: [67.21, 25.31],
    aliases: ['THF'],
  },
  {
    name: 'Toluene-d8',
    formula: 'C7D8',
    shift: 2.08,
    carbon: [20.43, 137.48, 128.87, 127.96, 125.13],
    aliases: ['Tol', 'Toluene'],
  },
  {
    name: 'Dichloromethane-d2',
    formula: 'CD2Cl2',
    shift: 5.32,
    carbon: [53.84],
    aliases: ['DCM'],
  },
  { name: 'Pyridine-d5', formula: 'C5D5N', shift: 8.74, aliases: ['Pyr', 'Pyridine'] },
  { name: 'Tetramethylsilane', formula: 'Si(CH3)4', shift: 0, carbon: [0], aliases: ['TMS'] },
];

const key = (text: string) => text.replaceAll(/[^a-z0-9]/gi, '').toLowerCase();

/** The solvents that have a line of `nucleus`, with `shift` set to it. */
export function solventsFor(nucleus = '1H'): Solvent[] {
  return SOLVENTS.flatMap((entry) => {
    if (nucleus === '13C') {
      if (!entry.carbon?.length) return [];
      const [shift, ...others] = entry.carbon;
      return [{ ...entry, shift, others, nucleus }];
    }
    return nucleus === '1H' ? [{ ...entry, nucleus }] : [];
  });
}

/** The solvent a dataset names, by formula, name or the spelling it uses, with
 * its line of `nucleus`; undefined when that nucleus has none tabulated. */
export function findSolvent(solvent: string | null, nucleus = '1H'): Solvent | undefined {
  if (!solvent) return undefined;
  const wanted = key(solvent);
  return solventsFor(nucleus).find((entry) =>
    [entry.formula, entry.name, ...(entry.aliases ?? [])].some((text) => key(text) === wanted),
  );
}

export const solventShift = (solvent: string | null, nucleus = '1H'): number | undefined =>
  findSolvent(solvent, nucleus)?.shift;

/** A formula as it is written: the counts lowered, as in CDCl₃ or C₆D₆. */
export function formatSolvent(formula: string): string {
  const lowered = '₀₁₂₃₄₅₆₇₈₉';
  // Only a count that follows a letter or a bracket drops.
  return formula.replaceAll(/(?<=[A-Za-z)])\d+/g, (digits) =>
    [...digits].map((d) => lowered[Number(d)] ?? d).join(''),
  );
}
