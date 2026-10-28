import type { DocumentInfo, MultipletInfo, MultipletRange } from './api';
import { suggestName } from './impurities';
import { findSolvent, formatSolvent } from './solvents';
import { fixed, nucleusLabel } from './traces';

/** The letter a multiplet is called by when it has not been named: A, B, C… down
 * the spectrum, as a report lettering does. */
export const letterOf = (index: number): string =>
  String.fromCharCode(65 + (index % 26)).repeat(Math.floor(index / 26) + 1);

/** How many nuclei a multiplet stands for: what was typed, or its integral
 * rounded, since a proton count is a whole number. */
export const nucleiOf = (range: MultipletRange, info: MultipletInfo | undefined): number | null =>
  range.nuclei ?? (info?.value == null ? null : Math.max(1, Math.round(info.value)));

/** How far apart two couplings may be and still be one: a J measured from two
 * multiplets differs by about the digital resolution and the line width. */
export const J_TOLERANCE = 0.5;

/** A coupling of one multiplet found again in another. */
export interface SharedCoupling {
  /** Which of this multiplet's couplings. */
  j: number;
  other: number;
  /** Which of the other's couplings. */
  k: number;
  hz: number;
}

/** The couplings multiplet `at` shares with the others. A coupling is felt by
 * both nuclei it joins, so the same J in two multiplets says they are
 * neighbours - and should be quoted as the same number. */
export function sharedCouplings(
  infos: MultipletInfo[],
  at: number,
  tolerance = J_TOLERANCE,
): SharedCoupling[] {
  const found: SharedCoupling[] = [];
  (infos[at]?.jsHz ?? []).forEach((mine, j) =>
    infos.forEach((info, other) => {
      if (other === at || info.kind === 'm') return;
      info.jsHz.forEach((hz, k) => {
        if (Math.abs(hz - mine) <= tolerance) found.push({ j, other, k, hz });
      });
    }),
  );
  return found;
}

/** The classes a multiplet can be given, in the order they are met. */
export const CLASSES = Object.keys({
  s: 0,
  d: 0,
  t: 0,
  q: 0,
  quint: 0,
  sext: 0,
  sept: 0,
  oct: 0,
  non: 0,
  dd: 0,
  ddd: 0,
  dddd: 0,
  dt: 0,
  td: 0,
  dq: 0,
  qd: 0,
  tt: 0,
  m: 0,
});

/** What a letter stands for: how often that coupling splits the multiplet. The
 * longest spellings come first, so `quint` is not read as a `q` with `uint` left
 * over. `s` and `m` carry no coupling at all. */
const LETTERS: [string, number][] = [
  ['quint', 4],
  ['sext', 5],
  ['sept', 6],
  ['oct', 7],
  ['non', 8],
  ['d', 1],
  ['t', 2],
  ['q', 3],
  ['s', 0],
  ['m', 0],
];

/** How often each of a class's couplings splits it, read off its letters rather
 * than looked up: a triplet is one coupling twice, a dt is one coupling once and
 * the next twice. The spectrum can now be read as classes nobody listed, so a
 * table of them would be a table to forget to add to. */
export function splittingsOf(kind: string): number[] | undefined {
  let rest = kind.trim();
  const splittings: number[] = [];
  while (rest) {
    const letter = LETTERS.find(([spelling]) => rest.startsWith(spelling));
    if (!letter) return undefined;
    if (letter[1] > 0) splittings.push(letter[1]);
    rest = rest.slice(letter[0].length);
  }
  return splittings;
}

/** Every coupling a multiplet is split by, one entry per splitting. */
export function couplingsOf(kind: string, jsHz: number[]): number[] {
  const pattern = splittingsOf(kind);
  if (!pattern) return jsHz.filter((j) => j > 0);
  return pattern
    .flatMap((times, i) => Array.from({ length: times }, () => jsHz[i] ?? 0))
    .filter((j) => j > 0);
}

export interface Line {
  ppm: number;
  /** Share of the multiplet's height, the tallest line being 1. */
  weight: number;
}

/** The lines a multiplet would have, first order: every coupling splits every
 * line in two, and lines that land together add up - which is where 1:2:1 and
 * 1:3:3:1 come from without anyone writing them down. */
