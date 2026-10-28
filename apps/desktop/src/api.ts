import { invoke } from '@tauri-apps/api/core';

/** Tauri's postMessage fallback returns bytes as a JSON array on macOS. */
async function invokeBuffer(command: string, args: Record<string, unknown>): Promise<ArrayBuffer> {
  const bytes = await invoke<ArrayBuffer | number[]>(command, args);
  return bytes instanceof ArrayBuffer ? bytes : Uint8Array.from(bytes).buffer;
}

/** Mirrors `nmr_core::process::Operation`. */
export type Operation =
  | { op: 'conjugate' }
  | { op: 'remove_digital_filter' }
  | { op: 'digital_filter' }
  | { op: 'dc_offset' }
  | { op: 'exponential'; lb_hz: number }
  | { op: 'gaussian'; g1_hz: number; g2_hz: number; g3: number }
  | { op: 'sine_bell'; offset: number; end: number; power: number }
  | { op: 'zero_fill'; size: number }
  | { op: 'linear_prediction'; size: number; order: number }
  | { op: 'ft' }
  | { op: 'phase'; p0: number; p1: number; pivot: number }
  | { op: 'magnitude' }
  | { op: 'power' }
  | { op: 'reverse' }
  | { op: 'reference'; from_ppm: number; to_ppm: number }
  | { op: 'baseline_airpls'; lambda: number }
  | { op: 'baseline_whittaker'; lambda: number; filter_hz: number; auto_filter?: boolean }
  | { op: 'baseline_polynomial'; order: number }
  | {
      op: 'baseline_points';
      points: BaselinePoint[];
      through: Through;
      order: number;
      lambda: number;
      span: number;
    };

/** A point of a hand-set baseline: `at` a fraction of the spectrum, and for a
 * point placed freely `y`, a fraction of the largest absolute value. */
export interface BaselinePoint {
  at: number;
  y?: number;
}

/** How a hand-set baseline joins its points. */
export type Through = 'segments' | 'spline' | 'polynomial' | 'whittaker';

export type PeakMethod = 'standard' | 'deconvolution';
export type IntegrationMethod = 'sum' | 'fitted';

export interface PeakFit {
  source?: string;
  peaks: { ppm: number; height: number; widthHz: number; area: number; shape?: number }[];
  regions: {
    from: number;
    to: number;
    status: 'converged' | 'unresolved' | 'failed';
    rms: number | null;
    reason?:
      | 'invalidInput'
      | 'belowThreshold'
      | 'componentLimit'
      | 'insufficientSamples'
      | 'optimizer'
      | 'degenerate'
      | 'residual';
  }[];
}

/** Integrals (ppm ranges), their normalisation and picked peaks (ppm). */
export interface Analysis {
  integrals: [number, number][];
  /** Integral `index` is shown as `value`; otherwise the first integral is 1. */
  reference: { index: number; value: number } | null;
  peaks: number[];
  /** One per peak: null follows the impurity table, '' shows nothing, and any
   * other text is what the peak has been called. */
  labels: (string | null)[];
  /** Ranges read as multiplets. */
  multiplets: MultipletRange[];
  /** Missing in older projects: standard picking and sum integration. */
  peakMethod?: PeakMethod;
  integrationMethod?: IntegrationMethod;
  peakFit?: PeakFit | null;
}

/** A range read as one multiplet; a name or a count given by hand wins over what
 * the spectrum says. */
export interface MultipletRange {
  from: number;
  to: number;
  name?: string | null;
  nuclei?: number | null;
  /** Where it is quoted; null reads it off the lines again. */
  ppm?: number | null;
  /** A class typed over what the lines read as; null reads them again. */
  kind?: string | null;
  /** Couplings typed over the ones measured, in Hz; null measures them again. */
  jsHz?: number[] | null;
  /** The width a simulated line is drawn with, in Hz; null uses the one shown. */
  widthHz?: number | null;
  /** Whether the shape it would have is drawn over the signal, this one alone. */
  shape?: boolean | null;
  /** The atoms of the structure this signal is assigned to, by position. */
  atoms?: number[];
}

/** What a multiplet reads as, worked out from the spectrum. */
export interface MultipletInfo {
  ppm: number;
  /** s, d, t, q, quint, sext, sept, dd, or m. */
  kind: string;
  jsHz: number[];
  lines: number;
  /** The range's integral, on the same scale as the integrals table. */
  value: number | null;
  /** How wide its lines are, in Hz. */
  widthHz: number;
  /** Share of the signal its lines account for, 0 to 1; null for an m. */
  explained: number | null;
}

export interface DocumentInfo {
  id: number;
  name: string;
  path: string;
  nucleus: string;
  solvent: string | null;
  /** Spectrometer frequency for this nucleus, in MHz. */
  frequencyMhz: number;
  points: number;
  firstPpm: number;
  deltaPpm: number;
  min: number;
  max: number;
  noise: number;
  revision: number;
  operations: Operation[];
  analysis: Analysis;
  /** Null means the selected area method could not quantify this range. */
  integralValues: (number | null)[];
  peakIntensities: number[];
  /** How wide each picked line is, in Hz, in the order of `analysis.peaks`. */
  peakWidths: number[];
  multipletInfo: MultipletInfo[];
  canUndo: boolean;
  canRedo: boolean;
}

