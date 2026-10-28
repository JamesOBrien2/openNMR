/**
 * A stand-in for the Rust back end, so the interface can be opened in a plain
 * browser and looked at without Tauri: `npm run dev`, then /preview.html.
 *
 * Open folder offers real spectra when `public/preview-data` holds them, written
 * by `cargo run -p nmr-core --example preview_data -- <experiment folders>` with
 * the crate's own processing; otherwise three made-up spectra of a titration.
 * The analysis is a rough imitation of `nmr_core::analysis`, good enough to fill
 * every table and draw every annotation. It is for studying how the app looks
 * and behaves, not for results.
 */
import type {
  Analysis,
  CrossPeak,
  Document2DInfo,
  DocumentInfo,
  MultipletInfo,
  Operation,
  PhaseParams,
  PeakFit,
  PeakMethod,
} from '../api';

const POINTS = 32768;
const FIRST_PPM = 16.5;
const WIDTH_PPM = 20;
const MHZ = 400.13;
const DELTA = -WIDTH_PPM / POINTS;
const HZ_PER_POINT = (WIDTH_PPM * MHZ) / POINTS;
/** Real spectra written by the preview_data example, if there are any. */
const REAL = fetch('/preview-data/index.json')
  .then((r) => (r.ok ? (r.json() as Promise<string[]>) : []))
  .catch(() => [] as string[]);
/** The phase error the made-up FID carries, undone by the recorded phase. */
const RECORDED = { p0: 38, p1: -12 };

interface Signal {
  ppm: number;
  height: number;
  /** Couplings in Hz, each splitting every line in two. */
  js?: number[];
  widthHz?: number;
}

const SIGNALS: Signal[] = [
  { ppm: 10.21, height: 0.08, widthHz: 12 },
  { ppm: 7.92, height: 0.55, js: [8.4] },
  { ppm: 7.41, height: 0.55, js: [8.4] },
  { ppm: 7.18, height: 0.3, js: [7.5, 7.5] },
  { ppm: 4.29, height: 0.36, js: [7.1, 7.1, 7.1] },
  { ppm: 3.33, height: 0.45, widthHz: 6 },
  { ppm: 2.5, height: 0.4, js: [1.9, 1.9, 1.9, 1.9] },
  { ppm: 2.36, height: 1, widthHz: 1 },
  { ppm: 1.31, height: 0.6, js: [7.1, 7.1] },
  { ppm: 0, height: 0.15, widthHz: 0.8 },
];

/** Known sample components for exercising fitted/unavailable states, not a fitter. */
const FIT_SIGNALS: Signal[] = [
  { ppm: 6.1, height: 0.65, widthHz: 1.6 },
  { ppm: 6.096, height: 0.42, widthHz: 1.6 },
  { ppm: 5.2, height: 0.2, widthHz: 2 },
];

/** Deterministic noise, so every reload looks the same. */
function noise(seed: number) {
  let s = seed;
  return () => {
    s = (s * 1103515245 + 12345) % 2147483648;
    return s / 2147483648 - 0.5;
  };
}

const indexOf = (ppm: number) => (ppm - FIRST_PPM) / DELTA;

/** The rolling baseline the made-up spectrum sits on. */
const roll = (i: number) => {
  const x = i / POINTS;
  return 0.03 * Math.sin(2.4 * Math.PI * x) + 0.02 * x;
};

/** Complex spectrum, dephased by `RECORDED` and with a rolling baseline. */
function synthesise(shift: number, seed: number, fitFixture = false) {
  const re = new Float64Array(POINTS);
  const im = new Float64Array(POINTS);
  for (const signal of [...SIGNALS, ...(fitFixture ? FIT_SIGNALS : [])]) {
    const moves = signal.ppm > 7 && signal.ppm < 8 ? shift : 0;
    let lines = [{ offset: 0, weight: 1 }];
    for (const j of signal.js ?? []) {
      lines = lines.flatMap((l) => [
        { offset: l.offset - j / 2, weight: l.weight / 2 },
        { offset: l.offset + j / 2, weight: l.weight / 2 },
      ]);
    }
    const w = (signal.widthHz ?? 0.9) / HZ_PER_POINT;
    const peak = Math.max(...lines.map((l) => l.weight));
    for (const line of lines) {
      const centre = indexOf(signal.ppm + moves + line.offset / MHZ);
      const h = (signal.height * line.weight) / peak;
      const from = Math.max(0, Math.floor(centre - 400 * w));
      const to = Math.min(POINTS, Math.ceil(centre + 400 * w));
      for (let i = from; i < to; i++) {
        const u = (i - centre) / w;
        re[i] += h / (1 + u * u);
        im[i] -= (h * u) / (1 + u * u);
      }
    }
  }
  const random = noise(seed);
  for (let i = 0; i < POINTS; i++) {
    re[i] += random() * 0.004;
    im[i] += random() * 0.004;
  }
  // Undo of the recorded phase is what makes it absorptive, so apply its inverse.
  rotate(re, im, -RECORDED.p0, -RECORDED.p1, 0);
  for (let i = 0; i < POINTS; i++) re[i] += roll(i);
  return { re, im };
}

