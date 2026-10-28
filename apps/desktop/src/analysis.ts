import type { Analysis, CrossPeak, Molecule, MultipletRange } from './api';

/** Highest ppm first, as a spectrum reads. The normalisation points at an
 * integral by position, so it is found again by identity and repointed. */
function ordered(a: Analysis, list: [number, number][]): Analysis {
  const held = a.reference ? list[a.reference.index] : undefined;
  const integrals = [...list].sort((x, y) => y[0] - x[0]);
  const reference =
    a.reference && held ? { ...a.reference, index: integrals.indexOf(held) } : a.reference;
  return { ...a, integrals, reference };
}

export function addIntegral(a: Analysis, from: number, to: number): Analysis {
  const range: [number, number] = [Math.max(from, to), Math.min(from, to)];
  if (
    a.integrals.some(
      ([high, low]) => Math.max(high, low) === range[0] && Math.min(high, low) === range[1],
    )
  )
    return a;
  return ordered(a, [...a.integrals, range]);
}

/** Removes an integral; the normalisation follows its reference integral or resets. */
export function removeIntegral(a: Analysis, index: number): Analysis {
  const ref = a.reference;
  const reference =
    !ref || ref.index === index
      ? null
      : { ...ref, index: ref.index > index ? ref.index - 1 : ref.index };
  return { ...a, integrals: a.integrals.filter((_, i) => i !== index), reference };
}

/** Moves every peak and integral by `delta` ppm.
 *
 * Referencing shifts the axis under the spectrum, but a peak is stored as the ppm
 * it sits at, so without this they would stay where the axis used to be. */
export const shiftAnalysis = (a: Analysis, delta: number): Analysis => ({
  ...a,
  peaks: a.peaks.map((ppm) => ppm + delta),
  integrals: a.integrals.map(([from, to]) => [from + delta, to + delta] as [number, number]),
  // A shift typed or fitted moves too; couplings and widths are in Hz and do not.
  multiplets: a.multiplets.map((m) => ({
    ...m,
    from: m.from + delta,
    to: m.to + delta,
    ppm: m.ppm === null || m.ppm === undefined ? m.ppm : m.ppm + delta,
  })),
});

/** Normalises all integrals so that integral `index` reads `value`. */
export const normalizeIntegral = (a: Analysis, index: number, value: number): Analysis => ({
  ...a,
  reference: { index, value },
});

/** A peak and what it has been called, so the two never drift apart. */
interface Labelled {
  ppm: number;
  label: string | null;
}

const pairs = (a: Analysis): Labelled[] =>
  a.peaks.map((ppm, i) => ({ ppm, label: a.labels[i] ?? null }));

/** Highest ppm first, which is how a spectrum reads. */
const unpair = (a: Analysis, list: Labelled[]): Analysis => {
  const sorted = [...list].sort((x, y) => y.ppm - x.ppm);
  return { ...a, peaks: sorted.map((p) => p.ppm), labels: sorted.map((p) => p.label) };
};

/** Adds peaks not within `tolerance` ppm of one already there, unlabelled. */
export function mergePeaks(a: Analysis, found: number[], tolerance: number): Analysis {
  const list = pairs(a);
  for (const ppm of found) {
    if (!list.some((peak) => Math.abs(peak.ppm - ppm) < tolerance)) list.push({ ppm, label: null });
  }
  return unpair(a, list);
}

/** Moves peak `index` to `ppm`, carrying its label with it. */
export const setPeak = (a: Analysis, index: number, ppm: number): Analysis =>
  unpair(
    a,
    pairs(a).map((peak, i) => (i === index ? { ...peak, ppm } : peak)),
  );

/** Names peak `index`: `null` follows the impurity table, `''` shows nothing. */
export const setPeakLabel = (a: Analysis, index: number, label: string | null): Analysis => ({
  ...a,
  labels: a.peaks.map((_, i) => (i === index ? label : (a.labels[i] ?? null))),
});

/** Moves the edges of integral `index`. */
export const setIntegral = (a: Analysis, index: number, from: number, to: number): Analysis =>
  ordered(
    a,
    a.integrals.map((r, i) =>
      i === index ? ([Math.max(from, to), Math.min(from, to)] as [number, number]) : r,
    ),
  );

