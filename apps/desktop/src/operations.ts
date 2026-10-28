import type { Operation, PhaseParams } from './api';

export type BaselineOperation = Extract<
  Operation,
  | { op: 'baseline_airpls' }
  | { op: 'baseline_whittaker' }
  | { op: 'baseline_polynomial' }
  | { op: 'baseline_points' }
>;

export type PhaseOperation = Extract<Operation, { op: 'phase' }>;

export const findPhase = (ops: Operation[]) =>
  ops.find((o): o is PhaseOperation => o.op === 'phase');

/** Swaps step `index` with its neighbour `by` positions away. */
export function moveOperation(ops: Operation[], index: number, by: -1 | 1): Operation[] {
  const target = index + by;
  if (target < 0 || target >= ops.length) return ops;
  const moved = [...ops];
  [moved[index], moved[target]] = [moved[target], moved[index]];
  return moved;
}

/** Sets a numeric parameter of step `index`; sizes are whole numbers of at least 1. */
export function setParameter(
  ops: Operation[],
  index: number,
  key: string,
  value: number,
): Operation[] {
  const v = key === 'size' ? Math.max(1, Math.round(value)) : value;
  return ops.map((op, i) => (i === index ? ({ ...op, [key]: v } as Operation) : op));
}

const WINDOWS: Operation['op'][] = ['exponential', 'gaussian', 'sine_bell'];

/**
 * `ops` with `step` where it can run: a step on the FID before the Fourier
 * transform (DC offset, then the window, then zero filling, then the transform)
 * and a step on the spectrum right after it. Put in front of the phase instead,
 * a step on the FID landed after the transform, where it cannot run.
 */
export function withStep(ops: Operation[], step: Operation): Operation[] {
  const ft = ops.findIndex((o) => o.op === 'ft');
  if (ft < 0) return [...ops, step];
  const before = (kinds: Operation['op'][]) => {
    const at = ops.findIndex((o, i) => i <= ft && kinds.includes(o.op));
    return at < 0 ? ft : at;
  };
  const at =
    step.op === 'zero_fill'
      ? ft
      : WINDOWS.includes(step.op)
        ? before(['zero_fill', 'ft'])
        : step.op === 'dc_offset'
          ? before([...WINDOWS, 'zero_fill', 'ft'])
          : ft + 1;
  return [...ops.slice(0, at), step, ...ops.slice(at)];
}

/** Replaces the phase step, or inserts it right after the Fourier transform. */
export function withPhase(ops: Operation[], phase: PhaseParams): Operation[] {
  const step: PhaseOperation = { op: 'phase', ...phase };
  const index = ops.findIndex((o) => o.op === 'phase');
  if (index >= 0) return ops.map((o, i) => (i === index ? step : o));
  const ft = ops.findIndex((o) => o.op === 'ft');
  return ft < 0 ? [...ops, step] : [...ops.slice(0, ft + 1), step, ...ops.slice(ft + 1)];
}

export const DEFAULT_AIRPLS: BaselineOperation = { op: 'baseline_airpls', lambda: 1e7 };
/** Local defaults, matching the Rust pipeline. */
export const DEFAULT_WHITTAKER: BaselineOperation = {
  op: 'baseline_whittaker',
  lambda: 1e5,
  filter_hz: 7.3,
};
export const DEFAULT_POLYNOMIAL: BaselineOperation = { op: 'baseline_polynomial', order: 5 };

export const isBaseline = (o: Operation): o is BaselineOperation => o.op.startsWith('baseline_');

export type PointsOperation = Extract<Operation, { op: 'baseline_points' }>;

/** A hand-set baseline with no points yet, joined by a cubic spline; the
 *  order and smoothness are kept for when the user switches to those. */
export const DEFAULT_POINTS: PointsOperation = {
  op: 'baseline_points',
  points: [],
  through: 'spline',
  order: 3,
  lambda: 1e5,
  span: 5,
};

export const THROUGH_NAMES: Record<PointsOperation['through'], string> = {
  segments: 'Linear segments',
  spline: 'Cubic splines',
  polynomial: 'Polynomial',
  whittaker: 'Whittaker smoother',
};

/** Replaces the existing baseline step, or appends one. */
export function withBaseline(ops: Operation[], baseline: BaselineOperation): Operation[] {
  const index = ops.findIndex(isBaseline);
  return index < 0 ? [...ops, baseline] : ops.map((o, i) => (i === index ? baseline : o));
}

const num = (n: number) => n.toLocaleString('en-US', { maximumSignificantDigits: 4 });

/** Short label for the processing list. */
export function describe(o: Operation): string {
  switch (o.op) {
    case 'conjugate':
      return 'Mirror (conjugate FID)';
    case 'remove_digital_filter':
      return 'Digital filter removal';
    case 'digital_filter':
      return 'Digital filter removal, as a phase';
    case 'dc_offset':
      return 'DC offset correction';
    case 'exponential':
      return `Exponential window, ${num(o.lb_hz)} Hz`;
    case 'gaussian':
      return `Gaussian window, ${num(o.g1_hz)} / ${num(o.g2_hz)} Hz`;
    case 'sine_bell':
      return `${o.power === 2 ? 'Squared sine-bell' : 'Sine-bell'}, shift ${num(o.offset)}`;
    case 'zero_fill':
      return `Zero fill to ${o.size.toLocaleString('en-US')} points`;
    case 'linear_prediction':
      return `Linear prediction to ${o.size.toLocaleString('en-US')} points, order ${o.order}`;
    case 'ft':
      return 'Fourier transform';
    case 'phase':
      return `Phase ${num(o.p0)}°, ${num(o.p1)}°`;
    case 'magnitude':
      return 'Magnitude';
    case 'power':
      return 'Power';
    case 'reverse':
      return 'Reverse';
    case 'reference':
      return `Reference ${num(o.from_ppm)} → ${num(o.to_ppm)} ppm`;
    case 'baseline_airpls':
      return `Baseline: airPLS, λ ${o.lambda.toExponential(1)}`;
    case 'baseline_whittaker':
      return `Baseline: Whittaker, λ ${o.lambda.toExponential(1)}, filter ${num(o.filter_hz)} Hz${o.auto_filter === false ? ' (manual)' : ''}`;
    case 'baseline_polynomial':
      return `Baseline: polynomial, order ${o.order}`;
    case 'baseline_points':
      return `Baseline: ${o.points.length} points, ${THROUGH_NAMES[o.through].toLowerCase()}`;
  }
}

/** Names the peak at `fromPpm` as `toPpm`, replacing any earlier reference. */
export function withReference(ops: Operation[], fromPpm: number, toPpm: number): Operation[] {
  const step: Operation = { op: 'reference', from_ppm: fromPpm, to_ppm: toPpm };
  const index = ops.findIndex((o) => o.op === 'reference');
  if (index >= 0) return ops.map((o, i) => (i === index ? step : o));
  const ft = ops.findIndex((o) => o.op === 'ft');
  return ft < 0 ? [...ops, step] : [...ops.slice(0, ft + 1), step, ...ops.slice(ft + 1)];
}