function rotate(re: Float64Array, im: Float64Array, p0: number, p1: number, pivot: number) {
  const base = ((p0 - p1 * pivot) * Math.PI) / 180;
  const slope = (p1 * Math.PI) / 180;
  for (let k = 0; k < re.length; k++) {
    const a = base + (slope * k) / re.length;
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const r = re[k] * c - im[k] * s;
    im[k] = re[k] * s + im[k] * c;
    re[k] = r;
  }
}

interface Doc {
  name: string;
  path: string;
  nucleus: string;
  solvent: string | null;
  mhz: number;
  points: number;
  /** The axis as it comes out of the transform, before any reference. */
  origin: number;
  delta: number;
  /** The spectrum as it comes out of the transform, before phase. */
  re: Float64Array;
  im: Float64Array;
  /** What the default baseline correction takes off. */
  baseline: Float64Array;
  recorded: { p0: number; p1: number };
  lb: number | null;
  size: number | null;
  conjugate: boolean;
  real: Float64Array;
  first: number;
  state: { operations: Operation[]; analysis: Analysis };
  undo: Doc['state'][];
  redo: Doc['state'][];
  revision: number;
  fitFixture?: boolean;
}

const docs: (Doc | undefined)[] = [];

const emptyAnalysis = (): Analysis => ({
  integrals: [],
  reference: null,
  peaks: [],
  labels: [],
  multiplets: [],
});

/** The steps `process::auto_1d` would open the spectrum with. */
const defaultOps = (doc: Doc, baseline: boolean): Operation[] => [
  ...(doc.conjugate ? [{ op: 'conjugate' } as Operation] : []),
  { op: 'digital_filter' },
  ...(doc.lb ? [{ op: 'exponential', lb_hz: doc.lb } as Operation] : []),
  ...(doc.size ? [{ op: 'zero_fill', size: doc.size } as Operation] : []),
  { op: 'ft' },
  { op: 'phase', p0: doc.recorded.p0, p1: doc.recorded.p1, pivot: 0 },
  ...(baseline ? [{ op: 'baseline_whittaker', lambda: 1e5, filter_hz: 7.3 } as Operation] : []),
];

/** The real part `ops` give; a baseline step is `baseline` instead when one
 * is given (`null` takes it out). */
function realOf(doc: Doc, ops: Operation[], baseline?: Operation | null) {
  const re = Float64Array.from(doc.re);
  const im = Float64Array.from(doc.im);
  let steps = ops;
  if (baseline !== undefined) {
    steps = ops.filter((o) => !o.op.startsWith('baseline_'));
    if (baseline) steps = [...steps, baseline];
  }
  for (const op of steps) {
    if (op.op === 'phase') rotate(re, im, op.p0, op.p1, op.pivot);
    else if (op.op === 'baseline_points') {
      const curve = handSet(re, op).curve;
      for (let i = 0; i < doc.points; i++) re[i] -= curve[i];
    } else if (op.op.startsWith('baseline_'))
      for (let i = 0; i < doc.points; i++) re[i] -= doc.baseline[i];
  }
  return re;
}

/** A stand-in for `process::hand_set`: each point at the mean of its span (or
 * its own height), joined by straight segments whatever the function. */
function handSet(real: Float64Array, op: Extract<Operation, { op: 'baseline_points' }>) {
  const n = real.length;
  const top = real.reduce((m, v) => Math.max(m, Math.abs(v)), 0);
  const values = op.points.map((p) => {
    const i = Math.min(n - 1, Math.max(0, Math.round(p.at * n)));
    if (p.y !== undefined) return [i, p.y * top];
    const [a, b] = [Math.max(0, i - op.span), Math.min(n, i + op.span + 1)];
    let sum = 0;
    for (let k = a; k < b; k++) sum += real[k];
    return [i, sum / (b - a)];
  });
  const sorted = [...values].sort((x, y) => x[0] - y[0]);
  const curve = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const k = sorted.findIndex((v) => v[0] >= i);
    const [x0, y0] = sorted[Math.max(0, k < 0 ? sorted.length - 1 : k - 1)] ?? [0, 0];
    const [x1, y1] = sorted[k < 0 ? sorted.length - 1 : k] ?? [0, 0];
    curve[i] = x1 === x0 ? y1 : y0 + ((y1 - y0) * (i - x0)) / (x1 - x0);
  }
  return { values, curve, top };
}

