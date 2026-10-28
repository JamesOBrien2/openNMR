/** The page the plot is printed on. The plot is laid out on it at the size the
 * page will be, so what is on screen in page mode is what the file holds. */
export interface PageSetup {
  /** Page size and the clear space round the plot, in mm. */
  width: number;
  height: number;
  margin: number;
  font: string;
  /** How large the axis is lettered, in points. Everything is laid out in
   * proportion to it, as on screen. */
  textSize: number;
  /** The other kinds of text, in points. */
  peakSize: number;
  integralSize: number;
  multipletSize: number;
  nameSize: number;
  /** Traces in black, in each spectrum's colour, or all in `traceColour`. */
  traces: 'black' | 'spectra' | 'one';
  traceColour: string;
  /** Each kind of thing in its own colour. */
  axisColour: string;
  peakColour: string;
  integralColour: string;
  multipletColour: string;
  nameColour: string;
  background: string;
  /** How far above the axis the lowest thing on the page sits when it is
   * fitted - the baseline, or the integrals under it - in % of the height. */
  baselineGap: number;
  /** What the axis measures, lettered under it; empty for nothing. */
  axisTitle: string;
}

export const PAGE_SETUP: PageSetup = {
  width: 82.5,
  height: 60,
  margin: 2,
  font: 'Arial',
  textSize: 7,
  peakSize: 6.5,
  integralSize: 7,
  multipletSize: 7,
  nameSize: 7,
  traces: 'black',
  traceColour: '#000000',
  axisColour: '#000000',
  peakColour: '#000000',
  integralColour: '#000000',
  multipletColour: '#000000',
  nameColour: '#000000',
  background: '#ffffff',
  baselineGap: 5,
  axisTitle: 'ppm',
};

/** Page sizes to start from, width by height in mm. A journal gives a column's
 * width; the height is the figure's own. */
export const PAGES: { name: string; width: number; height: number }[] = [
  { name: 'Single column', width: 82.5, height: 60 },
  { name: 'Double column', width: 177.8, height: 90 },
  { name: 'A4 portrait', width: 210, height: 297 },
  { name: 'A4 landscape', width: 297, height: 210 },
  { name: 'Letter portrait', width: 215.9, height: 279.4 },
  { name: 'Letter landscape', width: 279.4, height: 215.9 },
  { name: 'Slide 16:9', width: 254, height: 142.9 },
];

/** The plot's own text sizes, in its pixels. */
export const PLOT_TEXT = { axis: 11, peaks: 10, integrals: 11, multiplets: 11, names: 11 };
const PT_PER_MM = 72 / 25.4;

/** Pixels of the plot to a point of page: its axis lettering, 11 of its
 * pixels, comes out at `textSize` points. */
const pixelsPerPt = (setup: PageSetup) => PLOT_TEXT.axis / setup.textSize;

/** Pixels of the plot to a millimetre of page. */
export const pixelsPerMm = (setup: PageSetup) => pixelsPerPt(setup) * PT_PER_MM;

/** Each kind of text in the plot's pixels, so it prints at its size in points. */
export function textSizesOf(setup: PageSetup) {
  const px = pixelsPerPt(setup);
  return {
    axis: PLOT_TEXT.axis,
    peaks: setup.peakSize * px,
    integrals: setup.integralSize * px,
    multiplets: setup.multipletSize * px,
    names: setup.nameSize * px,
  };
}

/** The plot's size in its own pixels: the page less its margins. */
export function plotSize(setup: PageSetup): { width: number; height: number } {
  const perMm = pixelsPerMm(setup);
  return {
    width: (setup.width - 2 * setup.margin) * perMm,
    height: (setup.height - 2 * setup.margin) * perMm,
  };
}