export const removePeak = (a: Analysis, index: number): Analysis => ({
  ...a,
  peaks: a.peaks.filter((_, i) => i !== index),
  labels: a.peaks.map((_, i) => a.labels[i] ?? null).filter((_, i) => i !== index),
});

/** Highest shift first, as the spectrum reads. */
const orderedMultiplets = (a: Analysis, list: MultipletRange[]): Analysis => ({
  ...a,
  multiplets: [...list].sort((x, y) => y.from - x.from),
});

export const addMultiplet = (a: Analysis, from: number, to: number): Analysis =>
  orderedMultiplets(a, [...a.multiplets, { from: Math.max(from, to), to: Math.min(from, to) }]);

/** The multiplet covering the same range as integral `index`, if there is one.
 *
 * An integral and a multiplet read off the same signal are the same range twice:
 * automatically they come from `signal_regions` and are equal, and by hand they
 * are meant to be. They are drawn as one row when they agree, so they are matched
 * by where they are rather than by anything held between them - nothing has to be
 * kept in step, and a range edited apart stops being a pair on its own.
 *
 * `near` is how far apart two ends may be and still be the same range: a point of
 * the spectrum, since a drag lands between points and exact equality would come
 * apart the first time either was moved.
 */
export function pairedMultiplet(a: Analysis, index: number, near: number): number {
  const range = a.integrals[index];
  if (!range) return -1;
  return a.multiplets.findIndex(
    (m) => Math.abs(m.from - range[0]) <= near && Math.abs(m.to - range[1]) <= near,
  );
}

/** The integral covering the same range as multiplet `index`, if there is one. */
export function pairedIntegral(a: Analysis, index: number, near: number): number {
  const m = a.multiplets[index];
  if (!m) return -1;
  return a.integrals.findIndex(
    (range) => Math.abs(m.from - range[0]) <= near && Math.abs(m.to - range[1]) <= near,
  );
}

/** Moves the ends of multiplet `index`, as `setIntegral` does for an integral:
 * highest shift first, so an end dragged past the other turns the range round
 * rather than inverting it. */
export const setMultipletRange = (a: Analysis, index: number, from: number, to: number): Analysis =>
  orderedMultiplets(
    a,
    a.multiplets.map((m, i) =>
      i === index ? { ...m, from: Math.max(from, to), to: Math.min(from, to) } : m,
    ),
  );

/** Cuts multiplet `index` in two at `ppm`. Both halves read their lines again,
 * since what was typed described the whole. */
export function splitMultiplet(a: Analysis, index: number, ppm: number): Analysis {
  const m = a.multiplets[index];
  if (!m || ppm >= m.from || ppm <= m.to) return a;
  return orderedMultiplets(a, [
    ...a.multiplets.filter((_, i) => i !== index),
    { from: m.from, to: ppm },
    { from: ppm, to: m.to },
  ]);
}

/** Joins multiplet `index` with the next one down the spectrum. The whole is
 * read again and keeps only the first one's name. */
export function mergeMultiplets(a: Analysis, index: number): Analysis {
  const [m, n] = [a.multiplets[index], a.multiplets[index + 1]];
  if (!m || !n) return a;
  const joined = [...new Set([...(m.atoms ?? []), ...(n.atoms ?? [])])];
  return orderedMultiplets(a, [
    ...a.multiplets.filter((_, i) => i !== index && i !== index + 1),
    {
      from: Math.max(m.from, n.from),
      to: Math.min(m.to, n.to),
      name: m.name,
      ...(joined.length ? { atoms: joined } : {}),
    },
  ]);
}

/** Whether multiplet `index` draws the shape it would have, over the signal. */
export const setMultipletShape = (a: Analysis, index: number, shape: boolean): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) => (i === index ? { ...m, shape } : m)),
});

export const removeMultiplet = (a: Analysis, index: number): Analysis => ({
  ...a,
  multiplets: a.multiplets.filter((_, i) => i !== index),
});

/** Names multiplet `index`; empty goes back to the letter it is given by place. */
export const setMultipletName = (a: Analysis, index: number, name: string): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) => (i === index ? { ...m, name: name || null } : m)),
});

/** How many nuclei a multiplet stands for; 0 or less follows its integral again. */
export const setMultipletNuclei = (a: Analysis, index: number, nuclei: number): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) =>
    i === index ? { ...m, nuclei: nuclei > 0 ? nuclei : null } : m,
  ),
});