function process(doc: Doc) {
  let first = doc.origin;
  for (const op of doc.state.operations) {
    if (op.op === 'reference') first += op.to_ppm - op.from_ppm;
  }
  doc.real = realOf(doc, doc.state.operations);
  doc.first = first;
  doc.revision++;
}

const at = (doc: Doc, index: number) => doc.first + index * doc.delta;
const idx = (doc: Doc, ppm: number) => Math.round((ppm - doc.first) / doc.delta);
const range = (doc: Doc, a: number, b: number) => {
  const [i, j] = [idx(doc, a), idx(doc, b)].sort((x, y) => x - y);
  return [Math.max(0, i), Math.min(doc.points - 1, j)];
};

function noiseLevel(real: Float64Array) {
  const values = Array.from(real.subarray(0, real.length >> 4)).sort((a, b) => a - b);
  const median = values[values.length >> 1];
  const deviations = values.map((v) => Math.abs(v - median)).sort((a, b) => a - b);
  return 1.4826 * deviations[deviations.length >> 1];
}

function area(doc: Doc, a: number, b: number) {
  const [i, j] = range(doc, a, b);
  let sum = 0;
  for (let k = i; k <= j; k++) sum += doc.real[k];
  return sum * Math.abs(doc.delta);
}

function maxima(doc: Doc, a: number, b: number, threshold: number) {
  const [i, j] = range(doc, a, b);
  const found: number[] = [];
  for (let k = Math.max(1, i); k < Math.min(doc.points - 1, j); k++) {
    const y = doc.real[k];
    const sides = (doc.real[k - 6] + doc.real[k + 6]) / 2;
    // A line, not a ripple: the top stands clear of the points either side.
    const sharp = y - sides > threshold / 2;
    if (y > threshold && sharp && y > doc.real[k - 1] && y >= doc.real[k + 1]) found.push(k);
  }
  return found;
}

function widthHz(doc: Doc, k: number) {
  const half = doc.real[k] / 2;
  let [l, r] = [k, k];
  while (l > 0 && doc.real[l] > half) l--;
  while (r < doc.points - 1 && doc.real[r] > half) r++;
  return (r - l) * Math.abs(doc.delta) * doc.mhz;
}

function regions(doc: Doc, a: number, b: number): [number, number][] {
  const low = Math.min(a, b);
  const high = Math.max(a, b);
  const peaks = doc.state.analysis.peaks
    .filter((ppm) => ppm >= low && ppm <= high)
    .sort((x, y) => y - x);
  const regions: [number, number][] = [];
  for (const ppm of peaks) {
    const width = Math.max(0.015, (widthHz(doc, idx(doc, ppm)) / doc.mhz) * 3);
    const last = regions[regions.length - 1];
    if (last && ppm + width >= last[1]) last[1] = Math.min(last[1], Math.max(low, ppm - width));
    else regions.push([Math.min(high, ppm + width), Math.max(low, ppm - width)]);
  }
  return regions;
}

/** Only known fixture components succeed. Real exported data never gets a made-up fit. */
function fixtureFit(doc: Doc, accepted: number[]): PeakFit {
  const peaks: PeakFit['peaks'] = [];
  const regions: PeakFit['regions'] = [];
  const used = new Set<number>();
  for (const ppm of accepted) {
    const index = doc.fitFixture
      ? FIT_SIGNALS.slice(0, 2).findIndex(
          (line, i) => !used.has(i) && Math.abs(line.ppm - ppm) < 0.004,
        )
      : -1;
    if (index >= 0) {
      used.add(index);
      const line = FIT_SIGNALS[index];
      const width = line.widthHz ?? 1;
      peaks.push({
        ppm,
        height: line.height,
        widthHz: 2 * width,
        area: (line.height * Math.PI * width) / doc.mhz,
      });
    }
    regions.push({
      from: ppm + 0.025,
      to: ppm - 0.025,
      status: index >= 0 ? 'converged' : 'failed',
      rms: index >= 0 ? 0.002 : null,
    });
  }
  return { source: 'illustrative-preview', peaks, regions };
}

function fittedArea(doc: Doc, a: number, b: number): number | null {
  const fit = doc.state.analysis.peakFit;
  if (!fit) return null;
  const low = Math.min(a, b);
  const high = Math.max(a, b);
  if (
    fit.regions.some(
      (region) =>
        region.status !== 'converged' &&
        Math.max(region.from, region.to) > low &&
        Math.min(region.from, region.to) < high,
    )
  )
    return null;
  const peaks = fit.peaks.filter((peak) => peak.ppm >= low && peak.ppm < high);
  return peaks.length ? peaks.reduce((sum, peak) => sum + peak.area, 0) : null;
}