/** A 2D experiment: both axes, F2 first. */
export interface Document2DInfo {
  id: number;
  name: string;
  path: string;
  nuclei: [string, string];
  solvent: string | null;
  /** Spectrometer frequency of F2's nucleus and of F1's, in MHz. */
  frequenciesMhz: [number, number];
  /** Points along F2 and F1. */
  points: [number, number];
  firstPpm: [number, number];
  deltaPpm: [number, number];
  noise: number;
  /** Largest absolute value. */
  max: number;
  /** Peaks can point down (phase-sensitive); in magnitude mode they cannot. */
  signed: boolean;
  peaks: CrossPeak[];
  canUndo: boolean;
  canRedo: boolean;
}

/** A cross peak picked on a 2D, and the atoms of the structure it is assigned
 *  to, by position in the molecule of the ¹H spectrum beside it. */
export interface CrossPeak {
  /** F2 and F1, in ppm. */
  at: [number, number];
  atoms: number[];
}

/** The cross peak nearest `at` (`[F2, F1]` in ppm), within `radius` points
 *  along F2 and along F1. */
export const nearestPeak2d = (id: number, at: [number, number], radius: [number, number]) =>
  invoke<[number, number] | null>('nearest_peak_2d', { id, at, radius });

/** Replaces a 2D's cross peaks, as one step to undo. */
export const setPeaks2d = (id: number, peaks: CrossPeak[]) =>
  invoke<Document2DInfo>('set_peaks_2d', { id, peaks });

export const undo2d = (id: number) => invoke<Document2DInfo>('undo_2d', { id });

export const redo2d = (id: number) => invoke<Document2DInfo>('redo_2d', { id });

export interface Opened {
  documents: DocumentInfo[];
  documents2d: Document2DInfo[];
  errors: string[];
}

/** Phase in degrees; `p0` is the phase at `pivot` (fraction of the spectrum). */
export interface PhaseParams {
  p0: number;
  p1: number;
  pivot: number;
}

/** `phase` works a phase out only when the dataset carries none of its own; the
 * one the spectrometer recorded is always applied. */
/** Opens a dataset; `baseline` is the correction to open with, or null for none. */
export const openPath = (path: string, phase: boolean, baseline: Operation | null) =>
  invoke<Opened>('open', { path, phase, baseline });

export const setOperations = (id: number, operations: Operation[]) =>
  invoke<DocumentInfo>('set_operations', { id, operations });

/** `amend` makes it part of the last change, undone together with it. */
export const setAnalysis = (id: number, analysis: Analysis, amend = false) =>
  invoke<DocumentInfo>('set_analysis', { id, analysis, amend });

/** Replaces the full automatic peak list and records its fitted components. */
export const autoPeaks = (id: number, threshold: number, method: PeakMethod) =>
  invoke<DocumentInfo>('auto_peaks', { id, threshold, method });

/** Fits only the accepted peak list; it does not discover additional peaks. */
export const fitPeaks = (id: number, threshold: number, amend = false) =>
  invoke<DocumentInfo>('fit_peaks', { id, threshold, amend });

export const findPeaks = (id: number, from: number, to: number, threshold: number) =>
  invoke<number[]>('find_peaks', { id, from, to, threshold });

/** Regions worth a look, from the same threshold that picks peaks: a signal is
 * what both an integral and a multiplet are drawn around. */
export const signalRegions = (id: number, from: number, to: number, threshold: number) =>
  invoke<[number, number][]>('signal_regions', { id, from, to, threshold });

export const nearestPeak = (id: number, ppm: number, radius: number) =>
  invoke<number | null>('nearest_peak', { id, ppm, radius });

/** Running integral as `ppm, value` pairs. */
export async function fetchIntegralCurve(id: number, from: number, to: number, samples: number) {
  return new Float32Array(await invokeBuffer('integral_curve', { id, from, to, samples }));
}

/** Replaces operations and analysis together as one undo step. */
export const setState = (id: number, operations: Operation[], analysis: Analysis) =>
  invoke<DocumentInfo>('set_state', { id, operations, analysis });

/** A project to travel inside a PDF, as `writeProject` takes one. */
export interface Attachment {
  manifest: string;
  ids: number[];
  extras: { name: string; dataUrl: string }[];
}

/** Writes SVG `pages` as one PDF, with `project` attached when given. */
export const savePdf = (path: string, pages: string[], project: Attachment | null) =>
  invoke<null>('save_pdf', { path, pages, project });

/** The font families installed on this computer, by name. */
export const systemFonts = () => invoke<string[]>('system_fonts');

/** Where each document goes inside a project file, in the order given. */
export const projectLayout = (ids: number[]) => invoke<string[]>('project_layout', { ids });

/** Writes a project file with the data of documents `ids` in it; `extras` are
 * structure drawings, by their place in the archive. */