export function simulateLines(ppm: number, kind: string, jsHz: number[], obsMhz: number): Line[] {
  let lines: Line[] = [{ ppm, weight: 1 }];
  for (const j of couplingsOf(kind, jsHz)) {
    const half = j / 2 / obsMhz;
    const split: Line[] = [];
    for (const line of lines) {
      for (const side of [-1, 1]) {
        const at = line.ppm + side * half;
        const same = split.find((s) => Math.abs(s.ppm - at) < 1e-9);
        if (same) same.weight += line.weight / 2;
        else split.push({ ppm: at, weight: line.weight / 2 });
      }
    }
    lines = split;
  }
  const tallest = Math.max(...lines.map((l) => l.weight));
  return lines
    .map((l) => ({ ppm: l.ppm, weight: l.weight / tallest }))
    .sort((a, b) => b.ppm - a.ppm);
}

/** One multiplet as a report quotes it: `7.31 (d, J = 7.8 Hz, 1H)`, and
 * `7.40 - 7.20 (m, 2H)` for one whose lines could not be read. What is quoted for
 * an unresolved signal is the range it covers, since a point inside it says
 * something the spectrum does not. `nuclei` null leaves the count out, as a ¹³C
 * doublet from a fluorine is quoted: `115.4 (d, J = 21.5 Hz)`. */
export function describeMultiplet(
  info: MultipletInfo,
  nuclei: number | null,
  decimals: number,
  range?: MultipletRange,
): string {
  const js = info.jsHz.map((j) => j.toFixed(1)).join(', ');
  const coupling = js ? `, J = ${js} Hz` : '';
  const where =
    info.kind === 'm' && range
      ? `${fixed(Math.max(range.from, range.to), decimals)} – ${fixed(Math.min(range.from, range.to), decimals)}`
      : fixed(info.ppm, decimals);
  return `${where} (${info.kind}${coupling}${nuclei === null ? '' : `, ${nuclei}H`})`;
}

/** Places a ¹³C shift is quoted to: a carbon line is tens of times wider in ppm
 * than a proton's, and papers write one. */
const CARBON_DECIMALS = 1;

/** Decimal places a shift of `nucleus` is written to: ¹³C to one, as it is
 *  reported, anything else to the `decimals` setting. */
export const placesFor = (nucleus: string, decimals: number) =>
  nucleus === '13C' ? CARBON_DECIMALS : decimals;

/** The whole spectrum as a report line, the way it is written up: nucleus,
 * frequency and solvent, then the signals down the spectrum. ¹H is quoted by its
 * multiplets; any other nucleus by its picked peaks, bare, one place for ¹³C,
 * with a multiplet that was read (a carbon split by fluorine) in place of the
 * peaks inside it. Peaks still taken for the solvent or an impurity are left
 * out; one whose suggested name was typed over or emptied is the sample's. */
export function report(doc: DocumentInfo, decimals: number): string {
  const solvent = findSolvent(doc.solvent, doc.nucleus);
  const where = solvent ? `, ${formatSolvent(solvent.formula)}` : '';
  const head = `${nucleusLabel(doc.nucleus)} NMR (${Math.round(doc.frequencyMhz)} MHz${where})`;
  const multiplets = doc.analysis.multiplets.flatMap((range, i) => {
    const info = doc.multipletInfo[i];
    return info ? [{ range, info }] : [];
  });
  if (doc.nucleus === '1H') {
    const parts = multiplets.map(({ range, info }) =>
      describeMultiplet(info, nucleiOf(range, info), decimals, range),
    );
    return `${head} δ ${parts.join(', ')}.`;
  }

  const places = placesFor(doc.nucleus, decimals);
  const inside = (ppm: number) =>
    multiplets.some(
      ({ range }) => ppm <= Math.max(range.from, range.to) && ppm >= Math.min(range.from, range.to),
    );
  const signals = [
    ...multiplets.map(({ range, info }) => ({
      ppm: info.ppm,
      text: describeMultiplet(info, null, places, range),
    })),
    ...doc.analysis.peaks.flatMap((ppm, i) => {
      const other = (doc.analysis.labels[i] ?? null) === null && suggestName(ppm, solvent);
      return other || inside(ppm) ? [] : [{ ppm, text: fixed(ppm, places) }];
    }),
  ].sort((a, b) => b.ppm - a.ppm);
  return `${head} δ ${signals.map((s) => s.text).join(', ')}.`;
}