const KINDS = ['s', 'd', 't', 'q', 'quint', 'sext', 'sept'];

function info(id: number): DocumentInfo {
  const doc = docs[id];
  if (!doc) throw new Error(`No document ${id}`);
  const { analysis } = doc.state;
  const noise = noiseLevel(doc.real);
  const selectedArea = (a: number, b: number) =>
    analysis.integrationMethod === 'fitted' ? fittedArea(doc, a, b) : area(doc, a, b);
  const areas = analysis.integrals.map(([a, b]) => selectedArea(a, b));
  const reference = analysis.reference;
  const referenceArea = areas[reference?.index ?? 0];
  const unit =
    referenceArea == null || referenceArea === 0 ? null : referenceArea / (reference?.value ?? 1);
  // As the app does: with no integrals the smallest multiplet (of those at least
  // a third of the median) is one nucleus; with integrals they share their scale.
  const multipletAreas = analysis.multiplets.map((m) => selectedArea(m.from, m.to));
  const sorted = multipletAreas
    .filter((a): a is number => a !== null && a > 0)
    .sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)];
  const multipletUnit = analysis.integrals.length
    ? unit
    : (sorted.find((a) => a >= median / 3) ?? Infinity);
  const multipletInfo: MultipletInfo[] = analysis.multiplets.map((m, k) => {
    const lines = maxima(doc, m.from, m.to, noise * 20);
    const ppms = lines.map((k) => at(doc, k));
    const gaps = ppms.slice(1).map((p, i) => Math.abs(p - ppms[i]) * doc.mhz);
    const j = gaps.length ? gaps.reduce((s, g) => s + g, 0) / gaps.length : 0;
    return {
      ppm: ppms.length ? ppms.reduce((s, p) => s + p, 0) / ppms.length : (m.from + m.to) / 2,
      kind: KINDS[lines.length - 1] ?? 'm',
      jsHz: j ? [Math.round(j * 100) / 100] : [],
      lines: lines.length,
      value:
        multipletAreas[k] == null || multipletUnit == null
          ? null
          : multipletAreas[k] / multipletUnit,
      widthHz: 0.9,
      explained: lines.length > 1 ? 0.97 : null,
    };
  });
  let [min, max] = [Infinity, -Infinity];
  for (const y of doc.real) [min, max] = [Math.min(min, y), Math.max(max, y)];
  return {
    id,
    name: doc.name,
    path: doc.path,
    nucleus: doc.nucleus,
    solvent: doc.solvent,
    frequencyMhz: doc.mhz,
    points: doc.points,
    firstPpm: doc.first,
    deltaPpm: doc.delta,
    min,
    max,
    noise,
    revision: doc.revision,
    operations: doc.state.operations,
    analysis,
    integralValues: areas.map((v) => (v === null || unit === null ? null : v / unit)),
    peakIntensities: analysis.peaks.map((p) => doc.real[idx(doc, p)] ?? 0),
    peakWidths: analysis.peaks.map((p) => widthHz(doc, idx(doc, p))),
    multipletInfo,
    canUndo: doc.undo.length > 0,
    canRedo: doc.redo.length > 0,
  };
}

function decimate(data: ArrayLike<number>, start: number, end: number, columns: number) {
  const n = Math.max(0, Math.min(end, data.length) - start);
  if (n <= 2 * columns) {
    const out = new Float32Array(2 * n);
    for (let i = 0; i < n; i++) [out[2 * i], out[2 * i + 1]] = [i, data[start + i]];
    return out.buffer;
  }
  const out = new Float32Array(4 * columns);
  for (let c = 0; c < columns; c++) {
    const [a, b] = [Math.floor((c * n) / columns), Math.floor(((c + 1) * n) / columns)];
    let [lo, hi] = [a, a];
    for (let i = a; i < b; i++) {
      if (data[start + i] < data[start + lo]) lo = i;
      if (data[start + i] > data[start + hi]) hi = i;
    }
    const [f, s] = [Math.min(lo, hi), Math.max(lo, hi)];
    out.set([f, data[start + f], s, data[start + s]], 4 * c);
  }
  return out.buffer;
}

function change(id: number, next: Doc['state'], amend = false, preserveFit = false) {
  const doc = docs[id];
  if (!doc) throw new Error(`No document ${id}`);
  if (!amend || doc.undo.length === 0) doc.undo.push(doc.state);
  doc.redo = [];
  const changedPeaks =
    JSON.stringify(next.analysis.peaks) !== JSON.stringify(doc.state.analysis.peaks);
  const changedOperations =
    JSON.stringify(next.operations) !== JSON.stringify(doc.state.operations);
  doc.state =
    !preserveFit && (changedPeaks || changedOperations)
      ? {
          ...next,
          analysis: { ...next.analysis, peakFit: null },
        }
      : next;
  process(doc);
  return info(id);
}

