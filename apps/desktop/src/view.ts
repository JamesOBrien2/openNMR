/** Visible window: ppm at the left and right edges (left is higher) and display intensity. */
export interface View {
  x0: number;
  x1: number;
  y0: number;
  y1: number;
}

/** ppm limits of the data, `left > right`. */
export interface Limits {
  left: number;
  right: number;
  minSpan: number;
  /** The vertical axis when it is in ppm too, as a 2D's F1 is, drawn high at
   * the foot: `left` is then `y0`'s end and `right` `y1`'s. A spectrum's
   * intensity has none, and is not held in. */
  y?: Omit<Limits, 'y'>;
}

export interface TraceRequest {
  start: number;
  end: number;
  columns: number;
}

export type Axis = 'x' | 'y' | 'xy';

export const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

export const ppmAt = (v: View, px: number, width: number) => v.x0 + (px / width) * (v.x1 - v.x0);

export const pxOfPpm = (v: View, ppm: number, width: number) =>
  ((ppm - v.x0) / (v.x1 - v.x0)) * width;

export const intensityAt = (v: View, py: number, height: number) =>
  v.y1 - (py / height) * (v.y1 - v.y0);

/** `[a0, a1]`, high end first, kept within `limits` and no narrower than they allow. */
function within(a0: number, a1: number, limits: Omit<Limits, 'y'>): [number, number] {
  const full = limits.left - limits.right;
  const span = clamp(a0 - a1, Math.min(limits.minSpan, full), full);
  const start = clamp(a0, limits.right + span, limits.left);
  return [start, start - span];
}

function clampView(v: View, limits: Limits): View {
  const [x0, x1] = within(v.x0, v.x1, limits);
  if (!limits.y) return { ...v, x0, x1 };
  const [y0, y1] = within(v.y0, v.y1, limits.y);
  return { x0, x1, y0, y1 };
}

export const fullX = (v: View, limits: Limits): View => ({
  ...v,
  x0: limits.left,
  x1: limits.right,
});

/** Zooms the ppm axis by `factor` keeping `anchor` (ppm) under the cursor. */
export function zoomX(v: View, anchor: number, factor: number, limits: Limits): View {
  const span = (v.x0 - v.x1) * factor;
  const x0 = anchor + ((v.x0 - anchor) / (v.x0 - v.x1)) * span;
  return clampView({ ...v, x0, x1: x0 - span }, limits);
}

/** Scales intensity by `factor` around `anchor`; held within `limits.y` when
 * the vertical axis has limits. */
export function zoomY(v: View, anchor: number, factor: number, limits?: Limits): View {
  const next = {
    ...v,
    y0: anchor - (anchor - v.y0) * factor,
    y1: anchor + (v.y1 - anchor) * factor,
  };
  return limits?.y ? clampView(next, limits) : next;
}

/** Moves the content by a screen offset in pixels. */
export function pan(
  v: View,
  dx: number,
  dy: number,
  width: number,
  height: number,
  limits: Limits,
): View {
  const dppm = (dx / width) * (v.x1 - v.x0);
  const dyv = (dy / height) * (v.y1 - v.y0);
  return clampView({ x0: v.x0 - dppm, x1: v.x1 - dppm, y0: v.y0 + dyv, y1: v.y1 + dyv }, limits);
}

/** Zooms to a dragged pixel rectangle along `axis`. */
export function zoomBox(
  v: View,
  box: { x0: number; y0: number; x1: number; y1: number },
  width: number,
  height: number,
  axis: Axis,
  limits: Limits,
): View {
  const next = { ...v };
  if (axis !== 'y') {
    next.x0 = ppmAt(v, Math.min(box.x0, box.x1), width);
    next.x1 = Math.min(ppmAt(v, Math.max(box.x0, box.x1), width), next.x0 - limits.minSpan);
  }
  if (axis !== 'x') {
    next.y0 = intensityAt(v, Math.max(box.y0, box.y1), height);
    next.y1 = intensityAt(v, Math.min(box.y0, box.y1), height);
  }
  return clampView(next, limits);
}

/** Zooms by `factor` around a pixel along `axis` (click-to-zoom). */
export function zoomAt(
  v: View,
  px: number,
  py: number,
  width: number,
  height: number,
  factor: number,
  axis: Axis,
  limits: Limits,
): View {
  let next = v;
  if (axis !== 'y') next = zoomX(next, ppmAt(v, px, width), factor, limits);
  if (axis !== 'x') next = zoomY(next, intensityAt(v, py, height), factor, limits);
  return next;
}

/** Intensity range `lo..hi` with 5 % padding, always including zero. */
const BOTTOM_MARGIN = 0.22;
const TOP_MARGIN = 0.05;
/** However many labels there are, half the plot stays with the spectrum. */
const MAX_RESERVE = 0.5;

/** `reserveTop` is the share of the height the peak labels stand in: the fit
 * keeps the highest point below it, so a tall line is not drawn through its own
 * label. Zero, the default, fits as before. */
/** `baselineAt`, when given, is where the baseline sits as a share of the
 * height up from the foot, whatever the signals are. */