export const writeProject = (
  path: string,
  manifest: string,
  ids: number[],
  extras: { name: string; dataUrl: string }[],
) => invoke<null>('save_project', { path, manifest, ids, extras });

/** A project file's `project.json`, and where the data it carries was
 * unpacked; `root` is null for a project that points to data on disk. */
export const readProject = (path: string) =>
  invoke<{ manifest: string; root: string | null }>('read_project', { path });

/** Keeps the session to recover after a crash; null forgets it. */
export const keepSession = (manifest: string | null) => invoke<null>('keep_session', { manifest });

/** The session running when the app last stopped without closing, if any. */
export const lastSession = () => invoke<string | null>('last_session');

/** A drawing off the clipboard as a data URL, which the web view will not give
 * up itself: it hands a page no flavours at all when the copy came from another
 * application. */
export const clipboardStructure = () => invoke<string | null>('clipboard_structure');

/** A molecule as its molfile reads: heavy atoms with the hydrogens each carries,
 * and a class per atom - atoms of one class are equivalent by their bonds. */
export interface Molecule {
  atoms: { symbol: string; x: number; y: number; charge: number; h: number; class: number }[];
  bonds: { a: number; b: number; order: number; stereo: number }[];
  formula: string;
}

export const readMolecule = (text: string) => invoke<Molecule>('read_molecule', { text });

/** A file as it stands: a structure is a PDF or a picture, not text. */
export const readBytes = (path: string) => invokeBuffer('read_bytes', { path });

/** Writes `content` to `path`. The path travels in a header because the body is raw bytes. */
export const saveFile = (path: string, content: Uint8Array) =>
  invoke<null>('save_file', content, { headers: { path: encodeURIComponent(path) } });

/** What the dataset says about how it was recorded, label and value. */
export const fetchMetadata = (id: number) => invoke<[string, string][]>('metadata', { id });

/** Forgets a document, so reopening its folder reads the spectrum afresh. */
export const closeDocument = (id: number) => invoke<null>('close', { id });

export const undo = (id: number) => invoke<DocumentInfo>('undo', { id });

export const redo = (id: number) => invoke<DocumentInfo>('redo', { id });

/** Shift, couplings and line width of a multiplet fitted to the spectrum
 * together; null when it has no first-order pattern to fit. */
export const fitMultiplet = (id: number, index: number) =>
  invoke<{ ppm: number; jsHz: number[]; widthHz: number } | null>('fit_multiplet', { id, index });

export const largestPeak = (id: number) => invoke<number>('largest_peak', { id });

export const autoPhase = (id: number, current: PhaseParams) =>
  invoke<PhaseParams>('auto_phase', { id, current });

/** Like `fetchTrace`, but with a phase correction applied to the transformed spectrum. */
export async function fetchPhaseTrace(
  id: number,
  phase: PhaseParams,
  start: number,
  end: number,
  columns: number,
) {
  const args = { id, phase, start, end, columns };
  return new Float32Array(await invokeBuffer('phase_trace', args));
}

/** Like `fetchTrace`, with `baseline` in place of the baseline step, or none. */
export async function fetchBaselineTrace(
  id: number,
  baseline: Operation | null,
  start: number,
  end: number,
  columns: number,
) {
  const args = { id, baseline, start, end, columns };
  return new Float32Array(await invokeBuffer('baseline_trace', args));
}

/** A hand-set baseline as drawn while its points are set: for each point the
 * value it stands at and the baseline there, the curve at `samples` points
 * from `start` to `end`, and what a free point's `y` is a fraction of. */
export const handSetBaseline = (
  id: number,
  baseline: Operation,
  start: number,
  end: number,
  samples: number,
) =>
  invoke<{ points: [number, number][]; curve: number[]; top: number }>('hand_set_baseline', {
    id,
    baseline,
    start,
    end,
    samples,
  });

/** Points to start a hand-set baseline from, as fractions of the spectrum. */
export const proposeBaselinePoints = (id: number) =>
  invoke<number[]>('propose_baseline_points', { id });

/** Contour lines of a 2D at each of `levels`, one buffer per level of
 *  `x0, y0, x1, y1` segments in points along F2 and F1. */
export async function fetchContours(id: number, levels: number[]) {
  const all = new Float32Array(await invokeBuffer('contours', { id, levels }));
  let at = levels.length;
  return levels.map((_, k) => all.subarray(at, (at += all[k])));
}

/** The 2D's own projection along F2 (`dim` 0) or F1 (1), as `fetchTrace`
 *  gives a spectrum. */
export async function fetchProjection(
  id: number,
  dim: 0 | 1,
  start: number,
  end: number,
  columns: number,
) {
  const args = { id, dim, start, end, columns };
  return new Float32Array(await invokeBuffer('projection_2d', args));
}

/** Decimated `x, y` pairs for points `start..end`, with `x` relative to `start`. */
export async function fetchTrace(id: number, start: number, end: number, columns: number) {
  return new Float32Array(await invokeBuffer('trace', { id, start, end, columns }));
}