/** The class of multiplet `index`; empty reads the lines again. */
export const setMultipletKind = (a: Analysis, index: number, kind: string): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) => (i === index ? { ...m, kind: kind || null } : m)),
});

/** The couplings of multiplet `index` in Hz; none reads them again. */
export const setMultipletJs = (a: Analysis, index: number, jsHz: number[]): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) =>
    i === index ? { ...m, jsHz: jsHz.length > 0 ? jsHz : null } : m,
  ),
});

/** Reads a typed list of couplings: "7.8, 1.2" and "7,8 1,2" both work, since a
 * decimal comma is what half the world types. */
export const parseJs = (text: string): number[] =>
  text
    .split(/[;,\s]+/)
    .map((part) => Number(part.replace(',', '.')))
    .filter((j) => Number.isFinite(j) && j > 0);

/** The width a multiplet's simulated lines are drawn with, in Hz; 0 or less uses
 * the width the spectrum shows. */
export const setMultipletWidth = (a: Analysis, index: number, hz: number): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) => (i === index ? { ...m, widthHz: hz > 0 ? hz : null } : m)),
});

/** Where multiplet `index` is quoted at; null reads it off its lines again. */
export const setMultipletShift = (a: Analysis, index: number, ppm: number | null): Analysis => ({
  ...a,
  multiplets: a.multiplets.map((m, i) =>
    i === index ? { ...m, ppm: ppm !== null && Number.isFinite(ppm) ? ppm : null } : m,
  ),
});

/** The couplings a run of picked shifts stands for: the gaps between them in Hz,
 * read down the spectrum, since the spacing between two lines is what a coupling
 * is. The order they were clicked in does not matter. */
export function couplingsBetween(ppms: number[], obsMhz: number): number[] {
  const down = [...ppms].sort((a, b) => b - a);
  return down
    .slice(1)
    .map((ppm, i) => (down[i] - ppm) * obsMhz)
    .filter((hz) => hz > 0);
}

/** The atoms sharing `atom`'s class, hydrogens on them or not: what one click on
 * a structure picks, since equivalent hydrogens cannot be told apart. */
export const classOf = (molecule: Molecule, atom: number): number[] =>
  molecule.atoms.flatMap((a, i) => (a.class === molecule.atoms[atom]?.class && a.h > 0 ? [i] : []));

/** Assigns the atoms in `atoms` to multiplet `index`, or takes them away again when
 * it already has them all - unless `add`, which only ever gives. A hydrogen gives one signal, so they leave any other
 * multiplet. The nuclei follow the hydrogens assigned: set here rather than worked
 * out wherever they are read, so the table, the plot and the report agree. */
export function toggleAssignment(
  a: Analysis,
  index: number,
  molecule: Molecule,
  atoms: number[],
  add = false,
): Analysis {
  const held = a.multiplets[index]?.atoms ?? [];
  const remove = !add && atoms.every((i) => held.includes(i));
  return {
    ...a,
    multiplets: a.multiplets.map((m, k) => {
      const now = (m.atoms ?? []).filter((i) => !atoms.includes(i));
      if (k === index && !remove) now.push(...atoms);
      if (k !== index && now.length === (m.atoms ?? []).length) return m;
      const h = now.reduce((sum, i) => sum + (molecule.atoms[i]?.h ?? 0), 0);
      return { ...m, atoms: now, nuclei: h > 0 ? h : null };
    }),
  };
}

/** Takes every assignment away: a structure that is gone or replaced has no atoms
 * for them to point at. The nuclei counted from them go too. */
export const clearAssignments = (a: Analysis): Analysis =>
  a.multiplets.some((m) => m.atoms?.length)
    ? {
        ...a,
        multiplets: a.multiplets.map((m) =>
          m.atoms?.length ? { ...m, atoms: [], nuclei: null } : m,
        ),
      }
    : a;

/** `peaks` with cross peak `index` given `atoms`, or let go of them when it
 *  has exactly those already. Other cross peaks keep theirs: a CH₂ whose
 *  hydrogens differ gives two cross peaks of one carbon in an HSQC. */
export function assignCrossPeak(peaks: CrossPeak[], index: number, atoms: number[]): CrossPeak[] {
  const same = (given: number[]) =>
    given.length === atoms.length && given.every((a) => atoms.includes(a));
  return peaks.map((p, k) => (k === index ? { ...p, atoms: same(p.atoms) ? [] : atoms } : p));
}