/** Steps of the made-up titration; `?spectra=17` on the preview's address makes
 *  that many, to see a tall stack. */
const TITRATION = Array.from(
  { length: Number(new URLSearchParams(location.search).get('spectra') ?? 3) || 3 },
  (_, k) => k * 0.03,
);

function add(
  doc: Omit<Doc, 'real' | 'first' | 'state' | 'undo' | 'redo' | 'revision'>,
  baseline: boolean,
) {
  const existing = docs.findIndex((d) => d?.path === doc.path);
  if (existing >= 0) return info(existing);
  const full: Doc = {
    ...doc,
    real: doc.re,
    first: doc.origin,
    state: { operations: [], analysis: emptyAnalysis() },
    undo: [],
    redo: [],
    revision: 0,
  };
  full.state.operations = defaultOps(full, baseline);
  process(full);
  docs.push(full);
  return info(docs.length - 1);
}

function made(path: string, baseline: boolean) {
  const steps = path.includes('titration') ? TITRATION : [0];
  return steps.map((shift, k) => {
    const fitFixture = path.includes('deconvolution');
    const { re, im } = synthesise(shift, 7 + k, fitFixture);
    const created = add(
      {
        name: `${path.split('/').filter(Boolean).pop() ?? 'demo'} / ${k + 1}`,
        path: `${path}/${k + 1}`,
        nucleus: '1H',
        solvent: 'DMSO',
        mhz: MHZ,
        points: POINTS,
        origin: FIRST_PPM,
        delta: DELTA,
        re,
        im,
        baseline: Float64Array.from({ length: POINTS }, (_, i) => roll(i)),
        recorded: RECORDED,
        lb: 0.3,
        size: null,
        conjugate: true,
        fitFixture,
      },
      baseline,
    );
    const doc = docs[created.id];
    if (fitFixture && doc && doc.state.analysis.peaks.length === 0) {
      doc.state.analysis = {
        ...emptyAnalysis(),
        peaks: FIT_SIGNALS.map((s) => s.ppm),
        labels: [null, null, null],
        integrals: [
          [6.16, 6.04],
          [5.26, 5.14],
        ],
        peakMethod: 'deconvolution',
        integrationMethod: 'fitted',
      };
    }
    return info(created.id);
  });
}

interface Written {
  name: string;
  path: string;
  nucleus: string;
  solvent: string | null;
  mhz: number;
  first: number;
  delta: number;
  phase: [number, number];
  lb: number | null;
  size: number;
  conjugate: boolean;
  re: number[];
  im: number[];
  baseline: number[];
}

async function real(file: string, baseline: boolean) {
  const w = (await (await fetch(`/preview-data/${file}`)).json()) as Written;
  return add(
    {
      name: w.name,
      path: w.path,
      nucleus: w.nucleus,
      solvent: w.solvent,
      mhz: w.mhz,
      points: w.re.length,
      origin: w.first,
      delta: w.delta,
      re: Float64Array.from(w.re),
      im: Float64Array.from(w.im),
      baseline: Float64Array.from(w.baseline),
      recorded: { p0: w.phase[0], p1: w.phase[1] },
      lb: w.lb,
      size: w.size,
      conjugate: w.conjugate,
    },
    baseline,
  );
}

/** 2D experiments as the preview_data example writes them, by id; they take a
 *  slot in `docs` too, empty, so ids are shared as the app's are. */
const twoD = new Map<number, { info: Document2DInfo; real: Float32Array }>();

interface Written2D {
  kind: '2d';
  name: string;
  path: string;
  nuclei: [string, string];
  solvent: string | null;
  mhz: number;
  mhz1: number;
  points: [number, number];
  first: [number, number];
  delta: [number, number];
  noise: number;
  max: number;
  signed: boolean;
  data: string;
}

async function real2d(w: Written2D): Promise<Document2DInfo> {
  const known = [...twoD.values()].find((d) => d.info.path === w.path);
  if (known) return known.info;
  const real = new Float32Array(await (await fetch(`/preview-data/${w.data}`)).arrayBuffer());
  const info: Document2DInfo = {
    id: docs.length,
    name: w.name,
    path: w.path,
    nuclei: w.nuclei,
    solvent: w.solvent,
    frequenciesMhz: [w.mhz, w.mhz1],
    points: w.points,
    firstPpm: w.first,
    deltaPpm: w.delta,
    noise: w.noise,
    max: w.max,
    signed: w.signed,
    peaks: [],
    canUndo: false,
    canRedo: false,
  };
  docs.push(undefined);
  twoD.set(info.id, { info, real });
  return info;
}

