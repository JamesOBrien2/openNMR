import type { DocumentInfo } from './api';

/** How several spectra share the plot. */
export type StackMode = 'overlay' | 'stacked' | 'split';

export const STACK_MODES: StackMode[] = ['overlay', 'stacked', 'split'];

/** Where a spectrum has been moved to by hand, and how far it has been scaled. */
export interface Adjustment {
  /** Shift along the ppm axis. */
  dx: number;
  /** Shift in display units, one being a full spectrum height. */
  dy: number;
  scale: number;
}

export const NO_ADJUSTMENT: Adjustment = { dx: 0, dy: 0, scale: 1 };

/** Vertical distance between stacked spectra, in units of one spectrum height. */
export const STACK_STEP = 1.1;

/** Sideways distance between stacked spectra, as a fraction of the visible range. */
export const STACK_SPREAD = 0.07;

/** Stacking angle in degrees: 90 stacks straight up, 0 lays the spectra side
 * by side, in between gives the usual diagonal stack. */
export const DEFAULT_STACK_ANGLE = 90;

/** The drawing range of one spectrum of a stack, in raw intensity.
 *
 * Every spectrum owns a fixed band: the baseline of `position` sits at that
 * fraction of the plot height and stays there, so raising `gain` grows the traces
 * inside their bands instead of pushing the baselines apart. A value `v` lands at
 * `position / count + (v / scale) * gain / count` of the height, which inverts to
 * the range below.
 */
export function rowRange(
  position: number,
  count: number,
  scale: number,
  gain: number,
): [number, number] {
  const span = scale / Math.max(gain, 1e-6);
  return [-position * span, (count - position) * span];
}

/** Offset of the spectrum at `position` in a stack, in spectrum heights and in
 * fractions of the visible ppm range. */
export function stackOffset(position: number, angle: number): { dx: number; dy: number } {
  const radians = (Math.min(Math.max(angle, 0), 90) * Math.PI) / 180;
  return {
    dx: position * STACK_SPREAD * Math.cos(radians),
    dy: position * STACK_STEP * Math.sin(radians),
  };
}

/** Trace colours for light and dark themes, the first the single-spectrum
 * colour: the stains a plate is developed in (CAM, permanganate, anisaldehyde,
 * vanillin, iodine, phosphomolybdic acid), and no green, which is what is live. */
export const PALETTE = {
  light: ['#2b59a6', '#6d3fb0', '#b52d6e', '#a86a0c', '#7b4f2a', '#1f7373'],
  dark: ['#82a8ff', '#b58cff', '#ff7fb4', '#f0b34f', '#d29a66', '#5fd1c8'],
};

/** A nucleus as it is written: the mass number raised, as in ¹H or ¹³C. */
export function nucleusLabel(nucleus: string): string {
  const raised = '⁰¹²³⁴⁵⁶⁷⁸⁹';
  return nucleus.replace(/^\d+/, (digits) =>
    [...digits].map((d) => raised[Number(d)] ?? d).join(''),
  );
}

/** Largest absolute intensity of one spectrum. */
export const scaleOf = (d: DocumentInfo) => Math.max(Math.abs(d.min), Math.abs(d.max)) || 1;

/** The divisor a spectrum is drawn with. One scale for the whole set keeps their
 * relative heights, which is what makes a stack comparable; `autoScale` gives
 * each spectrum its own instead, filling its own row. */
export const scaleFor = (d: DocumentInfo, docs: DocumentInfo[], autoScale: boolean) =>
  autoScale ? scaleOf(d) : Math.max(...docs.map(scaleOf));

export const lastPpm = (d: DocumentInfo) => d.firstPpm + (d.points - 1) * d.deltaPpm;

/** Fractional point indices of a document at the given ppm values. */
export const indexOf = (d: DocumentInfo, ppm: number) => (ppm - d.firstPpm) / d.deltaPpm;

/** Distinct colour for spectrum `position` of `count`: the palette while it
 * lasts, then a ramp around the hue circle, so a titration of a dozen or more
 * spectra still reads as a series. */
export function colorAt(position: number, count: number, palette: string[]): string {
  if (count <= palette.length) return palette[position % palette.length];
  const hue = (position / count) * 320;
  const channel = (k: number) => {
    const n = (k + hue / 30) % 12;
    return 0.62 - 0.42 * Math.max(-1, Math.min(Math.min(n - 3, 9 - n), 1));
  };
  const hex = (v: number) =>
    Math.round(Math.max(0, Math.min(1, v)) * 255)
      .toString(16)
      .padStart(2, '0');
  return `#${hex(channel(0))}${hex(channel(8))}${hex(channel(4))}`;
}

/** The annotations that can be moved up and down, each kept per spectrum. */
export const MOVED = ['labels', 'curve', 'bracket', 'multiplet'] as const;
export type Moved = (typeof MOVED)[number];

/** A shift written to `decimals` places, never as -0.00: a line on zero whose
 *  top lands a hair below it is still at zero. */
export const fixed = (value: number, decimals: number) =>
  (Math.abs(value) < 0.5 * 10 ** -decimals ? 0 : value).toFixed(decimals);