export function fitY(v: View, lo: number, hi: number, reserveTop = 0, baselineAt?: number): View {
  if (!(hi > lo)) return v;
  const bottom = Math.min(lo, 0);
  const span = hi - bottom;
  // A wider margin below than above, so the baseline sits clear of the axis
  // rather than on it.
  const y0 = bottom - span * BOTTOM_MARGIN;
  const r = Math.min(Math.max(reserveTop, 0), MAX_RESERVE);
  // Solves (y1 - hi) / (y1 - y0) = r, which puts `hi` exactly under the band.
  if (baselineAt !== undefined && hi > 0) {
    // The baseline where it was asked to sit, as a share of the height up from
    // the foot, and the highest line under the labels: two conditions, which
    // settle the span and where it starts.
    const f = Math.min(Math.max(baselineAt, 0), 0.8);
    const span = hi / Math.max(1 - Math.max(r, TOP_MARGIN) - f, 0.05);
    return { ...v, y0: -f * span, y1: (1 - f) * span };
  }
  return { ...v, y0, y1: Math.max(hi + span * TOP_MARGIN, (hi - r * y0) / (1 - r)) };
}

/** The zooms that were asked for, which Shift with the arrows walks: `at` is
 * the one on screen. */
export interface History {
  views: View[];
  at: number;
}

export const historyOf = (v: View): History => ({ views: [v], at: 0 });

/** `h` with `v` after the one on screen, dropping any that had been stepped
 * back over, as an editor's undo does. */
export const record = (h: History, v: View): History => ({
  views: [...h.views.slice(0, h.at + 1), v],
  at: h.at + 1,
});

/** `h` stepped `by` places, as far as it goes. */
export const step = (h: History, by: number): History => ({
  ...h,
  at: clamp(h.at + by, 0, h.views.length - 1),
});

/** `h` with the one on screen changed in place, for what changes the view
 * without being a zoom that was asked for. */
export const amend = (h: History, v: View): History => ({
  ...h,
  views: h.views.map((entry, k) => (k === h.at ? v : entry)),
});

/** Lowest and highest y of a trace buffer between point indices `i0..i1`. */
export function extent(xy: Float32Array, start: number, i0: number, i1: number) {
  let lo = Infinity;
  let hi = -Infinity;
  for (let k = 0; k < xy.length; k += 2) {
    const index = start + xy[k];
    if (index < i0 || index > i1) continue;
    lo = Math.min(lo, xy[k + 1]);
    hi = Math.max(hi, xy[k + 1]);
  }
  return { lo, hi };
}

/** Round tick values between `a` and `b` (either order), at most about `maxTicks`. */
export function niceTicks(a: number, b: number, maxTicks: number) {
  const lo = Math.min(a, b);
  const hi = Math.max(a, b);
  if (!(hi > lo) || maxTicks < 1) return { values: [], decimals: 0 };
  const raw = (hi - lo) / maxTicks;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  // Tolerance so rounding noise in `raw` (0.08 / 4 = 0.020000000000000018) keeps step 0.02.
  const step =
    [1, 2, 5, 10].map((m) => m * magnitude).find((s) => s >= raw * (1 - 1e-9)) ?? 10 * magnitude;
  const decimals = Math.max(0, -Math.floor(Math.log10(step)));
  const values: number[] = [];
  for (let k = Math.ceil(lo / step - 1e-9); k * step <= hi + step * 1e-9; k++) {
    values.push(Number((k * step).toFixed(decimals)));
  }
  return { values, decimals };
}

/** Data to fetch for visible points `i0..i1`: one extra screen on each side so panning stays sharp. */
export function traceRequest(
  i0: number,
  i1: number,
  points: number,
  widthPx: number,
): TraceRequest {
  const span = i1 - i0;
  const start = clamp(Math.floor(i0 - span), 0, points);
  const end = clamp(Math.ceil(i1 + span) + 1, start, points);
  const columns = Math.max(1, Math.round((widthPx * (end - start)) / Math.max(span, 1)));
  return { start, end, columns };
}

/** Whether already loaded data still covers points `i0..i1` at a similar resolution. */
export function covers(have: TraceRequest, i0: number, i1: number, want: TraceRequest): boolean {
  if (have.start > Math.max(0, Math.floor(i0))) return false;
  if (have.end < Math.min(want.end, Math.ceil(i1) + 1)) return false;
  const density = (r: TraceRequest) => Math.max((r.end - r.start) / r.columns, 2);
  const ratio = density(have) / density(want);
  return ratio > 0.7 && ratio < 1.4;
}

/** Contour levels of a 2D from `lowest` up past nothing higher than `highest`,
 *  each `step` times the last, at most `count`: the geometric ladder a contour
 *  plot is read by, so a small peak and a large one both show. */
export function contourLevels(lowest: number, highest: number, step: number, count: number) {
  const levels: number[] = [];
  for (let level = lowest; level <= highest && levels.length < count; level *= step) {
    levels.push(level);
  }
  return levels;
}