/** Marching squares as `nmr_core::contour::segments` does it, saddles by the
 *  mean of the cell. */
function segments(z: Float32Array, cols: number, level: number): number[] {
  const out: number[] = [];
  const rows = Math.floor(z.length / cols);
  for (let r = 0; r + 1 < rows; r++) {
    for (let c = 0; c + 1 < cols; c++) {
      const a = z[r * cols + c];
      const b = z[r * cols + c + 1];
      const d = z[(r + 1) * cols + c];
      const e = z[(r + 1) * cols + c + 1];
      const k = +(a >= level) | (+(b >= level) << 1) | (+(e >= level) << 2) | (+(d >= level) << 3);
      if (k === 0 || k === 15) continue;
      const t = (p: number, q: number) => (level - p) / (q - p);
      const top = [c + t(a, b), r];
      const right = [c + 1, r + t(b, e)];
      const bottom = [c + t(d, e), r + 1];
      const left = [c, r + t(a, d)];
      const above = (a + b + d + e) / 4 >= level;
      const pairs: Record<number, number[][]> = {
        1: [left, top],
        14: [left, top],
        2: [top, right],
        13: [top, right],
        3: [left, right],
        12: [left, right],
        4: [right, bottom],
        11: [right, bottom],
        6: [top, bottom],
        9: [top, bottom],
        7: [left, bottom],
        8: [left, bottom],
        5: above ? [top, right, left, bottom] : [left, top, right, bottom],
        10: above ? [left, top, right, bottom] : [top, right, left, bottom],
      };
      for (const p of pairs[k]) out.push(p[0], p[1]);
    }
  }
  return out;
}

/** Cross peaks and their history, as `src-tauri` keeps them. */
const peakHistory = new Map<number, { undo: CrossPeak[][]; redo: CrossPeak[][] }>();

function setPeaks(id: number, peaks: CrossPeak[], record = true) {
  const d = twoD.get(id);
  if (!d) throw new Error('No such 2D experiment');
  const h = peakHistory.get(id) ?? { undo: [], redo: [] };
  if (record) {
    h.undo.push(d.info.peaks);
    h.redo = [];
  }
  peakHistory.set(id, h);
  d.info = { ...d.info, peaks, canUndo: h.undo.length > 0, canRedo: h.redo.length > 0 };
  return d.info;
}

function stepPeaks(id: number, back: boolean) {
  const d = twoD.get(id);
  const h = peakHistory.get(id);
  if (!d || !h) throw new Error('No such 2D experiment');
  const [from, to] = back ? [h.undo, h.redo] : [h.redo, h.undo];
  const next = from.pop();
  if (next) to.push(d.info.peaks);
  return setPeaks(id, next ?? d.info.peaks, false);
}

function contours(id: number, levels: number[]) {
  const d = twoD.get(id);
  if (!d) throw new Error('No such 2D experiment');
  const lines = levels.map((level) => segments(d.real, d.info.points[0], level));
  return new Float32Array([...lines.map((l) => l.length), ...lines.flat()]).buffer;
}

async function open(path: string, baseline: boolean) {
  if (path.startsWith('/preview-data/')) {
    const file = path.slice('/preview-data/'.length);
    const w = (await (await fetch(`/preview-data/${file}`)).json()) as Written | Written2D;
    if ('kind' in w && w.kind === '2d') {
      return { documents: [], documents2d: [await real2d(w)], errors: [] };
    }
    return { documents: [await real(file, baseline)], documents2d: [], errors: [] };
  }
  return { documents: made(path, baseline), documents2d: [], errors: [] };
}

const metadata = (d: Doc): [string, string][] => [
  ['Nucleus', d.nucleus],
  ['Solvent', d.solvent ?? '—'],
  ['Spectrometer frequency', `${d.mhz.toFixed(2)} MHz`],
  ['Spectral width', `${(Math.abs(d.delta) * d.points * d.mhz).toFixed(1)} Hz`],
  ['Points', d.points.toLocaleString('en-US')],
];

type Args = Record<string, unknown>;
const num = (a: Args, k: string) => a[k] as number;

/** Answers `invoke(cmd, args)` as the Rust commands in `src-tauri` would; `open`
 *  and the folder dialog answer with a promise, since real spectra are fetched. */
