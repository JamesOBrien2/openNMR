import { describe as group, expect, it } from 'vitest';
import type { Operation } from './api';
import {
  DEFAULT_AIRPLS,
  DEFAULT_WHITTAKER,
  DEFAULT_POINTS,
  DEFAULT_POLYNOMIAL,
  describe,
  findPhase,
  moveOperation,
  setParameter,
  withBaseline,
  withPhase,
  withStep,
} from './operations';

const ops: Operation[] = [{ op: 'exponential', lb_hz: 0.3 }, { op: 'ft' }];

group('baseline through points', () => {
  it('replaces the automatic baseline and says how many points it has', () => {
    const points = { ...DEFAULT_POINTS, points: [{ at: 0.1 }, { at: 0.9, y: 0.02 }] };
    const ops: Operation[] = [{ op: 'ft' }, DEFAULT_WHITTAKER];
    expect(withBaseline(ops, points)).toEqual([{ op: 'ft' }, points]);
    expect(describe(points)).toBe('Baseline: 2 points, cubic splines');
  });
});

group('withBaseline', () => {
  it('appends when there is no baseline step', () => {
    expect(withBaseline(ops, DEFAULT_AIRPLS)).toEqual([...ops, DEFAULT_AIRPLS]);
  });

  it('replaces an existing baseline step in place', () => {
    const withTwo = [...withBaseline(ops, DEFAULT_AIRPLS), { op: 'reverse' } as const];
    expect(withBaseline(withTwo, DEFAULT_POLYNOMIAL)).toEqual([
      ...ops,
      DEFAULT_POLYNOMIAL,
      { op: 'reverse' },
    ]);
  });
});

group('withPhase', () => {
  const phase = { p0: 10, p1: -20, pivot: 0.3 };

  it('inserts the phase step right after the Fourier transform', () => {
    const result = withPhase([...ops, DEFAULT_AIRPLS], phase);
    expect(result.map((o) => o.op)).toEqual(['exponential', 'ft', 'phase', 'baseline_airpls']);
    expect(findPhase(result)).toEqual({ op: 'phase', ...phase });
  });

  it('replaces an existing phase step', () => {
    const once = withPhase(ops, phase);
    expect(withPhase(once, { ...phase, p0: 99 })).toEqual([
      ...ops,
      { op: 'phase', ...phase, p0: 99 },
    ]);
  });
});

group('editing steps', () => {
  it('moves a step and ignores moves past the ends', () => {
    expect(moveOperation(ops, 0, 1).map((o) => o.op)).toEqual(['ft', 'exponential']);
    expect(moveOperation(ops, 0, -1)).toBe(ops);
  });

  it('sets parameters, rounding sizes', () => {
    expect(setParameter(ops, 0, 'lb_hz', 1.5)[0]).toEqual({ op: 'exponential', lb_hz: 1.5 });
    const zf: Operation[] = [{ op: 'zero_fill', size: 1024 }];
    expect(setParameter(zf, 0, 'size', 2048.4)[0]).toEqual({ op: 'zero_fill', size: 2048 });
  });
});

group('describe', () => {
  it('labels steps with their parameters', () => {
    expect(describe({ op: 'zero_fill', size: 131072 })).toBe('Zero fill to 131,072 points');
    expect(describe(DEFAULT_AIRPLS)).toBe('Baseline: airPLS, λ 1.0e+7');
    expect(describe(DEFAULT_WHITTAKER)).toBe('Baseline: Whittaker, λ 1.0e+5, filter 7.3 Hz');
    expect(
      describe({ op: 'baseline_whittaker', lambda: 32768, filter_hz: 2.11, auto_filter: false }),
    ).toBe('Baseline: Whittaker, λ 3.3e+4, filter 2.11 Hz (manual)');
  });
});

group('withStep', () => {
  const ops: Operation[] = [
    { op: 'remove_digital_filter' },
    { op: 'exponential', lb_hz: 0.3 },
    { op: 'zero_fill', size: 65536 },
    { op: 'ft' },
    { op: 'phase', p0: 10, p1: 0, pivot: 0 },
  ];
  const kinds = (list: Operation[]) => list.map((o) => o.op);
  it('puts steps on the FID before the transform, in the order they run', () => {
    expect(kinds(withStep(ops, { op: 'sine_bell', offset: 0.5, end: 1, power: 2 }))).toEqual([
      'remove_digital_filter',
      'exponential',
      'sine_bell',
      'zero_fill',
      'ft',
      'phase',
    ]);
    expect(kinds(withStep(ops, { op: 'dc_offset' }))).toEqual([
      'remove_digital_filter',
      'dc_offset',
      'exponential',
      'zero_fill',
      'ft',
      'phase',
    ]);
    const bare = ops.filter((o) => o.op !== 'zero_fill');
    expect(kinds(withStep(bare, { op: 'zero_fill', size: 131072 }))).toEqual([
      'remove_digital_filter',
      'exponential',
      'zero_fill',
      'ft',
      'phase',
    ]);
  });
  it('puts steps on the spectrum right after the transform', () => {
    expect(kinds(withStep(ops, { op: 'magnitude' }))).toEqual([
      'remove_digital_filter',
      'exponential',
      'zero_fill',
      'ft',
      'magnitude',
      'phase',
    ]);
  });
});