export function handle(cmd: string, a: Args): unknown {
  const doc = () => {
    const d = docs[num(a, 'id')];
    if (!d) throw new Error('No such document');
    return d;
  };
  switch (cmd) {
    case 'open':
      return open(a.path as string, Boolean(a.baseline));
    case 'set_operations':
      return change(num(a, 'id'), { ...doc().state, operations: a.operations as Operation[] });
    case 'set_analysis':
      return change(
        num(a, 'id'),
        { ...doc().state, analysis: a.analysis as Analysis },
        a.amend === true,
      );
    case 'set_state':
      return change(
        num(a, 'id'),
        {
          operations: a.operations as Operation[],
          analysis: a.analysis as Analysis,
        },
        false,
        true,
      );
    case 'undo':
    case 'redo': {
      const d = doc();
      const [from, to] = cmd === 'undo' ? [d.undo, d.redo] : [d.redo, d.undo];
      const state = from.pop();
      if (state) {
        to.push(d.state);
        d.state = state;
        process(d);
      }
      return info(num(a, 'id'));
    }
    case 'auto_peaks': {
      const d = doc();
      const method = a.method as PeakMethod;
      const picked = maxima(d, d.first, at(d, d.points - 1), num(a, 'threshold')).map((k) =>
        at(d, k),
      );
      const peaks =
        method === 'deconvolution' && d.fitFixture
          ? [
              ...picked.filter((ppm) => Math.abs(ppm - 6.098) > 0.04),
              ...FIT_SIGNALS.slice(0, 2)
                .filter((line) => line.height > num(a, 'threshold'))
                .map((line) => line.ppm),
            ].sort((x, y) => y - x)
          : picked;
      const labels = peaks.map((ppm) => {
        const i = d.state.analysis.peaks.findIndex((p) => Math.abs(p - ppm) < Math.abs(d.delta));
        return i >= 0 ? (d.state.analysis.labels[i] ?? null) : null;
      });
      return change(
        num(a, 'id'),
        {
          ...d.state,
          analysis: {
            ...d.state.analysis,
            peaks,
            labels,
            peakMethod: method,
            peakFit: method === 'deconvolution' ? fixtureFit(d, peaks) : null,
          },
        },
        false,
        true,
      );
    }
    case 'fit_peaks': {
      const d = doc();
      return change(
        num(a, 'id'),
        {
          ...d.state,
          analysis: { ...d.state.analysis, peakFit: fixtureFit(d, d.state.analysis.peaks) },
        },
        a.amend === true,
        true,
      );
    }
    case 'find_peaks': {
      const d = doc();
      return maxima(d, num(a, 'from'), num(a, 'to'), num(a, 'threshold')).map((k) => at(d, k));
    }
    case 'signal_regions':
      return regions(doc(), num(a, 'from'), num(a, 'to'));
    case 'nearest_peak': {
      const d = doc();
      let k = idx(d, num(a, 'ppm'));
      const radius = num(a, 'radius');
      let best = k;
      for (let i = k - radius; i <= k + radius; i++) if (d.real[i] > d.real[best]) best = i;
      k = best;
      return k >= 0 && k < d.points ? at(d, k) : null;
    }
    case 'integral_curve': {
      const d = doc();
      const [i, j] = range(d, num(a, 'from'), num(a, 'to'));
      const samples = Math.max(1, Math.min(num(a, 'samples'), j - i + 1));
      const out = new Float32Array(2 * samples);
      let running = 0;
      let next = 0;
      for (let k = i; k <= j && next < samples; k++) {
        running += d.real[k] * Math.abs(d.delta);
        if (k >= i + Math.floor(((j - i) * next) / Math.max(1, samples - 1))) {
          out.set([at(d, k), running], 2 * next++);
        }
      }
      return out.buffer;
    }
    case 'trace':
      return decimate(doc().real, num(a, 'start'), num(a, 'end'), num(a, 'columns'));
    case 'phase_trace': {
      // The trial phase in place of the phase step, and the steps after it
      // replayed, as the back end does.
      const d = doc();
      const step = { op: 'phase', ...(a.phase as PhaseParams) } as Operation;
      const ops = d.state.operations.some((o) => o.op === 'phase')
        ? d.state.operations.map((o) => (o.op === 'phase' ? step : o))
        : [step, ...d.state.operations];
      return decimate(realOf(d, ops), num(a, 'start'), num(a, 'end'), num(a, 'columns'));
    }
    case 'baseline_trace': {
      const d = doc();
      const real = realOf(d, d.state.operations, (a.baseline as Operation | null) ?? null);
      return decimate(real, num(a, 'start'), num(a, 'end'), num(a, 'columns'));
    }
    case 'hand_set_baseline': {
      const d = doc();
      const op = a.baseline as Extract<Operation, { op: 'baseline_points' }>;
      const { values, curve, top } = handSet(realOf(d, d.state.operations, null), op);
      const [start, end, samples] = [num(a, 'start'), num(a, 'end'), num(a, 'samples')];
      return {
        points: values.map(([i, v]) => [v, curve[i]]),
        curve: Array.from(
          { length: samples },
          (_, k) => curve[Math.round(start + ((end - start) * k) / (samples - 1))],
        ),
        top,
      };
    }
    case 'propose_baseline_points':
      return Array.from({ length: 19 }, (_, k) => (k + 0.5) / 19);
    case 'largest_peak': {
      const d = doc();
      let best = 0;
      for (let k = 0; k < d.points; k++) {
        if (Math.hypot(d.re[k], d.im[k]) > Math.hypot(d.re[best], d.im[best])) best = k;
      }
      return best / d.points;
    }
    case 'auto_phase':
      return { ...(a.current as PhaseParams), ...doc().recorded, pivot: 0 };
    case 'metadata': {
      const d = twoD.get(num(a, 'id'));
      if (d) {
        return [
          ['Nuclei', d.info.nuclei.join(', ')],
          ['Solvent', d.info.solvent ?? '—'],
          ['Points', d.info.points.join(' × ')],
        ];
      }
      return metadata(doc());
    }
    case 'contours':
      return contours(num(a, 'id'), a.levels as number[]);
    // The top is the core's to find; here the point clicked stands for it.
    case 'nearest_peak_2d':
      return a.at;
    case 'projection_2d': {
      const d = twoD.get(num(a, 'id'));
      if (!d) throw new Error('No such 2D experiment');
      const [cols, rows] = d.info.points;
      const along = num(a, 'dim') === 0;
      const values = Array.from({ length: along ? cols : rows }, (_, i) => {
        let most = 0;
        for (let j = 0; j < (along ? rows : cols); j++) {
          most = Math.max(most, Math.abs(d.real[along ? j * cols + i : i * cols + j]));
        }
        return most;
      });
      return decimate(values, num(a, 'start'), num(a, 'end'), num(a, 'columns'));
    }
    case 'set_peaks_2d':
      return setPeaks(num(a, 'id'), a.peaks as CrossPeak[]);
    case 'undo_2d':
      return stepPeaks(num(a, 'id'), true);
    case 'redo_2d':
      return stepPeaks(num(a, 'id'), false);
    case 'fit_multiplet': {
      // The real fit is in nmr_core::fit; here the reading stands in for it.
      const found = info(num(a, 'id')).multipletInfo[num(a, 'index')];
      return found && found.kind !== 'm'
        ? { ppm: found.ppm, jsHz: found.jsHz, widthHz: found.widthHz }
        : null;
    }
    case 'close':
      docs[num(a, 'id')] = undefined;
      twoD.delete(num(a, 'id'));
      return null;
    case 'read_project':
      throw new Error('Projects cannot be opened in the preview');
    case 'system_fonts':
      return ['Arial', 'Georgia', 'Helvetica', 'Helvetica Neue', 'Menlo', 'Times New Roman'];
    case 'project_layout':
      return (a.ids as number[]).map((_, k) => `spectra/${k}`);
    case 'save_project':
      return null;
    // Kept across a reload, which is what a crash looks like from the page.
    case 'keep_session':
      if (a.manifest) localStorage.setItem('previewSession', a.manifest as string);
      else localStorage.removeItem('previewSession');
      return null;
    case 'last_session':
      return localStorage.getItem('previewSession');
    case 'save_pdf':
      // Kept where a check of the preview can look at what would be written.
      (globalThis as { previewPages?: unknown }).previewPages = a.pages;
      return null;
    case 'read_molecule':
      // Ethanol, whatever was pasted: the real reading is the crate's.
      return {
        formula: 'C2H6O',
        atoms: [
          { symbol: 'C', x: 0, y: 0, charge: 0, h: 3, class: 0 },
          { symbol: 'C', x: 1.3, y: 0.75, charge: 0, h: 2, class: 1 },
          { symbol: 'O', x: 2.6, y: 0, charge: 0, h: 1, class: 2 },
        ],
        bonds: [
          { a: 0, b: 1, order: 1, stereo: 0 },
          { a: 1, b: 2, order: 1, stereo: 0 },
        ],
      };
    case 'read_bytes':
    case 'clipboard_structure':
      return null;
    case 'save_file':
      return null;
    case 'plugin:dialog|open':
      return (a.options as { directory?: boolean })?.directory
        ? REAL.then((files) =>
            new URLSearchParams(location.search).get('fixture') === 'deconvolution'
              ? ['/demo/deconvolution']
              : files.length && !location.search.includes('spectra')
                ? files.map((f) => `/preview-data/${f}`)
                : ['/demo/titration'],
          )
        : '/demo/demo.opennmr';
    case 'plugin:dialog|save':
      return '/demo/saved';
    default:
      throw new Error(`The preview has no stand-in for ${cmd}`);
  }
}
