<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { X } from '@lucide/svelte';
  import {
    autoPhase,
    fetchBaselineTrace,
    fetchIntegralCurve,
    fetchPhaseTrace,
    fetchTrace,
    handSetBaseline,
    largestPeak,
    proposeBaselinePoints,
    type DocumentInfo,
    type PhaseParams,
  } from './api';
  import { hexColor, TraceRenderer, type Vec4 } from './gl';
  import { isEditable, isMac, windowOpen } from './keys';
  import { letterOf, nucleiOf, simulateLines } from './multiplets';
  import { fixed, MOVED, type Moved } from './traces';
  import {
    pressPan,
    pressZoom,
    RANGE_TOOLS,
    viewAction,
    wheelAction,
    type RangeTool,
    type Tool,
    type ViewAction,
  } from './tools';
  import { PLOT_TEXT } from './page';
  import { DEFAULT_POINTS, findPhase, THROUGH_NAMES, type PointsOperation } from './operations';
  import { suggestName } from './impurities';
  import { findSolvent, formatSolvent, solventShift, solventsFor } from './solvents';
  import { CanvasPainter, paintScale, SvgPainter, type Painter } from './painter';
  import { paintCursor, type CursorKind } from './cursors';
  import {
    colorAt,
    indexOf,
    lastPpm,
    NO_ADJUSTMENT,
    rowRange,
    scaleFor,
    stackOffset,
    type Adjustment,
    type StackMode,
  } from './traces';
  import {
    amend,
    covers,
    extent,
    fitY,
    historyOf,
    intensityAt,
    record,
    step,
    type History,
    pan,
    ppmAt,
    pxOfPpm,
    traceRequest,
    zoomAt,
    zoomBox,
    zoomY,
    type Axis,
    type Limits,
    type TraceRequest,
    type View,
  } from './view';

  let {
    docs,
    active,
    stack,
    palette,
    colorOverrides,
    names,
    threshold,
    onerror,
    onphase,
    onbaselinepoints,
    onrange,
    onpickpoint,
    onpickvalue,
    onopenmultiplet,
    onhovermultiplet,
    onassign,
    marked,
    armed,
    shifts,
    onshift,
    onmove,
    display,
    onmovetrace,
    onscaletrace,
    onthreshold,
    showAll,
    showLabels,
    snap,
    showCurves,
    showMultiplets,
    showIntegrals = true,
    multipletLabel = 'shift',
    zoom = 1,
    ink,
    textSizes,
    baselineAt,
    axisTitle,
    showSimulation,
    decimals,
    angle,
    autoScale,
    adjusting,
    clipped,
    onactivate,
    onreadout,
    onoverlap,
    onreference,
    onremove,
    onlabelpeak,
    mode = $bindable(),
    zoomAxis = $bindable(),
    panAxis = $bindable(),
    crosshair = $bindable(),
    focused = true,
    onfocus,
  }: {
    /** Spectra to draw, bottom to top; includes `active`. */
    docs: DocumentInfo[];
    active: DocumentInfo;
    /** Overlay, stacked or cascade. */
    stack: StackMode;
    /** Trace colours of the chosen theme, the first for a single spectrum. */
    palette: string[];
    /** Colours chosen by hand for particular spectra, by document id. */
    colorOverrides: Record<number, string>;
    /** Names given by hand, by document id; a spectrum is called this everywhere. */
    names: Record<number, string>;
    /** Peak picking threshold in the active spectrum's intensity units. */
    threshold: number;
    onerror: (message: string) => void;
    /** Called with the final values when a manual phase correction is applied. */
    onphase: (phase: PhaseParams) => void;
    /** Called with the baseline when one drawn through points is applied. */
    onbaselinepoints: (baseline: PointsOperation) => void;
    /** A range dragged with a tool that takes one: integrated, read as one
     * multiplet, or picked for peaks. */
    onrange: (tool: RangeTool, from: number, to: number) => void;
    /** A shift picked off the spectrum to answer something being asked elsewhere. */
    onpickvalue: (ppm: number) => void;
    /** A multiplet's own chip was clicked, which is the way into its window. */
    onopenmultiplet: (index: number) => void;
    /** The multiplet the pointer is over, by way of its region or of one of its
     * peaks, or none: the structure lights up the hydrogens it is assigned to. */
    onhovermultiplet?: (index: number | undefined) => void;
    /** Hydrogens of the structure are waiting for a signal: a click in a multiplet's
     * region gives it to them rather than opening its window. */
    armed?: boolean;
    onassign?: (index: number) => void;
    /** A multiplet lit from outside, when the pointer is over its hydrogens. */
    marked?: number;
    /** `exact` keeps `ppm`; otherwise the highest point within `radius` points is picked. */
    onpickpoint: (ppm: number, exact: boolean, radius: number) => void;
    /** How far each spectrum's labels have been moved down, by document id. */
    shifts: Record<number, Partial<Record<Moved, number>>>;
    /** Called when a spectrum's labels have been dragged up or down. */
    onshift: (id: number, what: Moved, dy: number) => void;
    /** Called when an end of an integral's or a multiplet's range has been dragged. */
    onmove: (kind: 'integral' | 'multiplet', index: number, from: number, to: number) => void;
    /** Where each spectrum has been moved and scaled by hand, by document id. */
    display: Record<number, Adjustment>;
    onmovetrace: (id: number, dx: number, dy: number) => void;
    onscaletrace: (id: number, factor: number) => void;
    /** A new peak picking threshold, in multiples of the noise. */
    onthreshold: (factor: number) => void;
    /** Label the peaks of every spectrum, not only the one being worked on. */
    showAll: boolean;
    /** Draw the peak labels at all. They are kept either way, so this only hides
     * them, and nothing is reserved at the top of the plot for them. */
    showLabels: boolean;
    /** Picking takes the top of the nearest line. Off, it takes the pointer where
     * it is, which is what Shift does while it is held. */
    snap: boolean;
    /** Draw the integral curves. The range and its value are shown either way. */
    showCurves: boolean;
    /** Draw the multiplet ranges and what each is called. */
    showMultiplets: boolean;
    /** Integral curves, brackets and values; off hides them all. */
    showIntegrals?: boolean;
    /** What a multiplet is called on the plot: its shift, its name, or both. */
    multipletLabel?: 'shift' | 'name' | 'both';
    /** Draw the lines each multiplet would have, to be read against the signal. */
    showSimulation: boolean;
    /** Decimal places for a shift, so every ppm in the app reads the same. */
    decimals: number;
    /** How much larger the plot is shown than it is laid out: a page is laid
     *  out at its own size and shown enlarged. Drawing is scaled, not the
     *  element - an enlarged element is drawn at its own size and blown up. */
    zoom?: number;
    /** Colours and typeface to draw with instead of the theme's: the page's,
     *  which is printed black on white whatever the screen is. */
    ink?: Partial<Colours>;
    /** Text sizes in the plot's own pixels, part by part, where they differ from
     *  its own: a page sets each in points. */
    textSizes?: Partial<TextSizes>;
    /** How far above the foot the lowest thing drawn sits when the view is
     *  fitted, as a share of the height: the baseline, or the integrals' values
     *  hanging under it. */
    baselineAt?: number;
    /** What the axis measures, lettered under it, as a page's is. */
    axisTitle?: string;
    /** Stacking angle in degrees; 90 stacks straight up. */
    angle: number;
    /** Give each spectrum its own scale instead of one for the whole set. */
    autoScale: boolean;
    /** Dragging moves and scales one spectrum rather than the view. */
    adjusting: boolean;
    /** Cut each spectrum off at its own row, so a tall peak stays in its band. */
    clipped: boolean;
    /** Shift with the up or down arrow walks the stack. */
    onactivate: (id: number) => void;
    /** Where the pointer is, for the tool bar to show. */
    onreadout?: (text: string) => void;
    /** How many multiplet names of the active spectrum found no room of their
     *  own and overlap another, so the figure can say so rather than hide it. */
    onoverlap?: (count: number) => void;
    /** A peak at `fromPpm` is to be called `toPpm`, which shifts the axis. */
    onreference: (fromPpm: number, toPpm: number) => void;
    /** Called when a peak, an integral or a multiplet is taken away from the plot itself. */
    onremove: (kind: 'peak' | 'integral' | 'multiplet', index: number) => void;
    /** `null` follows the impurity table again, `''` shows nothing. */
    onlabelpeak: (index: number, label: string | null) => void;
    /** The tool bar's state, shared by every pane it drives. */
    mode: Tool;
    zoomAxis: Axis;
    panAxis: Axis;
    crosshair: boolean;
    /** Split panes share the window, so only the focused one takes the keyboard. */
    focused?: boolean;
    onfocus?: () => void;
  } = $props();

  type Range = [number, number];

  /** How large each kind of text is drawn, and how much the room kept for it
   *  grows with it, so text set larger still fits where it is written. */
  const sizes = $derived<TextSizes>({ ...PLOT_TEXT, ...textSizes });
  const axisScale = $derived(sizes.axis / PLOT_TEXT.axis);
  const peakScale = $derived(sizes.peaks / PLOT_TEXT.peaks);
  const integralScale = $derived(sizes.integrals / PLOT_TEXT.integrals);
  const multipletScale = $derived(sizes.multiplets / PLOT_TEXT.multiplets);
  const AXIS_HEIGHT = $derived((axisTitle ? 44 : 28) * axisScale);
  const LENS = 0.8;
  const INTENSITY_STEP = 1.25;
  const DRAG_THRESHOLD = 4;
  const MIN_SPAN_POINTS = 8;
  const P0_PER_PX = 0.5;
  const P1_PER_PX = 2;
  const FINE_DRAG = 0.1;
  const PIVOT_GRAB_PX = 6;
  /** How near a point of a hand-set baseline a press takes hold of it. */
  const POINT_GRAB_PX = 7;
  const PEAK_CLICK_PX = 8;
  const CURVE_SAMPLES = 256;
  /** Peak labels stand upright in a band at the top: small, side by side, with
      a tick under each rather than a line to the peak. */
  const LABEL_BAND = $derived(84 * peakScale);
  /** Room the shift itself takes, so a name clears it. */
  const PPM_TEXT = $derived(34 * peakScale);
  /** A row needs at least this much height before its own labels will fit. */
  const LABEL_ROW_MIN = 90;
  /** What a compact label reserves above its row's peaks: one line of text, and a
      second for the multiplet names when there are any. */
  const COMPACT_LINE = 14;
  const LABEL_GAP = $derived(11 * peakScale);
  const LABEL_TICK = 7;
  const PEAK_TICK = 6;
  /** Room a bare shift needs above the band's tick; a name stands above it, which
      is what the whole LABEL_BAND is for. */
  const SHIFT_BAND = $derived(PPM_TEXT + 6);
  /** What hangs below the band: its tick and fan, and the peak's own tick. */
  const LABEL_TICKS = LABEL_TICK + 6 + PEAK_TICK + 2;
  const GRAB_PX = 6;
  /** How far a preview looks for the top of a line. */
  const PEAK_SNAP_PX = 30;
  /** How near the drawn trace a click counts as being on it. */
  const TRACE_GRAB_PX = 24;
  /** Tools that mark the line under the pointer before it is taken. */
  const PREVIEW_TOOLS: Tool[] = ['reference', 'peakByPeak', 'pickValue'];
  /** Rows the integral values step down by when two would overlap. */
  const VALUE_ROW = $derived(12 * integralScale);
  /** Air between the top of a signal and the foot of its integral curve, and the
      height the largest integral's curve rises to: each curve stands over its own
      signal rather than across it, where it hid the lines it measured. */
  const CURVE_GAP = 8;
  const CURVE_HEIGHT = 44;
  /** How near a multiplet's edge counts as on it, and the grip drawn there. */
  const EDGE_GRAB = 5;
  const GRIP_W = 6;
  const GRIP_H = 22;
  /** Height of the mark a multiplet's range is drawn as, above the signals. */
  const MULTIPLET_BAND = $derived(18 * multipletScale);
  /** Air between the integral's curve and the multiplet's chip: they were set one
      straight under the other and read as one crowded stack. */
  const MULTIPLET_GAP = 14;
  /** The chip a multiplet is named in: a character's width at size 10, the room
      either side of the text, and how tall the box stands. */
  const CHIP_CHAR = $derived(5.8 * multipletScale);
  const CHIP_PAD = 6;
  /** A line of text inside a chip, which holds one or several: its height is
      what its lines come to, so it has none of its own. */
  const CHIP_LINE = $derived(11 * multipletScale);
  /** The grip at each end of an integral's bracket, which is how its range is
      edited. Small, so it is a deliberate target rather than a thing to catch. */
  const HANDLE = 9;
  const HANDLE_RADIUS = 3;

  let area: HTMLDivElement;
  let glCanvas: HTMLCanvasElement;
  let overlay: HTMLCanvasElement;
  let axis: HTMLCanvasElement;

  /** The area's size on screen, and the plot's as it is laid out. */
  let areaWidth = $state(0);
  let areaHeight = $state(0);
  const width = $derived(areaWidth / zoom);
  const height = $derived(areaHeight / zoom);
  /** Device pixels to a pixel of the plot. */
  const dpr = $derived(devicePixelRatio * zoom);
  let spaceHeld = $state(false);
  /** Whether Shift is down, which decides what a click takes and what can be
      taken hold of; kept so both follow the key rather than the pointer. */
  let shiftHeld = $state(false);
  let phasing = $state<PhaseParams>();
  /** A baseline being drawn through points, until it is applied or cancelled. */
  let pointing = $state<PointsOperation>();
  /** Shows the spectrum with the baseline taken off rather than the baseline. */
  let pointsPreview = $state(false);
  /** New points stay where they are put rather than on the spectrum. */
  let freePoints = $state(false);
  let handSet = $state<{
    points: [number, number][];
    curve: number[];
    top: number;
    start: number;
    end: number;
    /** The points it was worked out for, as `pointsKey` writes them. */
    key: string;
  }>();
  const pointsKey = $derived(pointing ? JSON.stringify(pointing.points) : '');
  /** Typed zoom: the ppm the view should run between. */
  let zoomTo = $state<{ from: string; to: string }>();
  /** Referencing: the peak whose shift is being named, in ppm. */
  let referencing = $state<number>();
  /** The peak the pointer is over while referencing, before it is chosen. */
  let preview = $state<number>();
  let solventDialog = $state<HTMLDialogElement>();
  /** The peak a right click landed on, and where to put its menu. */
  let menu = $state<{
    /** What was right clicked: only a peak has a name to be typed into. */
    kind: 'peak' | 'integral' | 'multiplet';
    index: number;
    x: number;
    y: number;
    label: string;
    /** The name it opened with, so closing only writes what was actually typed. */
    from: string;
  }>();

  /** Closes the menu, keeping what was typed. A field reports a change only when
      it loses focus, and a press anywhere in the plot takes the menu away before
      that, so waiting for the change threw the edit away - which is why clearing
      a suggested name and clicking away left it where it was. */
  function closeMenu(keep = true) {
    const closing = menu;
    menu = undefined;
    if (!closing || !keep || closing.kind !== 'peak') return;
    const text = closing.label.trim();
    if (text !== closing.from.trim()) onlabelpeak(closing.index, text);
  }
  /** Whether one spectrum is picked out of a stack. Clicking away from every
      trace clears it, so each spectrum is read in its own colour. */
  let highlighted = $state(true);
  let zoomDialog = $state<HTMLDialogElement>();

  $effect(() => {
    if (zoomTo) zoomDialog?.showModal();
  });

  /** The shift the dataset's own solvent suggests, when it names one we know. */
  const suggestedShift = $derived(solventShift(active.solvent, active.nucleus));
  /** The dataset's own solvent first, where the eye starts; each at its line of
      the spectrum's nucleus. */
  const solventOrder = $derived(
    solventsFor(active.nucleus).sort(
      (a, b) => Number(b.shift === suggestedShift) - Number(a.shift === suggestedShift),
    ),
  );

  /** What the picked line is to be called, typed; it starts at the shift the
      dataset's solvent suggests, or where the line already is. */
  let manualShift = $state('');

  // Opening it as a modal is what dims the rest of the app and traps the keyboard.
  $effect(() => {
    if (referencing === undefined) return;
    manualShift = (untrack(() => suggestedShift) ?? referencing).toFixed(3);
    solventDialog?.showModal();
  });
  const tool = $derived<Tool>(spaceHeld ? 'pan' : mode);
  const selectionAxis = $derived<Axis>(tool === 'zoom' ? zoomAxis : 'x');

  const limits = $derived<Limits>({
    left: Math.max(...docs.map((d) => d.firstPpm)),
    right: Math.min(...docs.map(lastPpm)),
    minSpan: MIN_SPAN_POINTS * Math.abs(active.deltaPpm),
  });
  const activePosition = $derived(docs.findIndex((d) => d.id === active.id));
  const solvent = $derived(findSolvent(active.solvent, active.nucleus));
  /** What a peak is called on screen: what it was named, or what is suggested. */
  const nameOnScreen = (index: number, ppm: number) =>
    active.analysis.labels[index] ?? suggestName(ppm, solvent) ?? '';
  /** Room left under the bottom row of a stack, about `FOOT_PX` and counted in
      rows: its integral values and brackets hang below its baseline, which
      otherwise sat on the axis and pushed them up over the signal. */
  const FOOT_PX = 44;
  const FOOT = $derived(
    stack === 'stacked' && height > 0
      ? Math.min(0.75, (FOOT_PX * Math.max(docs.length, 1)) / height)
      : 0,
  );
  /** Spectra of a stack each own a row; everything else shares the whole plot. */
  const rows = $derived(stack === 'stacked' ? Math.max(docs.length, 1) + FOOT : 1);

  /** How far down a spectrum's own annotations reach: its labels, and the band
      its integral curves ride in, which is above the signals rather than across
      them. */
  function clearFor(d: DocumentInfo, position: number): number {
    return labelBand(position, d) + LABEL_TICKS + annotations(d);
  }

  /** Room the curves and multiplet marks of one spectrum take under its labels. */
  function annotations(d: DocumentInfo): number {
    const marks =
      showMultiplets && d.analysis.multiplets.length > 0 ? MULTIPLET_GAP + MULTIPLET_BAND : 0;
    const curves =
      showIntegrals &&
      showCurves &&
      d.analysis.integrationMethod !== 'fitted' &&
      d.analysis.integrals.length > 0
        ? CURVE_HEIGHT + CURVE_GAP
        : 0;
    return marks + curves;
  }

  /** The share of the plot, or of one stacked row, that peak labels stand in.
      Nothing is given away until there are peaks to label. */
  function labelReserve(): number {
    // Room is kept wherever something stands over the lines: the labels, or the
    // integral curves and multiplet names, which are placed under the same band
    // (`clearFor`). Kept for labels alone, a spectrum with integrals and no peaks
    // was fitted up to the top and its curves were drawn across its lines.
    const need = Math.max(
      0,
      ...labelledDocs().map((d) =>
        (showLabels && d.analysis.peaks.length) || annotations(d) > 0
          ? labelHeight(d) + LABEL_TICKS + annotations(d)
          : 0,
      ),
    );
    const per = stack === 'stacked' ? height / rows : height;
    if (compact()) {
      const lines = labelledDocs().some((d) => d.analysis.peaks.length) ? 1 : 0;
      const marks = showMultiplets && docs.some((d) => d.analysis.multiplets.length) ? 1 : 0;
      return per > 0 ? ((lines + marks) * COMPACT_LINE) / per : 0;
    }
    return need > 0 && per > 0 ? need / per : 0;
  }

  /** The spectra whose peaks are labelled: all of a stack when it shows them all,
      the active one otherwise, since they would share one band across the top. */
  function labelledDocs(): DocumentInfo[] {
    return stack === 'stacked' && showAll ? docs : [active];
  }

  /** How far a spectrum's labels reach down from the top of its row. A bare shift
      needs less room than one with a name above it, and giving the rest back to
      the spectrum is worth the band moving. */
  function labelHeight(d: DocumentInfo): number {
    const named = d.analysis.peaks.some(
      (ppm, i) => d.analysis.labels[i] ?? suggestName(ppm, solvent),
    );
    return named ? LABEL_BAND : SHIFT_BAND;
  }

  function fullView(): View {
    const lo = Math.min(...docs.map((d, k) => d.min / traceScale(d) + baseOffset(d, k)));
    const hi = Math.max(...docs.map((d, k) => d.max / traceScale(d) + baseOffset(d, k)));
    return fitY(
      { x0: limits.left, x1: limits.right, y0: 0, y1: 1 },
      lo,
      hi,
      labelReserve(),
      baselineShare(),
    );
  }

  const initial = untrack(fullView);
  let view = $state(initial);
  let history = $state(historyOf(initial));

  let renderer = $state<TraceRenderer>();
  let loaded = $state<Record<number, TraceRequest & { revision: number; phase: string }>>({});
  let curves = $state<Record<string, Float32Array>>({});
  // Deliberately not reactive: `loaded` and `curves` signal changes.
  const buffers: Record<number, Float32Array> = {};
  /** A range was asked for and its height is fitted once its trace arrives. */
  let fitWhenLoaded = false;
  const inflight: Record<string, boolean> = {};
  let cursor = $state<{ x: number; y: number }>();
  let selection = $state<{ x0: number; y0: number; x1: number; y1: number }>();
  let drag:
    | {
        kind:
          | 'view'
          | 'p0'
          | 'p1'
          | 'pivot'
          | 'peak'
          | 'integral'
          | 'trace'
          | 'threshold'
          | 'baseline'
          | 'multiplet'
          | 'multipletEnd'
          | 'curve'
          | 'bracket'
          | 'point';
        x: number;
        y: number;
        start: View;
        phase?: PhaseParams;
        /** Which peak, integral or multiplet is being dragged, and which of its
         * two edges where it has them. */
        index?: number;
        edge?: 0 | 1;
        /** Where the dragged spectrum sat when the drag started. */
        adjust?: Adjustment;
        moved: boolean;
      }
    | undefined;
  /** Intensity of a stack: it scales the traces inside their fixed rows. */
  let gain = $state(1);
  /** Where the dragged integral or multiplet edge sits now, in ppm. */
  let dragPpm = $state<number>();
  /** How far whatever is being dragged has come, while the drag is going on. */
  let dragDy = $state<number>();
  /** How far `what` has been moved down for spectrum `d`, the drag included. */
  function movedBy(d: DocumentInfo, what: Moved): number {
    const held = drag?.kind === what && d.id === active.id && dragDy !== undefined ? dragDy : 0;
    return (shifts[d.id]?.[what] ?? 0) + held;
  }
  /** One multiplet worked out as it would be if it were ideal. */
  interface Ideal {
    m: DocumentInfo['analysis']['multiplets'][number];
    info: DocumentInfo['multipletInfo'][number] | undefined;
    lines: { ppm: number; weight: number }[];
    half: number;
    amplitude: number;
    tallest: number;
  }

  /** The last worked-out multiplets and their outlines, so that a repaint that
      changes none of it draws them again without working any of it out. */
  let ideals: { multiplets: unknown; info: unknown; shapes: Ideal[] } | undefined;
  let outlines:
    | {
        ideal: Ideal[];
        x0: number;
        x1: number;
        w: number;
        scale: number;
        base: number;
        shift: number;
        points: (Float32Array | undefined)[];
      }
    | undefined;
  /** Where each integral curve of the active spectrum stands, for taking hold of. */
  let curveFeet: { x0: number; x1: number; y: number }[] = [];
  /** Each multiplet's region of the active spectrum, from under its name down to
      the baseline: pointed at anywhere, it shows its edges, and they are grabbed
      anywhere along their height, so no name can cover them. */
  let bands: { index: number; x0: number; x1: number; top: number; bottom: number }[] = [];
  /** Where each multiplet's chip is drawn and taken hold of, the active
      spectrum's only, as the peak labels are. `rest` is the square it is at
      rest, which is what the others are laid out around: what it says while it
      is pointed at must not move anything. */
  let chips: {
    index: number;
    x: number;
    y: number;
    w: number;
    h: number;
    rest: number;
  }[] = [];
  /** Which way whatever is under the pointer would move if it were taken hold
      of, which is what the cursor says: '' for nothing to take. */
  let grabAxis = $state<'' | 'x' | 'y' | 'click'>('');
  /** Peak whose label the pointer is over, drawn lit with its peak. */
  let hovered = $state<number>();
  /** The integral whose bracket the pointer is on: its ends show themselves. */
  let hoveredIntegral = $state<number>();
  /** The multiplet under the pointer: it says more about itself while it is. */
  let hoveredMultiplet = $state<number>();

  const layout = $derived(`${docs.map((d) => d.id).join(',')}:${stack}`);
  let shownLayout = untrack(() => layout);

  const nameOf = (d: DocumentInfo) => names[d.id] ?? d.name;
  const colorOf = (d: DocumentInfo, position: number) =>
    colorOverrides[d.id] ??
    (docs.length === 1 ? palette[0] : colorAt(position, docs.length, palette));
  /** A spectrum's own colour taken most of the way to grey: enough to say which
      spectrum a mark belongs to, not enough for the mark to be read as a trace. */
  function desaturated(color: string): string {
    if (!/^#[0-9a-f]{6}$/i.test(color)) return color;
    const n = parseInt(color.slice(1), 16);
    const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
    const grey = 0.299 * r + 0.587 * g + 0.114 * b;
    const towards = (v: number) =>
      Math.round(v + (grey - v) * 0.55)
        .toString(16)
        .padStart(2, '0');
    return `#${towards(r)}${towards(g)}${towards(b)}`;
  }

  const curveKey = (d: DocumentInfo, [from, to]: Range) => `${d.id}:${d.revision}:${from}:${to}`;
  const pyOf = (y: number) => ((view.y1 - y) / (view.y1 - view.y0)) * height;
  /** How far spectrum `position` is moved along the ppm axis in a cascade. */
  // Declarations, not arrow constants: `fullView` runs while the component is
  // still being set up, before any `const` below it exists.
  function adjustOf(d: DocumentInfo): Adjustment {
    return display[d.id] ?? NO_ADJUSTMENT;
  }

  /** One divisor for the whole set, so the spectra keep their relative heights,
      divided again by whatever this one has been scaled by hand. */
  function traceScale(d: DocumentInfo): number {
    return scaleFor(d, docs, autoScale) / adjustOf(d).scale;
  }

  /** Where the baseline of one spectrum sits, as a fraction of the height. */
  function rowFraction(d: DocumentInfo, position: number): number {
    return stack === 'stacked' ? (position + FOOT + adjustOf(d).dy) / rows : 0;
  }

  /** The band one spectrum of a stack may draw in, as a scissor box in device
      pixels, so a tall peak stays inside its own row. */
  function rowClip(d: DocumentInfo, position: number): Vec4 | undefined {
    if (!clipped || stack !== 'stacked') return undefined;
    const bottom = height * (1 - rowFraction(d, position));
    return [0, (height - bottom) * dpr, width * dpr, (height / rows) * dpr];
  }

  function stackAt(position: number) {
    return stack === 'stacked' ? stackOffset(position, angle) : { dx: 0, dy: 0 };
  }

  /** The top of the line nearest a pixel, in ppm, from the trace already drawn.
      Reading it here rather than asking the back end keeps the preview instant. */
  function nearestTop(px: number): number | undefined {
    const have = loaded[active.id];
    const xy = buffers[active.id];
    if (!have || !xy) return undefined;
    const shift = shiftPpmOf(active);
    let best = Number.NEGATIVE_INFINITY;
    let found: number | undefined;
    for (let k = 0; k < xy.length; k += 2) {
      const ppm = active.firstPpm + (have.start + xy[k]) * active.deltaPpm;
      if (Math.abs(pxOfPpm(view, ppm - shift, width) - px) > PEAK_SNAP_PX) continue;
      if (xy[k + 1] > best) {
        best = xy[k + 1];
        found = ppm;
      }
    }
    return found;
  }

  /** Where the drawn trace of `d` sits at pixel `px`, from the buffer on screen. */
  function traceYAt(d: DocumentInfo, position: number, px: number): number | undefined {
    const have = loaded[d.id];
    const xy = buffers[d.id];
    if (!have || !xy || xy.length < 2) return undefined;
    const shift = shiftPpmOf(d, position);
    const scale = traceScale(d);
    // The buffer runs in point order, so the entry nearest a pixel is found by
    // halving the range rather than by walking the whole of it: with a multiplet
    // or two this ran the length of the buffer once for each of them on every
    // repaint, and the plot repaints whenever the pointer moves.
    const wanted = (ppmAt(view, px, width) + shift - d.firstPpm) / d.deltaPpm - have.start;
    let low = 0;
    let high = xy.length / 2 - 1;
    while (high - low > 1) {
      const middle = (low + high) >> 1;
      if (xy[middle * 2] > wanted) high = middle;
      else low = middle;
    }
    let near = Math.abs(xy[low * 2] - wanted) <= Math.abs(xy[high * 2] - wanted) ? low : high;
    // The trace keeps a high and a low value at each place so that no spike is
    // lost when it is drawn small, and either may be landed on. The one further
    // from the baseline is the one that can be seen, which is what is being
    // asked for.
    const place = xy[near * 2];
    for (const other of [near - 1, near + 1]) {
      const at = other * 2;
      if (at < 0 || at + 1 >= xy.length || xy[at] !== place) continue;
      if (Math.abs(xy[at + 1]) > Math.abs(xy[near * 2 + 1])) near = other;
    }
    const ppm = d.firstPpm + (have.start + xy[near * 2]) * d.deltaPpm;
    const away = Math.abs(pxOfPpm(view, ppm - shift, width) - px);
    return away <= TRACE_GRAB_PX ? pyOfRow(xy[near * 2 + 1] / scale, d, position) : undefined;
  }

  /** The highest the trace of `d` is drawn between two pixels, in pixels. Read
      from the loaded trace itself: asking for the trace at each pixel landed on
      the low value of a column as often as the high one, and missed narrow lines. */
  function topIn(d: DocumentInfo, position: number, x0: number, x1: number): number | undefined {
    const have = loaded[d.id];
    const xy = buffers[d.id];
    if (!have || !xy) return undefined;
    const shift = shiftPpmOf(d, position);
    const [i0, i1] = [x0, x1]
      .map((px) => (ppmAt(view, px, width) + shift - d.firstPpm) / d.deltaPpm - have.start)
      .sort((a, b) => a - b);
    let high: number | undefined;
    for (let k = 0; k + 1 < xy.length; k += 2) {
      if (xy[k] < i0 || xy[k] > i1) continue;
      if (high === undefined || xy[k + 1] > high) high = xy[k + 1];
    }
    return high === undefined ? undefined : pyOfRow(high / traceScale(d), d, position);
  }
  /** Which spectrum of a stack a pixel falls in, counting from the bottom row. */
  function rowAt(y: number): number {
    const row = Math.floor((1 - y / height) * rows - FOOT);
    return Math.min(Math.max(row, 0), docs.length - 1);
  }

  /** Pixel row of a display value of the spectrum at `position`. */
  function pyOfRow(value: number, d: DocumentInfo, position: number): number {
    if (stack !== 'stacked') return pyOf(value + adjustOf(d).dy);
    return height * (1 - (rowFraction(d, position) + (value * gain) / rows));
  }

  function baseOffset(d: DocumentInfo, position: number): number {
    return stackAt(position).dy + adjustOf(d).dy;
  }

  function shiftPpmOf(d: DocumentInfo, position = docs.indexOf(d)): number {
    return stackAt(position).dx * (view.x0 - view.x1) + adjustOf(d).dx;
  }

  const pivotPpm = $derived(
    phasing ? active.firstPpm + phasing.pivot * active.points * active.deltaPpm : 0,
  );

  const readout = $derived.by(() => {
    if (!cursor || width === 0) return '';
    const level =
      stack === 'stacked'
        ? ((1 - cursor.y / height - rowFraction(active, activePosition)) * rows) / gain
        : intensityAt(view, cursor.y, height) - baseOffset(active, activePosition);
    const y = level * traceScale(active);
    const intensity =
      Math.abs(y) >= 1e4 || (y !== 0 && Math.abs(y) < 1e-2) ? y.toExponential(3) : y.toFixed(2);
    return `${ppmAt(view, cursor.x, width).toFixed(3)} ppm   ${intensity}`;
  });

  // The tool bar shows it, so it cannot collide with the name of a stack's row.
  $effect(() => {
    if (focused) onreadout?.(readout);
  });

  /** The view or intensity has been set by hand since the whole spectrum was last
      shown: labels then make no room of their own, since picking a peak must not
      undo the zoom it was picked in. */
  let chosen = false;

  function commit(nextView: View) {
    chosen = true;
    history = record(history, nextView);
    view = nextView;
  }

  function walk(by: number) {
    history = step(history, by);
    view = history.views[history.at];
  }

  function scaleIntensity(factor: number) {
    chosen = true;
    // In a stack the baselines are fixed, so intensity grows the traces in place.
    if (stack === 'stacked') {
      gain = Math.min(Math.max(gain / factor, 1e-3), 1e4);
      return;
    }
    const baseline = baseOffset(active, activePosition);
    view = zoomY(view, Math.min(Math.max(baseline, view.y0), view.y1), factor);
  }

  /** The whole spectrum, its height fitted with the baseline where it is set to
   *  sit. */
  const full = () => {
    commit(fullView());
    chosen = false;
  };

  // A new place for the baseline is shown at once, the range left as it is.
  let shownBaseline = untrack(() => baselineAt);
  $effect(() => {
    if (baselineAt === shownBaseline) return;
    shownBaseline = baselineAt;
    untrack(() => {
      // Only the baseline moves; the zoom (the span of intensity) is kept.
      if (stack === 'stacked' || baselineAt === undefined) return fitHigh(false);
      const span = view.y1 - view.y0;
      const y0 = -Math.min(Math.max(baselineShare() ?? 0, 0), 0.8) * span;
      view = { ...view, y0, y1: y0 + span };
    });
  });

  /** Fits the height to the highest line in view; `record` adds it to the zoom
   *  history, which a fit made while a slider moves should not. */
  function fitHigh(record = true) {
    let lo = Infinity;
    let hi = -Infinity;
    docs.forEach((d, k) => {
      const have = loaded[d.id];
      const xy = buffers[d.id];
      if (!have || !xy) return;
      const e = extent(xy, have.start, indexOf(d, view.x0), indexOf(d, view.x1));
      if (!(e.hi >= e.lo)) return;
      const [s, offset] = [traceScale(d), baseOffset(d, k)];
      lo = Math.min(lo, e.lo / s + offset);
      hi = Math.max(hi, e.hi / s + offset);
    });
    if (stack === 'stacked') {
      // Fill one row with the tallest signal in view rather than moving the rows,
      // less the band its labels stand in. Every row shrinks by the same share,
      // which is what keeps a stack comparable.
      if (hi > 0) gain = Math.min(Math.max(1 - labelReserve(), 0.1) / hi, 1e4);
      return;
    }
    const fitted = fitY(view, lo, hi, labelReserve(), baselineShare());
    if (record) commit(fitted);
    else view = fitted;
  }

  /** Makes room for the labels and nothing else: the view only ever grows
      upwards and a stack only ever scales down, so a zoom that was asked for is
      left as it is. Doing this with a fit put the intensity back to fitting the
      highest peak, which threw the zoom away whenever labels were shown. */
  function makeRoomForLabels() {
    let factor = 1;
    let highest: number | undefined;
    for (const d of labelledDocs()) {
      const position = docs.indexOf(d);
      // Only peaks that are on screen: when the view is zoomed in, a tall peak
      // sits far above the window, and making room for that is a zoom out.
      const tops = peakLabels(d, position)
        .map((peak) => peak.top)
        .filter((top) => top >= 0);
      if (tops.length === 0) continue;
      const top = Math.min(...tops);
      const clear = clearFor(d, position);
      if (top >= clear) continue;
      if (stack === 'stacked') {
        const base = pyOfRow(0, d, position);
        const room = base - clear;
        const tall = base - top;
        if (room > 0 && tall > 0) factor = Math.min(factor, room / tall);
      } else {
        highest = highest === undefined ? top : Math.min(highest, top);
      }
    }
    if (stack === 'stacked') {
      if (factor < 1) gain = Math.max(gain * factor, 1e-3);
      return;
    }
    if (highest === undefined) return;
    const clear = clearFor(active, activePosition);
    if (clear >= height * 0.9) return;
    // The intensity under the topmost label, moved down to where the band ends.
    const level = intensityAt(view, highest, height);
    const wanted = view.y0 + (level - view.y0) / (1 - clear / height);
    // Never more than doubling what is on screen: making room is a nudge, not a
    // reason to go back to the whole spectrum.
    const y1 = Math.min(wanted, view.y0 + (view.y1 - view.y0) * 2);
    if (y1 > view.y1) replace({ ...view, y1 });
  }

  /** Changes the view without adding to the zoom history, which is for the zooms
      that were asked for. */
  function replace(next: View) {
    view = next;
    history = amend(history, next);
  }

  let shownReserve = 0;
  // Labels appear long after the view was fitted, so the fit follows them rather
  // than waiting for F. Keyed on the room they need, which also moves when the
  // window is resized or the spectra are stacked, and never on the view itself,
  // which is what keeps this from feeding itself.
  $effect(() => {
    const reserve = labelReserve();
    const ready = docs.every((d) => buffers[d.id]);
    if (!ready || Math.abs(reserve - shownReserve) <= 0.01) return;
    shownReserve = reserve;
    if (reserve > 0 && !chosen) untrack(makeRoomForLabels);
  });

  async function startPhasing() {
    if (phasing) return;
    try {
      const current = findPhase(active.operations);
      const pivot = current?.pivot ?? (await largestPeak(active.id));
      phasing = { p0: current?.p0 ?? 0, p1: current?.p1 ?? 0, pivot };
    } catch (e) {
      onerror(String(e));
    }
  }

  function setPhase(patch: Partial<PhaseParams>) {
    if (phasing) phasing = { ...phasing, ...patch };
  }

  /** Moves the pivot without changing the displayed phase. */
  function movePivot(pivot: number) {
    if (!phasing) return;
    const p = Math.min(Math.max(pivot, 0), 1);
    phasing = { ...phasing, p0: phasing.p0 + phasing.p1 * (p - phasing.pivot), pivot: p };
  }

  async function biggestPivot() {
    try {
      movePivot(await largestPeak(active.id));
    } catch (e) {
      onerror(String(e));
    }
  }

  async function autoPhasing() {
    if (!phasing) return;
    try {
      phasing = await autoPhase(active.id, phasing);
    } catch (e) {
      onerror(String(e));
    }
  }

  async function startPointing() {
    if (pointing || phasing) return;
    try {
      const current = active.operations.find((o) => o.op === 'baseline_points');
      pointing = current
        ? structuredClone($state.snapshot(current))
        : {
            ...DEFAULT_POINTS,
            points: (await proposeBaselinePoints(active.id)).map((at) => ({ at })),
          };
      pointsPreview = false;
    } catch (e) {
      onerror(String(e));
    }
  }

  function setPoints(patch: Partial<PointsOperation>) {
    if (pointing) pointing = { ...pointing, ...patch };
  }

  async function automaticPoints() {
    try {
      const proposed = await proposeBaselinePoints(active.id);
      setPoints({ points: proposed.map((at) => ({ at })) });
    } catch (e) {
      onerror(String(e));
    }
  }

  /** The first and the last point of the spectrum, where a baseline most often
      bends and the automatic points stay away from. */
  function borderPoints() {
    if (!pointing) return;
    const ends = [0, (active.points - 1) / active.points];
    const kept = pointing.points.filter((p) => !ends.includes(p.at));
    setPoints({ points: [...kept, ...ends.map((at) => ({ at }))] });
  }

  function applyPoints() {
    if (!pointing) return;
    onbaselinepoints($state.snapshot(pointing));
    pointing = undefined;
  }

  const pointPpm = (at: number) => active.firstPpm + at * active.points * active.deltaPpm;
  const pointAt = (x: number) =>
    Math.min(Math.max(indexOf(active, ppmAt(view, x, width)) / active.points, 0), 1);
  /** The value of the active spectrum, before its baseline, at a pixel row. */
  const valueAt = (y: number) =>
    (intensityAt(view, y, height) - baseOffset(active, activePosition)) * traceScale(active);

  /** Pixel position of point `k`: on the baseline, or on zero in the preview. */
  function pointPx(k: number): [number, number] | undefined {
    const at = pointing?.points[k]?.at;
    // Until the answer for these very points is in, its heights are another's.
    if (handSet?.key !== pointsKey) return undefined;
    const [value, under] = handSet.points[k] ?? [];
    if (at === undefined || value === undefined || under === undefined) return undefined;
    const shown = pointsPreview ? value - under : value;
    return [
      pxOfPpm(view, pointPpm(at), width),
      pyOfRow(shown / traceScale(active), active, activePosition),
    ];
  }

  function pointNear(x: number, y: number): number | undefined {
    let best: number | undefined;
    let distance = POINT_GRAB_PX;
    pointing?.points.forEach((_, k) => {
      const at = pointPx(k);
      const d = at ? Math.hypot(at[0] - x, at[1] - y) : Infinity;
      if (d <= distance) [best, distance] = [k, d];
    });
    return best;
  }

  function ondblclick(e: MouseEvent) {
    if (!pointing) return;
    const k = pointNear(e.offsetX / zoom, e.offsetY / zoom);
    if (k !== undefined) setPoints({ points: pointing.points.filter((_, i) => i !== k) });
  }

  function applyZoomTo() {
    if (!zoomTo) return;
    const [from, to] = [Number(zoomTo.from), Number(zoomTo.to)];
    if (Number.isFinite(from) && Number.isFinite(to) && from !== to) {
      const [x0, x1] = from > to ? [from, to] : [to, from];
      commit({ ...view, x0, x1 });
    }
    zoomTo = undefined;
  }

  function applyReference(shift: number) {
    if (referencing === undefined) return;
    onreference(referencing, shift);
    referencing = undefined;
    mode = 'pointer';
  }

  function applyPhase() {
    if (!phasing) return;
    onphase($state.snapshot(phasing));
    phasing = undefined;
  }

  function onkeydown(e: KeyboardEvent) {
    if (!focused || isEditable(e.target)) return;
    const command = e.metaKey || e.ctrlKey;
    if (zoomTo && (e.key === 'Enter' || e.key === 'Escape')) {
      if (e.key === 'Enter') applyZoomTo();
      else zoomTo = undefined;
      e.preventDefault();
      return;
    }
    if (referencing !== undefined && e.key === 'Escape') {
      referencing = undefined;
      e.preventDefault();
      return;
    }
    // The plot's own windows are answered above; any other is not the plot's.
    if (windowOpen()) return;
    if (command && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'k') {
      mode = 'peakByPeak';
      e.preventDefault();
      return;
    }
    if (command || e.altKey) return;
    if (phasing && (e.key === 'Enter' || e.key === 'Escape')) {
      if (e.key === 'Enter') applyPhase();
      else phasing = undefined;
    } else if (pointing && (e.key === 'Enter' || e.key === 'Escape')) {
      if (e.key === 'Enter') applyPoints();
      else pointing = undefined;
    } else if (e.key === ' ') {
      spaceHeld = true;
    } else if (e.shiftKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      const step = e.key === 'ArrowUp' ? 1 : -1;
      const next = docs[activePosition + step];
      if (next) onactivate(next.id);
    } else if (viewAction(e)) {
      runView(viewAction(e));
    } else {
      switch (e.key.toLowerCase()) {
        case 'p':
          void startPhasing();
          break;
        case 'b':
          // B alone is the automatic baseline, which the window answers.
          if (!e.shiftKey) return;
          void startPointing();
          break;
        case 'i':
          if (e.shiftKey) return;
          mode = 'integrate';
          break;
        case 'j':
          if (e.shiftKey) return;
          // J analyses multiplets.
          mode = 'multiplet';
          break;
        case 'k':
          if (e.shiftKey) return;
          mode = 'peaks';
          break;
        case 'l':
          mode = 'reference';
          break;
        default:
          return;
      }
    }
    e.preventDefault();
  }

  /** Opens the naming menu under a peak's label; the right click and Enter share it. */
  function peakMenu(peak: PeakLabel): NonNullable<typeof menu> {
    return {
      kind: 'peak',
      index: peak.index,
      x: peak.label,
      y: labelBand(activePosition) + LABEL_TICK + 10,
      // Starts from the name on screen, so a suggestion can be corrected
      // rather than retyped: a benzene that is not benzene is one edit.
      label: nameOnScreen(peak.index, peak.ppm),
      from: nameOnScreen(peak.index, peak.ppm),
    };
  }

  /** What a screen reader is told when the keyboard steps to something. */
  let announced = $state('');
  /** Which kind of thing the arrow keys walk; Up and Down change it. */
  let walking = $state<'peak' | 'integral' | 'multiplet'>('peak');

  /** What the keyboard can reach, highest shift first: the plot's own objects,
   *  otherwise reached by pointer only. Up and Down choose the kind, Left and
   *  Right step through it, Enter names a peak and Delete takes one away. */
  function onplotkey(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
    const { peaks, integrals, multiplets } = active.analysis;
    const items = (
      walking === 'peak'
        ? peaks.map((ppm, index) => ({ ppm, index }))
        : (walking === 'integral' ? integrals : multiplets.map((m) => [m.from, m.to])).map(
            (range, index) => ({ ppm: (range[0] + range[1]) / 2, index }),
          )
    ).sort((a, b) => b.ppm - a.ppm);
    const current =
      walking === 'peak' ? hovered : walking === 'integral' ? hoveredIntegral : hoveredMultiplet;
    const at = items.findIndex((item) => item.index === current);
    const mark = (index: number | undefined) => {
      hovered = walking === 'peak' ? index : undefined;
      hoveredIntegral = walking === 'integral' ? index : undefined;
      hoveredMultiplet = walking === 'multiplet' ? index : undefined;
    };
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      const kinds = ['peak', 'integral', 'multiplet'] as const;
      walking = kinds[(kinds.indexOf(walking) + (e.key === 'ArrowDown' ? 1 : 2)) % 3];
      mark(undefined);
      announced = `Stepping through ${walking}s`;
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      if (!items.length) return void (announced = `No ${walking}s`);
      const step = e.key === 'ArrowRight' ? 1 : -1;
      const to =
        items[
          at < 0 ? (step > 0 ? 0 : items.length - 1) : (at + step + items.length) % items.length
        ];
      mark(to.index);
      announced = `${walking[0].toUpperCase()}${walking.slice(1)} ${to.ppm.toFixed(decimals ?? 2)} ppm, ${items.indexOf(to) + 1} of ${items.length}`;
    } else if (at >= 0 && walking === 'peak' && e.key === 'Enter') {
      const peak = peakLabels().find((p) => p.index === hovered);
      if (peak) menu = peakMenu(peak);
    } else if (at >= 0 && (e.key === 'Delete' || e.key === 'Backspace')) {
      announced = `Removed ${walking} ${items[at].ppm.toFixed(decimals ?? 2)} ppm`;
      onremove(walking, items[at].index);
      mark(undefined);
    } else return;
    e.preventDefault();
  }

  function runView(action: ViewAction | undefined) {
    switch (action) {
      case 'zoom':
        return void ([mode, zoomAxis] = pressZoom(mode, zoomAxis));
      case 'zoomOut':
        return void (mode = 'zoomOut');
      case 'pan':
        return void ([mode, panAxis] = pressPan(mode, panAxis));
      case 'full':
        return full();
      case 'fit':
        return fitHigh();
      case 'larger':
        return scaleIntensity(1 / INTENSITY_STEP);
      case 'smaller':
        return scaleIntensity(INTENSITY_STEP);
      case 'back':
        return walk(-1);
      case 'forward':
        return walk(1);
      case 'range':
        // Manual zoom: type the two ends rather than drag them.
        return void (zoomTo = { from: view.x0.toFixed(2), to: view.x1.toFixed(2) });
      case 'crosshair':
        return void (crosshair = !crosshair);
      case 'pointer':
        return void (mode = 'pointer');
    }
  }

  function onkeyup(e: KeyboardEvent) {
    if (!focused || e.key !== ' ' || isEditable(e.target)) return;
    spaceHeld = false;
    e.preventDefault();
  }

  interface TextSizes {
    axis: number;
    peaks: number;
    integrals: number;
    multiplets: number;
    names: number;
  }

  interface Colours {
    accent: string;
    live: string;
    fg: string;
    muted: string;
    bg: string;
    integral: string;
    baselineModel: string;
    /** Each kind of thing in its own colour, where a page sets one; on screen
     *  they follow the theme and the spectra. */
    axis?: string;
    peaks?: string;
    /** Integral values and brackets; their curves keep `integral`. */
    integrals?: string;
    multiplets?: string;
    names?: string;
    /** The interface's typeface, so the plot does not write in a different one. */
    font: string;
  }

  /** What the plot paints itself with, read from the stylesheet once and then
   * kept: a computed style is a question about the whole page, and the plot
   * repaints on every move of the pointer. `forgetColours` says it is stale. */
  let painted: Colours | undefined;

  onMount(() => {
    try {
      renderer = new TraceRenderer(glCanvas);
    } catch (e) {
      onerror(String(e));
    }
    const observer = new ResizeObserver(() => {
      areaWidth = area.clientWidth;
      areaHeight = area.clientHeight;
    });
    observer.observe(area);
    overlay.addEventListener('wheel', onwheel, { passive: false });
    // A colour changes when a theme is chosen, when a scheme of one's own is
    // edited - both written on <html> - or when the system turns dark, and
    // nothing else can change one. Forgetting also redraws, which is what makes a
    // colour chosen by hand take effect at once: it changes no prop of the plot,
    // so nothing else would repaint the overlay until the pointer moved.
    const forgetColours = () => {
      painted = undefined;
      if (width > 0) {
        drawAxis();
        drawOverlay();
      }
    };
    const themeWatch = new MutationObserver(forgetColours);
    themeWatch.observe(document.documentElement, { attributeFilter: ['style', 'data-theme'] });
    const scheme = matchMedia('(prefers-color-scheme: dark)');
    scheme.addEventListener('change', forgetColours);
    const release = () => {
      spaceHeld = false;
      shiftHeld = false;
    };
    // Shift changes what a click would take and what can be taken hold of, so
    // both have to follow it being pressed, not wait for the pointer to move.
    const shiftKey = (event: KeyboardEvent) => {
      if (event.key !== 'Shift' || !focused) return;
      shiftHeld = event.type === 'keydown';
      if (drag || !cursor) return;
      grabAxis = axisOfGrab(hitTest(cursor.x, cursor.y, shiftHeld), shiftHeld);
      preview = previewAt(cursor.x, shiftHeld);
    };
    window.addEventListener('keydown', onkeydown);
    window.addEventListener('keyup', onkeyup);
    window.addEventListener('keydown', shiftKey);
    window.addEventListener('keyup', shiftKey);
    window.addEventListener('blur', release);
    return () => {
      observer.disconnect();
      themeWatch.disconnect();
      scheme.removeEventListener('change', forgetColours);
      overlay.removeEventListener('wheel', onwheel);
      window.removeEventListener('keydown', onkeydown);
      window.removeEventListener('keyup', onkeyup);
      window.removeEventListener('keydown', shiftKey);
      window.removeEventListener('keyup', shiftKey);
      window.removeEventListener('blur', release);
    };
  });

  /** How each arrangement of spectra was left - zoom, its history, the height
   *  of a stack - so going back to a page finds it as it was: starting every
   *  page over from the whole spectrum lost the zoom, and with it where each
   *  baseline had been put. */
  const kept: Record<string, { view: View; history: History; gain: number; chosen: boolean }> = {};

  $effect(() => {
    if (layout === shownLayout) return;
    untrack(() => (kept[shownLayout] = { view, history, gain, chosen }));
    shownLayout = layout;
    const before = kept[layout];
    if (before) {
      ({ view, history, gain, chosen } = before);
      return;
    }
    chosen = false;
    const fresh = untrack(fullView);
    view = fresh;
    history = historyOf(fresh);
  });

  $effect(() => {
    for (const [canvas, h] of [
      [glCanvas, height],
      [overlay, height],
      [axis, AXIS_HEIGHT],
    ] as const) {
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(h * dpr);
    }
  });

  $effect(() => {
    if (width === 0 || !renderer) return;
    const widthPx = width * dpr;
    docs.forEach((d) => {
      // A spectrum moved by hand needs the points that land under the plot once
      // it has been shifted.
      const shift = shiftPpmOf(d);
      const [i0, i1] = [indexOf(d, view.x0 + shift), indexOf(d, view.x1 + shift)];
      const want = traceRequest(i0, i1, d.points, widthPx);
      const phase = d.id === active.id ? $state.snapshot(phasing) : undefined;
      // While points are set the spectrum is shown before its baseline, which
      // moving them does not change, or in the preview with theirs taken off.
      const points = d.id === active.id && pointing ? $state.snapshot(pointing) : undefined;
      const shownPoints = points && pointsPreview ? points : null;
      const key = phase
        ? `${phase.p0}:${phase.p1}:${phase.pivot}`
        : points
          ? `points:${JSON.stringify(shownPoints)}`
          : '';
      const have = loaded[d.id];
      const current = have?.revision === d.revision && have.phase === key;
      // One request per spectrum at a time; its result re-runs this effect, which
      // then asks for whatever changed in the meantime.
      if ((current && have && covers(have, i0, i1, want)) || inflight[d.id]) return;
      const { id, revision } = d;
      inflight[id] = true;
      const request = phase
        ? fetchPhaseTrace(id, phase, want.start, want.end, want.columns)
        : points
          ? fetchBaselineTrace(id, shownPoints, want.start, want.end, want.columns)
          : fetchTrace(id, want.start, want.end, want.columns);
      request.then(
        (xy) => {
          inflight[id] = false;
          renderer?.setData(id, xy);
          buffers[id] = xy;
          loaded[id] = { ...want, revision, phase: key };
          // Only a trace that reaches across the view now fits it: one asked for
          // before the view moved would fit the height to the range just left.
          const [a0, a1] = [indexOf(d, view.x0 + shift), indexOf(d, view.x1 + shift)];
          if (
            fitWhenLoaded &&
            id === active.id &&
            want.start <= Math.min(a0, a1) &&
            want.end >= Math.max(a0, a1)
          ) {
            fitWhenLoaded = false;
            fitHigh();
          }
        },
        (e: unknown) => {
          inflight[id] = false;
          onerror(String(e));
        },
      );
    });
  });

  // The points and the baseline through them, across the view at about a
  // sample a pixel. Only the last answer asked for is kept.
  let handSetAsked = 0;
  $effect(() => {
    const op = pointing && $state.snapshot(pointing);
    if (!op || width === 0) {
      handSet = undefined;
      return;
    }
    const last = active.points - 1;
    const ends = [indexOf(active, view.x0), indexOf(active, view.x1)];
    const start = Math.min(Math.max(Math.floor(Math.min(...ends)), 0), last);
    const end = Math.min(Math.max(Math.ceil(Math.max(...ends)), 0), last);
    const samples = Math.max(2, Math.round(width));
    const ask = ++handSetAsked;
    handSetBaseline(active.id, op, start, end, samples).then(
      (r) => {
        if (ask === handSetAsked) handSet = { ...r, start, end, key: JSON.stringify(op.points) };
      },
      (e: unknown) => onerror(String(e)),
    );
  });

  $effect(() => {
    // Every spectrum keeps its own integrals, and every one of them is drawn.
    const wanted = docs.flatMap((d) =>
      d.analysis.integrationMethod === 'fitted'
        ? []
        : d.analysis.integrals.map((range) => curveKey(d, range)),
    );
    if (Object.keys(curves).some((key) => !wanted.includes(key))) {
      curves = Object.fromEntries(Object.entries(curves).filter(([key]) => wanted.includes(key)));
    }
    for (const d of docs) {
      if (d.analysis.integrationMethod === 'fitted') continue;
      d.analysis.integrals.forEach(([from, to]) => {
        const key = curveKey(d, [from, to]);
        if (curves[key] || inflight[key]) return;
        inflight[key] = true;
        fetchIntegralCurve(d.id, from, to, CURVE_SAMPLES).then(
          (xy) => {
            inflight[key] = false;
            if (
              docs.some(
                (doc) =>
                  doc.id === d.id &&
                  doc.analysis.integrationMethod !== 'fitted' &&
                  doc.analysis.integrals.some((range) => curveKey(doc, range) === key),
              )
            )
              curves[key] = xy;
          },
          (e: unknown) => {
            inflight[key] = false;
            if (docs.some((doc) => doc.id === d.id && doc.analysis.integrationMethod !== 'fitted'))
              onerror(String(e));
          },
        );
      });
    }
  });

  $effect(() => {
    if (!renderer || width === 0 || height === 0) return;
    renderer.draw(
      docs.flatMap((d, k) => {
        const have = loaded[d.id];
        if (!have) return [];
        const [s, offset] = [traceScale(d), baseOffset(d, k)];
        const shift = shiftPpmOf(d, k);
        // A stack gives every spectrum a fixed row; otherwise they share the view.
        const [y0, y1] =
          stack === 'stacked'
            ? rowRange(k + FOOT + adjustOf(d).dy, rows, s, gain)
            : [(view.y0 - offset) * s, (view.y1 - offset) * s];
        const bufferView: Vec4 = [
          indexOf(d, view.x0 + shift) - have.start,
          indexOf(d, view.x1 + shift) - have.start,
          y0,
          y1,
        ];
        // Every spectrum is drawn at full strength: fading the others suggested
        // that zooming would only touch the one selected, which it does not. The
        // row's name says which spectrum the panel and the analysis belong to.
        return [
          { key: d.id, view: bufferView, color: hexColor(colorOf(d, k)), clip: rowClip(d, k) },
        ];
      }),
    );
  });

  $effect(() => {
    void palette;
    void ink;
    if (width === 0) return;
    drawAxis();
    drawOverlay();
  });

  function colors(): Colours {
    const theme = themeColours();
    return ink ? { ...theme, ...ink } : theme;
  }

  function themeColours(): Colours {
    if (painted) return painted;
    const style = getComputedStyle(overlay);
    const get = (name: string) => style.getPropertyValue(name).trim();
    painted = {
      accent: get('--accent'),
      live: get('--live') || get('--accent'),
      fg: get('--fg'),
      muted: get('--muted'),
      bg: get('--bg'),
      integral: get('--integral') || get('--accent'),
      baselineModel: get('--baseline-model') || get('--accent'),
      font: get('--font') || 'system-ui, sans-serif',
    };
    return painted;
  }

  function painter(canvas: HTMLCanvasElement, h: number) {
    const ctx = canvas.getContext('2d');
    if (!ctx) return undefined;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, h);
    return new CanvasPainter(ctx, colors().font);
  }

  /** Ticks and ppm labels hanging below the line `y = top`. */
  function paintAxis(p: Painter, top: number, color: string) {
    paintScale(p, {
      from: view.x0,
      to: view.x1,
      length: width,
      top,
      color,
      size: sizes.axis,
      gap: 8 * axisScale,
      title: axisTitle,
    });
  }

  interface PeakLabel {
    index: number;
    ppm: number;
    /** What it has been called, if anything was chosen. */
    name?: string;
    /** Where the peak is, and where its label had to move to stay readable. */
    x: number;
    label: number;
    top: number;
  }

  /** Peak labels packed along the top band, left to right, at least LABEL_GAP apart. */
  function peakLabels(d: DocumentInfo = active, position: number = activePosition): PeakLabel[] {
    const scale = traceScale(d);
    const shift = shiftPpmOf(d, position);
    const placed = d.analysis.peaks
      .map((ppm, index) => {
        return {
          index,
          ppm,
          name: d.analysis.labels[index] ?? undefined,
          x: pxOfPpm(view, ppm - shift, width),
          top: pyOfRow((d.peakIntensities[index] ?? 0) / scale, d, position),
        };
      })
      .filter((peak) => peak.x >= -LABEL_GAP && peak.x <= width + LABEL_GAP)
      .sort((a, b) => a.x - b.x);
    let previous = -Infinity;
    return placed.map((peak) => {
      const label = Math.max(peak.x, previous + LABEL_GAP);
      previous = label;
      return { ...peak, label };
    });
  }

  /** A multiplet's chip, which is clicked rather than dragged and so is offered
      whether or not anything is being moved. */
  function multipletChip(x: number, y: number) {
    const box = chips.find(
      (chip) => x >= chip.x && x <= chip.x + chip.w && y >= chip.y && y <= chip.y + chip.h,
    );
    return box ? { kind: 'multiplet' as const, index: box.index, edge: undefined } : undefined;
  }

  /** The range integral `index` is drawn at: the edge being dragged follows the
      pointer, so the bracket and its grips move while they are moved rather than
      jumping when they are let go. The curve under them keeps its last shape,
      since it is worked out in the back end and a range that changes with every
      pixel would ask for it again with every pixel; it catches up when the drag
      ends. */
  function draggedRange(d: DocumentInfo, index: number, range: Range): Range {
    const held =
      drag?.kind === 'integral' &&
      drag.index === index &&
      d.id === active.id &&
      dragPpm !== undefined;
    if (!held) return range;
    return (drag?.edge === 0 ? [dragPpm, range[1]] : [range[0], dragPpm]) as Range;
  }

  /** The area under a range, read off what is already in memory, so the number
      can follow the pointer without asking the back end for it with every pixel.
      `curve` is cumulative from its own `from` edge, so any shift inside it is a
      subtraction; beyond it there is nothing to read, and the decimated trace
      that is already drawn stands in - each column averaged between its high and
      its low, since taking either would count the noise twice. What that gives is
      close rather than exact, and the exact value arrives when the drag ends. */
  function areaOf(d: DocumentInfo, curve: Float32Array, stored: Range, at: Range): number {
    const along = (ppm: number) => {
      const first = curve[0];
      const last = curve[curve.length - 2];
      const inside = ppm <= Math.max(first, last) && ppm >= Math.min(first, last);
      if (!inside) return undefined;
      for (let k = 2; k < curve.length; k += 2) {
        const [before, after] = [curve[k - 2], curve[k]];
        if (ppm > Math.max(before, after) || ppm < Math.min(before, after)) continue;
        const span = after - before;
        const part = span === 0 ? 0 : (ppm - before) / span;
        return curve[k - 1] + part * (curve[k + 1] - curve[k - 1]);
      }
      return curve[curve.length - 1];
    };
    const whole = curve[curve.length - 1];
    const ends = [along(at[0]), along(at[1])];
    if (ends[0] !== undefined && ends[1] !== undefined) return Math.abs(ends[1] - ends[0]);
    // Past the end of the curve: the trace itself, as far as it is drawn.
    const have = loaded[d.id];
    const xy = buffers[d.id];
    if (!have || !xy) return whole;
    const dx = Math.abs(d.deltaPpm);
    const sum = (from: number, to: number) => {
      const [lo, hi] = [Math.min(from, to), Math.max(from, to)];
      let total = 0;
      for (let k = 0; k < xy.length; k += 4) {
        const ppm = d.firstPpm + (have.start + xy[k]) * d.deltaPpm;
        if (ppm < lo || ppm > hi) continue;
        const high = xy[k + 1];
        const low = k + 3 < xy.length ? xy[k + 3] : high;
        total += ((high + low) / 2) * dx * (xy[k + 2] - xy[k] || 1);
      }
      return Math.abs(total);
    };
    const grown =
      sum(Math.min(...at), Math.max(...at)) - sum(Math.min(...stored), Math.max(...stored));
    return Math.max(0, whole + grown);
  }

  /** Where a spectrum's integral bracket is drawn, which is also where its ends
      are taken hold of. Both read it here, so neither can drift from the other. */
  function bracketY(d: DocumentInfo, position: number): number {
    return Math.min(pyOfRow(0, d, position) + 8 + movedBy(d, 'bracket'), height - 16);
  }

  /** The ends of multiplet `index` as they are drawn: the one being dragged
      follows the pointer, so the band and its grips move while they are moved. */
  function multipletEnds(d: DocumentInfo, index: number): [number, number] {
    const m = d.analysis.multiplets[index];
    // Narrowed here rather than through a flag: a condition kept in a variable
    // does not tell the compiler that `dragPpm` is a number by the time it is read.
    const at = dragPpm;
    const held =
      drag?.kind === 'multipletEnd' &&
      drag.index === index &&
      d.id === active.id &&
      at !== undefined;
    if (!held || at === undefined) return [m.from, m.to];
    return drag?.edge === 0 ? [at, m.to] : [m.from, at];
  }

  /** An end of any multiplet under the pointer. Asked of them all rather than of
      the one being pointed at, or letting go of the chip would let go of its
      ends - which is what going to a grip does. */
  function multipletEnd(x: number, y: number) {
    if (!showMultiplets) return undefined;
    // The nearest edge wins: on a narrow multiplet both are within reach, and
    // taking the first one found moved the far side.
    let best: { index: number; at: number; away: number } | undefined;
    for (const band of bands) {
      if (y < band.top || y > band.bottom) continue;
      for (const at of [band.x0, band.x1]) {
        const away = Math.abs(at - x);
        if (away <= EDGE_GRAB && (!best || away < best.away))
          best = { index: band.index, at, away };
      }
    }
    if (!best) return undefined;
    // The ends are kept as from and to, which the pixels need not follow.
    const [from] = multipletEnds(active, best.index);
    const px = pxOfPpm(view, from - shiftPpmOf(active), width);
    const edge: 0 | 1 = Math.abs(px - best.at) < 1 ? 0 : 1;
    return { kind: 'multipletEnd' as const, index: best.index, edge };
  }

  /** The multiplet whose region the pointer is in, from its name down to the
      baseline, so that the name, the lines and the two edges are one thing. */
  function multipletRow(x: number, y: number): number | undefined {
    if (!showMultiplets) return undefined;
    const band = bands.find(
      (b) => x >= b.x0 - EDGE_GRAB && x <= b.x1 + EDGE_GRAB && y >= b.top && y <= b.bottom,
    );
    return band?.index;
  }

  /** Whether a peak lies in the multiplet lit from outside. */
  function inMarked(ppm: number): boolean {
    const m = marked === undefined ? undefined : active.analysis.multiplets[marked];
    return !!m && ppm <= m.from && ppm >= m.to;
  }

  /** The multiplet a peak belongs to, by where it lies. */
  const multipletOfPeak = (peak: number | undefined) => {
    const ppm = peak === undefined ? undefined : active.analysis.peaks[peak];
    return ppm === undefined
      ? undefined
      : active.analysis.multiplets.findIndex((m) => ppm <= m.from && ppm >= m.to);
  };

  $effect(() => {
    const found = hoveredMultiplet ?? multipletOfPeak(hovered);
    const index = found === undefined || found < 0 ? undefined : found;
    untrack(() => onhovermultiplet?.(index));
  });

  /** The end of an integral under the pointer: how a range is edited. Only the
      active spectrum's, as the peak labels are. */
  function integralHandle(x: number, y: number) {
    const d = active;
    if (Math.abs(y - bracketY(d, activePosition)) > HANDLE / 2 + 3) return undefined;
    const shift = shiftPpmOf(d);
    for (const [index, range] of d.analysis.integrals.entries()) {
      for (const edge of [0, 1] as const) {
        if (Math.abs(pxOfPpm(view, range[edge] - shift, width) - x) <= HANDLE / 2 + 3) {
          return { kind: 'integral' as const, index, edge };
        }
      }
    }
    return undefined;
  }

  /** The integral whose bracket the pointer is on, end to end. Its ends are
      drawn as grips while it is: a grip drawn where there is nothing to take is
      clutter, and one that is never drawn is never found. */
  function integralRow(x: number, y: number): number | undefined {
    const d = active;
    if (Math.abs(y - bracketY(d, activePosition)) > HANDLE / 2 + 3) return undefined;
    const shift = shiftPpmOf(d);
    const index = d.analysis.integrals.findIndex((range, i) => {
      const at = draggedRange(d, i, range);
      const ends = [at[0], at[1]].map((edge) => pxOfPpm(view, edge - shift, width));
      return x >= Math.min(...ends) - HANDLE && x <= Math.max(...ends) + HANDLE;
    });
    return index < 0 ? undefined : index;
  }

  /** An integral's end runs along the axis; everything else that can be taken
      hold of moves up and down. */
  function axisOfGrab(hit: { kind: string } | undefined, shift: boolean): '' | 'x' | 'y' | 'click' {
    if (!hit) return '';
    if (hit.kind === 'integral' || hit.kind === 'multipletEnd') return 'x';
    // A chip is clicked; its band only moves with Shift held, so the arrow that
    // says it moves is only shown then.
    if (hit.kind === 'multiplet') return shift ? 'y' : 'click';
    return 'y';
  }

  /** Where the threshold line is written, which is also the only place it can be
      taken hold of. Reckoned from the text rather than measured, as the chip is,
      since the SVG painter cannot measure one and the two have to agree. */
  const THRESHOLD_X = 8;
  /** Height of the pill a guide line is named and taken hold of by. */
  const PILL_H = 20;
  const thresholdLabel = () => `↕ ${(threshold / (active.noise || 1)).toFixed(1)} × noise`;
  const pillWidth = (text: string) => text.length * CHIP_CHAR + CHIP_PAD * 2;
  const thresholdGrabsTo = () => THRESHOLD_X + pillWidth(thresholdLabel());

  /** A line the hand sets rather than the data - the threshold, the pivot, the
      shift being picked - is drawn thin and solid in the accent, faded, and
      named in a filled pill as the rest of the interface names things. Dashed,
      with loose text beside it, it read as something from another program. */
  function guide(p: Painter, c: Colours, x0: number, y0: number, x1: number, y1: number) {
    const points =
      x0 === x1
        ? [x0 - 0.5, y0, x0 + 0.5, y0, x0 + 0.5, y1, x0 - 0.5, y1]
        : [x0, y0 - 0.5, x1, y0 - 0.5, x1, y0 + 0.5, x0, y0 + 0.5];
    p.shape(points, c.accent, 0.6);
  }

  /** How much black darkens the page by a similar step whatever the theme: a
      light page shows a little of it, a dark one needs more to show at all. */
  function dim(background: string): number {
    const hex = background.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i);
    const rgb = background.match(/(\d+)\D+(\d+)\D+(\d+)/);
    const parts = hex
      ? hex.slice(1, 4).map((h) => parseInt(h, 16))
      : rgb
        ? rgb.slice(1, 4).map(Number)
        : [255, 255, 255];
    const light = (0.2126 * parts[0] + 0.7152 * parts[1] + 0.0722 * parts[2]) / 255;
    return light > 0.5 ? 0.07 : 0.4;
  }

  function pill(p: Painter, c: Colours, left: number, middle: number, text: string) {
    const w = pillWidth(text);
    p.chip(left, middle - PILL_H / 2, w, PILL_H, c.accent, c.accent, PILL_H / 2);
    p.text(left + w / 2, middle, text, {
      color: c.bg,
      size: 11,
      align: 'center',
      baseline: 'middle',
      bold: true,
    });
  }

  /** A vertical guide's pill, low beside its line and never off the plot: at the
      top it went under the hint that says what the tool does. */
  function pillBeside(p: Painter, c: Colours, x: number, text: string) {
    const left = Math.min(x + 6, width - pillWidth(text) - 4);
    pill(p, c, left, height - PILL_H / 2 - 8, text);
  }

  /** What is under the pointer, if it can be taken hold of. */
  function hitTest(x: number, y: number, moving = false) {
    if (phasing) return undefined;
    if (tool === 'peaks') {
      // At its label and nowhere else. Tested along the whole width, this was a
      // band twelve pixels high lying across the plot at the foot of the signals,
      // and a drag that began anywhere in it moved the threshold instead of
      // picking: whether a peak could be taken depended on the height the drag
      // started at rather than on the peak.
      const line = pyOfRow(threshold / traceScale(active), active, activePosition);
      return Math.abs(y - line) <= PILL_H / 2 && x >= THRESHOLD_X && x <= thresholdGrabsTo()
        ? { kind: 'threshold' as const, index: undefined, edge: undefined }
        : undefined;
    }
    if (tool !== 'pointer') return undefined;
    // A multiplet's chip is the way into its window, and it is small, so it is
    // asked about before anything larger that lies under it.
    const end = multipletEnd(x, y);
    if (end) return end;
    const chip = multipletChip(x, y);
    if (chip) return chip;
    const handle = integralHandle(x, y);
    if (handle) return handle;
    // The curve, the bracket and the multiplet's rule are all moved the way the
    // peak labels are: taken hold of where they are drawn, but only with Shift
    // held. They lie across the plot, and without it ordinary work moved them.
    const d = active;
    if (!moving) return undefined;
    // The baseline of any spectrum, with Shift as everything lying across the
    // plot is, moved up or down; taking it picks that spectrum.
    const onBaseline = docs.findIndex((d, k) => Math.abs(y - pyOfRow(0, d, k)) <= GRAB_PX);
    if (onBaseline >= 0) {
      return { kind: 'baseline' as const, index: onBaseline, edge: undefined };
    }
    if (
      showCurves &&
      d.analysis.integrationMethod !== 'fitted' &&
      d.analysis.integrals.length > 0
    ) {
      if (curveFeet.some((f) => x >= f.x0 && x <= f.x1 && Math.abs(y - f.y) <= GRAB_PX)) {
        return { kind: 'curve' as const, index: undefined, edge: undefined };
      }
    }
    if (d.analysis.integrals.length > 0) {
      const bar = bracketY(d, activePosition);
      if (Math.abs(y - bar) <= GRAB_PX) {
        return { kind: 'bracket' as const, index: undefined, edge: undefined };
      }
    }

    const band = labelBand(activePosition);
    // A label that is not drawn cannot be taken hold of.
    const label =
      showLabels && !compact()
        ? peakLabels().find(
            (peak) => Math.abs(peak.label - x) <= GRAB_PX && Math.abs(y - band) <= LABEL_TICK + 6,
          )
        : undefined;
    if (label) return { kind: 'peak' as const, index: label.index, edge: undefined };
    return undefined;
  }

  /** The name of each spectrum at the top left of its own row, in its own colour,
      so a series can be read and clicked without going back to the list. */
  function paintRowNames(p: Painter, c: ReturnType<typeof colors>) {
    if (stack !== 'stacked' || docs.length < 2) return;
    docs.forEach((d, k) => {
      const factor = adjustOf(d).scale;
      const shown = nameOf(d);
      const label = factor === 1 ? shown : `${shown}   ×${factor.toFixed(2)}`;
      p.text(14, height * (1 - (k + FOOT + 1) / rows) + 16, label, {
        color: c.names ?? (highlighted && d.id === active.id ? c.fg : colorOf(d, k)),
        size: sizes.names,
      });
    });
  }

  /** How far under a baseline its integrals reach, brackets and values, as last
   *  drawn: what a fit leaves room for, so that the lowest thing is what sits
   *  the chosen distance above the axis. Known only once they are laid out, as
   *  the rows their values take depend on where they fall. */
  let integralDepth = 0;

  /** Where the baseline goes in a fit: the chosen gap, with the integrals'
   *  depth under it. */
  function baselineShare(): number | undefined {
    if (baselineAt === undefined || height <= 0) return baselineAt;
    const depth = showIntegrals && active.analysis.integrals.length > 0 ? integralDepth : 0;
    return Math.min(baselineAt + depth / height, 0.8);
  }

  function paintAnalysis(p: Painter, c: ReturnType<typeof colors>) {
    integralDepth = 0;
    paintRowNames(p, c);
    // Each spectrum's integrals are drawn against its own baseline, so a stack
    // does not gather them all at the foot of the window.
    docs.forEach((d, position) => {
      paintMultiplets(p, c, d, position);
      paintIntegrals(p, c, d, position);
    });
    if (!showLabels) return;
    labelledDocs().forEach((d) => {
      const position = docs.indexOf(d);
      const band = labelBand(position, d);
      const ink = c.peaks ?? annotationInk(c, d, position);
      if (compact()) {
        paintCompactLabels(p, d, position, ink);
        return;
      }
      for (const peak of peakLabels(d, position)) {
        paintPeakLabel(p, c, peak, band, ink);
      }
    });
  }

  /** A short row's labels: each shift level over its own peak, left to right, one
      dropped where it would overlap the last, and none under a multiplet, whose
      name already says where it is. */
  function paintCompactLabels(p: Painter, d: DocumentInfo, position: number, ink: string) {
    const ranges = showMultiplets ? d.analysis.multiplets : [];
    const ceiling = rowTop(d, position) + COMPACT_LINE;
    let reach = -Infinity;
    for (const peak of peakLabels(d, position)) {
      const inside = ranges.some(
        (m) => peak.ppm <= Math.max(m.from, m.to) && peak.ppm >= Math.min(m.from, m.to),
      );
      if (inside) continue;
      const text = fixed(peak.ppm, decimals);
      const half = (text.length * CHIP_CHAR) / 2;
      if (peak.x - half < reach + 4) continue;
      reach = peak.x + half;
      p.text(peak.x, Math.max(ceiling, peak.top - 4), text, {
        color: ink,
        size: sizes.peaks,
        align: 'center',
      });
    }
  }

  /** The integral curves of one spectrum, with their ranges and values. */
  /** What a spectrum is lettered in, unless a colour of its own is chosen: the
      text colour for the spectrum being worked on, its own colour for the rest. */
  function annotationInk(c: Colours, d: DocumentInfo, position: number): string {
    return highlighted && d.id === active.id ? c.fg : colorOf(d, position);
  }

  function paintIntegrals(
    p: Painter,
    c: ReturnType<typeof colors>,
    d: DocumentInfo,
    position: number,
  ) {
    if (d.id === active.id) curveFeet = [];
    if (!showIntegrals || crowded(d)) return;
    const integrals = d.analysis.integrals;
    const data = integrals.map((range) => curves[curveKey(d, range)]);
    const tallest = Math.max(
      0,
      ...data.map((xy) => (xy?.length ? Math.abs(xy[xy.length - 1]) : 0)),
    );
    const shift = shiftPpmOf(d, position);
    // Lettered as its peaks are, in the spectrum's colour or the one chosen: in
    // the grey of the curves the values were hard to read.
    const letters = c.integrals ?? annotationInk(c, d, position);
    const ceiling = clearFor(d, position) - CURVE_HEIGHT - CURVE_GAP;
    /** Right edge of the values already written on each row, left to right. */
    const rowsEnd: number[] = [];
    integrals.forEach((_, i) => {
      const xy = data[i];
      if (
        xy &&
        tallest !== 0 &&
        showCurves &&
        d.analysis.integrationMethod !== 'fitted' &&
        !compact()
      ) {
        // Standing on the signal it measures, just clear of its highest point,
        // and as tall as its share of the largest integral: over the lines rather
        // than across them.
        const [ex0, ex1] = [xy[0], xy[xy.length - 2]].map((ppm) =>
          pxOfPpm(view, ppm - shift, width),
        );
        const top = topIn(d, position, ex0, ex1) ?? pyOfRow(0, d, position);
        const rise = Math.max(8, (Math.abs(xy[xy.length - 1]) / tallest) * CURVE_HEIGHT);
        // Never below its own baseline: on a small page with large labels the
        // room kept for them reached past it, and the curves hung down under
        // the spectrum.
        const foot =
          Math.min(Math.max(ceiling + rise, top - CURVE_GAP), pyOfRow(0, d, position) - 2) +
          movedBy(d, 'curve');
        const points = new Float32Array(xy.length);
        for (let k = 0; k < xy.length; k += 2) {
          points[k] = pxOfPpm(view, xy[k] - shift, width);
          points[k + 1] = foot - (xy[k + 1] / xy[xy.length - 1]) * rise;
        }
        p.line(points, c.integral);
        if (d.id === active.id)
          curveFeet.push({ x0: Math.min(ex0, ex1), x1: Math.max(ex0, ex1), y: foot });
      }
      const [from, to] = draggedRange(d, i, integrals[i]);
      // The range and its value sit just under that same baseline, bracketed.
      const [x0, x1] = [from, to].map((edge) => pxOfPpm(view, edge - shift, width));
      const bar = bracketY(d, position);
      // While an end is held, a line down the plot says where it falls against the
      // signal: the bracket is at the foot and the lines being cut are at the top,
      // so the two are a plot apart and the eye has nothing to carry between them.
      // Dashed and in the accent, as the phase pivot is, since it belongs to the
      // hand rather than to the drawing, and only the end being moved is in doubt.
      if (d.id === active.id && drag?.kind === 'integral' && drag.index === i) {
        const held = Math.round(drag.edge === 0 ? x0 : x1) + 0.5;
        guide(p, c, held, 0, held, height);
      }
      p.line([x0, bar, x1, bar], letters);
      // The ends mark the range at rest and become grips while the pointer is on
      // that bracket, filled in the integral's own colour so they read as
      // something to take hold of rather than as more of the drawing.
      const grips =
        d.id === active.id &&
        (hoveredIntegral === i || (drag?.kind === 'integral' && drag.index === i));
      for (const x of [x0, x1]) {
        if (grips) {
          p.chip(x - HANDLE / 2, bar - HANDLE / 2, HANDLE, HANDLE, letters, letters, HANDLE_RADIUS);
        } else {
          p.line([x, bar - 4, x, bar + 4], letters);
        }
      }
      // The value is the area times one factor for the whole spectrum, so while
      // an end is held the number is that area again over the range on screen.
      const stored = integrals[i];
      const shown = d.integralValues[i];
      const moving = from !== stored[0] || to !== stored[1];
      const whole = xy?.[xy.length - 1] ?? 0;
      const value =
        shown == null ||
        (moving && (d.analysis.integrationMethod === 'fitted' || !xy || whole === 0))
          ? 'Unavailable'
          : (moving && xy && whole !== 0
              ? (areaOf(d, xy, stored, [from, to]) * shown) / Math.abs(whole)
              : shown
            ).toFixed(2);
      // Two values that would overlap are set a row apart, rather than printed
      // over each other.
      const centre = (x0 + x1) / 2;
      const half = (value.length * CHIP_CHAR * 1.1) / 2;
      let row = 0;
      while ((rowsEnd[row] ?? -Infinity) > Math.min(x0, x1, centre - half) - 4) row++;
      rowsEnd[row] = Math.max(x0, x1, centre + half);
      integralDepth = Math.max(
        integralDepth,
        bar + 12 + row * VALUE_ROW + sizes.integrals * 0.35 - pyOfRow(0, d, position),
      );
      p.text(centre, bar + 12 + row * VALUE_ROW, value, {
        color: letters,
        size: sizes.integrals,
        align: 'center',
      });
    });
  }

  /** Rows too short for the band of upright labels: each spectrum of the stack is
      then labelled small and level over its own peaks, inside its row. */
  function compact(): boolean {
    return stack === 'stacked' && docs.length > 1 && height / rows < LABEL_ROW_MIN;
  }

  /** In rows that short, only the spectrum being worked on carries its integrals
      and multiplets: every row's values hung below its baseline into the names of
      the row beneath, and none could be read. A click on a row shows its own. */
  function crowded(d: DocumentInfo): boolean {
    return compact() && d.id !== active.id;
  }

  /** The top of the row spectrum `d` is drawn in, in a stack. */
  function rowTop(d: DocumentInfo, position: number): number {
    return pyOfRow(0, d, position) - height / rows;
  }

  /** Where a spectrum's multiplet rule is drawn, which is also where it is taken
      hold of. */
  function multipletRule(position: number, d: DocumentInfo): number {
    return (
      labelBand(position, d) +
      LABEL_TICKS +
      MULTIPLET_GAP +
      MULTIPLET_BAND -
      6 +
      movedBy(d, 'multiplet')
    );
  }

  /** Each multiplet's range, marked under the labels with what it is called: the
      mark says which lines were read together, which is the thing to check. */
  function paintMultiplets(
    p: Painter,
    c: ReturnType<typeof colors>,
    d: DocumentInfo,
    position: number,
  ) {
    if (d.id === active.id) {
      chips = [];
      bands = [];
    }
    if (!showMultiplets || crowded(d)) {
      if (d.id === active.id) onoverlap?.(0);
      return;
    }
    const shift = shiftPpmOf(d, position);
    const ruled = multipletRule(position, d);
    const base = pyOfRow(0, d, position);

    // What each multiplet would be if it were ideal: its lines at its own width,
    // standing as high as the nuclei it holds require. Area is what a multiplet
    // means, so twice the nuclei is twice the area, whatever shape they make.
    // Worked out only when the analysis changes, not on every repaint.
    const ideal = idealOf(d);
    /** Chips held under the pointer, drawn once the rest are down. */
    const later: (() => void)[] = [];
    /** Lines from a moved name down to its lines: from x, y to x, y. */
    const leaders: [number, number, number, number][] = [];
    let overlapping = 0;

    // They share one height, so only the shape that would stand tallest needs
    // measuring against the signal - once for the spectrum rather than once for
    // every multiplet on it.
    let scale = 0;
    // Measured against a multiplet on screen: the tallest of them all may lie
    // outside the view, where there is no trace to measure, and then nothing was
    // drawn at all once the plot was zoomed in.
    const onScreen = (shape: Ideal) => {
      const [x0, x1] = [shape.m.from, shape.m.to].map((e) => pxOfPpm(view, e - shift, width));
      return Math.max(x0, x1) >= 0 && Math.min(x0, x1) <= width;
    };
    const tallest = ideal.reduce<Ideal | undefined>(
      (best, shape) =>
        onScreen(shape) &&
        shape.info &&
        shape.amplitude > 0 &&
        shape.tallest > 0 &&
        shape.amplitude * shape.tallest > (best ? best.amplitude * best.tallest : 0)
          ? shape
          : best,
      undefined,
    );
    if (tallest?.info) {
      // Against the highest point of the signal across its range, read from the
      // loaded trace: read at the multiplet's shift, a doublet was measured at the
      // valley between its lines, and the shapes came out a fraction of the height.
      const [x0, x1] = [tallest.m.from, tallest.m.to].map((e) => pxOfPpm(view, e - shift, width));
      const top = topIn(d, position, x0, x1);
      if (top !== undefined) scale = (base - top) / (tallest.amplitude * tallest.tallest);
    }

    const outlines = outlinesOf(d, ideal, scale, base, shift);
    const widthOf = (text: string) => text.length * CHIP_CHAR + CHIP_PAD * 2;
    const boxHeight = CHIP_LINE + 6;
    const headOf = (i: number) => {
      const { m, info } = ideal[i];
      const name = m.name ?? letterOf(i);
      const shiftText = fixed(info?.ppm ?? (m.from + m.to) / 2, decimals);
      return multipletLabel === 'name'
        ? name
        : multipletLabel === 'both'
          ? `${name} ${shiftText}`
          : shiftText;
    };
    const restingOf = (i: number) => {
      const kind = ideal[i].info?.kind;
      return kind ? `${headOf(i)} (${kind})` : headOf(i);
    };
    const xsOf = (i: number) => {
      const { m } = ideal[i];
      const ends = d.id === active.id ? multipletEnds(d, i) : [m.from, m.to];
      return ends.map((edge) => pxOfPpm(view, edge - shift, width));
    };

    // Each name stands just over its own lines where nothing is in the way, so it
    // is read where its signal is. The rest go into a row along the top, in the
    // order of their shifts, each pushed only as far aside as its neighbours need
    // and joined to its lines by a leader. Kept in order and never crossing, the
    // leaders always lead to the right signal; moved about one by one, names
    // crossed the solvent line and stood by other signals, which named them
    // wrongly, and one tall line lifted every name beside it.
    // Each name stands centred over its own lines, on whatever is under it, so
    // it is read where its signal is and nothing runs through it. Where a taller
    // line next to it would lift it far, it moves a little aside, never past
    // its own lines' edge; where it meets a name already placed, it goes a row
    // up. A name lifted well clear of its lines has a short leader down to them.
    // Rows, walls and names on end were tried first: with real spectra they put
    // names beside the wrong signal on long leaders.
    type Spot = { x: number; y: number; w: number; own: number; centre: number };
    const spots: (Spot | undefined)[] = [];
    if (!compact()) {
      const bandTop = Math.round(ruled - boxHeight / 2);
      const step = boxHeight + 2;
      const placedNames: Spot[] = [];
      const clashes = (x: number, y: number, w: number) =>
        placedNames.some(
          (o) => y < o.y + step && y + step > o.y && x < o.x + o.w + 6 && x + w + 6 > o.x,
        );
      const order = ideal
        .map((_, i) => i)
        .map((i) => ({ i, xs: xsOf(i) }))
        .filter(({ xs: [x0, x1] }) => !(x1 < 0 || x0 > width))
        .sort((u, v) => (u.xs[0] + u.xs[1]) / 2 - (v.xs[0] + v.xs[1]) / 2);
      for (const { i, xs } of order) {
        const [l, r] = [Math.min(...xs), Math.max(...xs)];
        const own = topIn(d, position, l, r);
        if (own === undefined) continue;
        const w = widthOf(restingOf(i));
        const centre = Math.round((l + r) / 2);
        // Standing on what is under it, from straight over its lines to a little
        // aside: the lowest wins, the nearest of equals.
        let best: { x: number; y: number; cost: number } | undefined;
        for (const k of [0, -1, 1, -2, 2]) {
          const x = Math.round(centre - w / 2 + (k * w) / 4);
          if (x < 0 || x + w > width) continue;
          // Aside only as far as its name still reaches over its own lines.
          if (x > r || x + w < l) continue;
          const under = Math.min(own, topIn(d, position, x, x + w) ?? own);
          const y = Math.max(
            bandTop,
            Math.round(under + movedBy(d, 'multiplet') - MULTIPLET_GAP - boxHeight),
          );
          const cost = -y + Math.abs(k) * 4;
          if (!best || cost < best.cost) best = { x, y, cost };
        }
        if (!best) continue;
        // Up a row at a time, at each a little to either side as well, before
        // it is let overlap.
        let { x, y } = best;
        const start = best;
        let free = !clashes(x, y, w);
        for (let row = start.y; !free && row >= bandTop; row -= step) {
          for (const k of [0, -1, 1, -2, 2, -3, 3]) {
            const at = Math.round(start.x + (k * w) / 2);
            if (at < 0 || at + w > width || clashes(at, row, w)) continue;
            // Never onto a line, nor beyond one from its own lines: aside, names
            // stood on the solvent line or across it, by other signals.
            const line = topIn(d, position, Math.min(at, l), Math.max(at + w, r));
            if (line !== undefined && line < row + boxHeight + 2) continue;
            [x, y, free] = [at, row, true];
            break;
          }
        }
        if (!free && d.id === active.id) overlapping++;
        const spot = { x, y, w, own, centre };
        placedNames.push(spot);
        spots[i] = spot;
      }
    }
    /** Names placed so far on this spectrum, in a short row. */
    const placed: { x: number; y: number; w: number }[] = [];
    ideal.forEach((shape, i) => {
      const { m } = shape;
      const [x0, x1] = xsOf(i);
      if (x1 < 0 || x0 > width) return;
      const held =
        d.id === active.id &&
        (hoveredMultiplet === i ||
          marked === i ||
          (drag?.kind === 'multipletEnd' && drag.index === i));
      // Said as a paper says it, by its shift, its name, or both, as the settings
      // choose.
      const resting = restingOf(i);
      // In the spectrum's own colour, taken towards grey: it says which spectrum
      // the multiplet belongs to without being taken for a signal.
      const tint = desaturated(colorOf(d, position));
      const ink = c.multiplets ?? annotationInk(c, d, position);
      // Where it sits is settled by its size at rest, never by what it says while
      // pointed at: laid out from the grown box, it moved out from under the
      // pointer, which unhovered it, which shrank it back, and it flickered.
      const rest = widthOf(resting);
      const centre = Math.round((x0 + x1) / 2);
      const [l, r] = [Math.min(x0, x1), Math.max(x0, x1)];
      const spot = spots[i];
      let restLeft = spot?.x ?? centre - Math.round(rest / 2);
      let boxTop = spot?.y ?? Math.round(ruled - boxHeight / 2);
      // A short row has no second line to give: a name that would overlap is left
      // out there, as a peak's shift is, and the table still says it. The names
      // run along the top of the row, clear of the integral values under the
      // baseline: set over each signal, a low one put its name on its own value.
      if (compact()) {
        boxTop = Math.round(rowTop(d, position) + COMPACT_LINE / 2 + 2 - boxHeight / 2);
        if (placed.some((o) => restLeft < o.x + o.w + 4 && restLeft + rest + 4 > o.x)) return;
        placed.push({ x: restLeft, y: boxTop, w: rest });
      }
      const top = boxTop + boxHeight;
      if (held) {
        // The region it was read from, from under its name to the baseline, and
        // its two edges as handles the whole of that height: a range is widened
        // by taking hold of its side, as in any editor, and no name can cover a
        // handle that long. The grip in the middle says it can be taken hold of.
        p.shape([l, top, r, top, r, base, l, base], tint, 0.1);
        const middle = (top + base) / 2;
        for (const at of [l, r]) {
          const edge = Math.round(at) + 0.5;
          p.line([edge, top, edge, base], tint);
          p.chip(edge - GRIP_W / 2, middle - GRIP_H / 2, GRIP_W, GRIP_H, tint, tint, GRIP_W / 2);
        }
      }
      // Pointed at, it says no more than at rest, only louder: grown, it covered
      // the names beside it. The rest is in its row of the table.
      const [left, boxY, boxWidth, boxH] = [restLeft, boxTop, rest, boxHeight];
      // The one being pointed at is drawn after all the others, so what it says
      // lies over its neighbours rather than under them. Filled with the page so
      // it stays legible over whatever it crosses; no frame of its own.
      const draw = () => {
        // Only the one pointed at, which lies over its neighbours, is backed; a
        // name at rest is clear of the lines and never knocks the trace out.
        if (held) p.chip(left, boxY, boxWidth, boxH, c.bg, c.bg, 4);
        p.text(left + boxWidth / 2, boxY + boxH / 2, resting, {
          color: ink,
          size: sizes.multiplets,
          align: 'center',
          baseline: 'middle',
          bold: held,
        });
      };
      if (held) later.push(draw);
      else draw();
      // Raised into the row, a line leads from it down to its lines.
      if (spot && spot.own - top > MULTIPLET_GAP + boxHeight) {
        leaders.push([restLeft + spot.w / 2, top + 1, centre + 0.5, spot.own - 3]);
      }
      if (d.id === active.id) {
        chips.push({ index: i, x: left, y: boxY, w: boxWidth, h: boxH, rest });
        bands.push({ index: i, x0: l, x1: r, top: boxY, bottom: base });
      }
      const points = outlines[i];
      // The switch in the tab says whether any shapes are drawn; each multiplet
      // says whether it is one of them, so asking for one does not ask for all.
      // The switch shows every multiplet's shape; one can be left out from its
      // own window. Needing both on drew nothing until every window was visited.
      if (!showSimulation || m.shape === false || !points) return;
      // Grey, and faint: an area lying behind the signal in the signal's own
      // colour is taken for another signal, and a spectrum has enough of those.
      p.shape(points, c.muted, 0.22);
      p.line(points.subarray(0, points.length - 4), c.muted);
    });
    if (d.id === active.id) onoverlap?.(overlapping);
    // Last, so every name is placed. Dashed and quiet, so a leader is never read
    // as a line of the spectrum; broken where it passes behind another name, as
    // a line through a name is unreadable and a name without its leader is lost.
    for (const [ax, ay, bx, by] of leaders) {
      const [dx, dy] = [bx - ax, by - ay];
      const gaps = chips
        .map((box) => {
          // Where along the leader (0 to 1) it is inside the box, if anywhere.
          let [enter, leave] = [0, 1];
          for (const [delta, from, lo, hi] of [
            [dx, ax, box.x - 2, box.x + box.w + 2],
            [dy, ay, box.y - 2, box.y + box.h + 2],
          ]) {
            if (delta === 0) {
              if (from <= lo || from >= hi) return undefined;
              continue;
            }
            const [u, v] = [(lo - from) / delta, (hi - from) / delta];
            [enter, leave] = [Math.max(enter, Math.min(u, v)), Math.min(leave, Math.max(u, v))];
          }
          return enter < leave ? [enter, leave] : undefined;
        })
        .filter((gap) => gap !== undefined)
        .sort((u, v) => u[0] - v[0]);
      let from = 0;
      for (const [enter, leave] of [...gaps, [1, 1]]) {
        if (enter > from) {
          p.line(
            [ax + dx * from, ay + dy * from, ax + dx * enter, ay + dy * enter],
            c.muted,
            [2, 3],
          );
        }
        from = Math.max(from, leave);
      }
    }
    for (const draw of later) draw();
  }

  /** What each multiplet of `d` would be if it were ideal. Kept until its
      analysis changes, since nothing else can alter it. */
  function idealOf(d: DocumentInfo): Ideal[] {
    if (ideals?.multiplets === d.analysis.multiplets && ideals.info === d.multipletInfo) {
      return ideals.shapes;
    }
    const shapes = d.analysis.multiplets.map((m, i) => {
      const info = d.multipletInfo[i];
      const lines = info ? simulateLines(info.ppm, info.kind, info.jsHz, d.frequencyMhz) : [];
      const half = Math.max(info?.widthHz ?? 1, 0.2) / 2 / d.frequencyMhz;
      const weight = lines.reduce((sum, line) => sum + line.weight, 0);
      const nuclei = nucleiOf(m, info);
      const amplitude = weight > 0 && nuclei !== null ? nuclei / (weight * half) : 0;
      const tallest = Math.max(0, ...lines.map((line) => line.weight));
      return { m, info, lines, half, amplitude, tallest };
    });
    ideals = { multiplets: d.analysis.multiplets, info: d.multipletInfo, shapes };
    return shapes;
  }

  /** The outline of each ideal multiplet in pixels. Kept until the view, the
      size or the height it is drawn to changes: moving the pointer changes none
      of them, and the plot repaints whenever the pointer moves. */
  function outlinesOf(
    d: DocumentInfo,
    ideal: Ideal[],
    scale: number,
    base: number,
    shift: number,
  ): (Float32Array | undefined)[] {
    const kept = outlines;
    if (
      kept &&
      kept.ideal === ideal &&
      kept.x0 === view.x0 &&
      kept.x1 === view.x1 &&
      kept.w === width &&
      kept.scale === scale &&
      kept.base === base &&
      kept.shift === shift
    ) {
      return kept.points;
    }
    const points = ideal.map((shape) => {
      if (!shape.info || scale <= 0 || shape.lines.length === 0) return undefined;
      const [x0, x1] = [shape.m.from, shape.m.to].map((e) => pxOfPpm(view, e - shift, width));
      const [from, to] = [Math.min(x0, x1) - 6, Math.max(x0, x1) + 6];
      const steps = Math.max(2, Math.min(Math.round(to - from), 600));
      const out = new Float32Array((steps + 3) * 2);
      for (let k = 0; k <= steps; k += 1) {
        const at = from + ((to - from) * k) / steps;
        const ppm = ppmAt(view, at, width) + shift;
        let sum = 0;
        for (const line of shape.lines) {
          const away = (ppm - line.ppm) / shape.half;
          sum += line.weight / (1 + away * away);
        }
        out[k * 2] = at;
        out[k * 2 + 1] = base - sum * shape.amplitude * scale;
      }
      out[(steps + 1) * 2] = to;
      out[(steps + 1) * 2 + 1] = base;
      out[(steps + 2) * 2] = from;
      out[(steps + 2) * 2 + 1] = base;
      return out;
    });
    outlines = { ideal, x0: view.x0, x1: view.x1, w: width, scale, base, shift, points };
    return points;
  }

  /** Labels sit in their own row when a stack shows them all, so two spectra can
      be read against each other; otherwise they share the band along the top. */
  function labelBand(position: number, d: DocumentInfo = active): number {
    const perRow = height / rows;
    const band = labelHeight(d);
    // In a stack a spectrum is labelled in its own row, never in a band along the
    // top: there they sat over the other spectra and read as theirs.
    const top = stack !== 'stacked' || perRow <= 0 ? band : rowTop(d, position) + band;
    const moved = movedBy(d, 'labels');
    // Labels go where they are put, but never off the plot.
    return Math.max(band, Math.min(height - LABEL_TICKS, top + moved));
  }

  function paintPeakLabel(
    p: Painter,
    c: ReturnType<typeof colors>,
    peak: PeakLabel,
    band: number,
    ink: string,
  ) {
    const x = Math.round(peak.x) + 0.5;
    const label = Math.round(peak.label) + 0.5;
    // The label and its peak light up together while the pointer is over them.
    if (peak.index === hovered || inMarked(peak.ppm)) ink = c.live;
    // A tick under the label, and a short fan to the peak only when the label
    // had to step aside; a line all the way down would clutter a multiplet.
    p.line([label, band - 2, label, band + LABEL_TICK], ink);
    if (Math.abs(label - x) > 1) {
      p.line([label, band + LABEL_TICK, x, band + LABEL_TICK + 6], ink);
    }
    // The peak itself is marked at its top only while its label is pointed at:
    // a tick on every line was noise on every multiplet.
    if (peak.index === hovered) p.line([x, peak.top - 2, x, peak.top - 2 - PEAK_TICK], ink);
    // What the peak has been called, above its shift: the impurity table suggests
    // one, an empty label says nothing, and anything else was typed in.
    const named = peak.name ?? suggestName(peak.ppm, solvent);
    if (named) {
      p.text(label, band - 6 - PPM_TEXT, named, {
        color: c.muted,
        size: sizes.peaks,
        align: 'left',
        baseline: 'middle',
        vertical: true,
      });
    }
    p.text(label, band - 6, fixed(peak.ppm, decimals), {
      color: ink,
      size: sizes.peaks,
      align: 'left',
      baseline: 'middle',
      vertical: true,
    });
  }

  /** What the pointer shows over the plot: the tool's own icon, drawn, unless
      the pointer is over something to take hold of, where the system's grab or
      resize cursor says which way it moves. */
  const cursorKind = $derived<CursorKind | undefined>(
    grabAxis
      ? undefined
      : phasing
        ? 'phase'
        : pointing
          ? 'points'
          : tool === 'pointer'
            ? undefined
            : tool,
  );

  /** The axis a tool is held to, which the mark at the exact point shows. */
  function cursorAxis(): Axis {
    if (phasing) return 'y';
    if (tool === 'zoom') return zoomAxis;
    if (tool === 'pan') return panAxis;
    return RANGE_TOOLS.includes(tool) ? 'x' : 'xy';
  }

  function drawAxis() {
    const p = painter(axis, AXIS_HEIGHT);
    if (p) paintAxis(p, 0, colors().axis ?? colors().muted);
  }

  function drawOverlay() {
    const p = painter(overlay, height);
    if (!p) return;
    const c = colors();
    if (tool === 'peaks') {
      // Only positive peaks are picked, so only the positive threshold is drawn.
      const y = Math.round(pyOfRow(threshold / traceScale(active), active, activePosition)) + 0.5;
      // Everything under the line is shaded a little darker: nothing there is
      // picked, and the shade says so before a region is dragged over it.
      p.shape([0, y, width, y, width, height, 0, height], '#000000', dim(c.bg));
      guide(p, c, 0, y, width, y);
      pill(p, c, THRESHOLD_X, y, thresholdLabel());
    }
    const marker = referencing ?? preview;
    if (marker !== undefined && PREVIEW_TOOLS.includes(tool)) {
      const x = Math.round(pxOfPpm(view, marker - shiftPpmOf(active), width)) + 0.5;
      guide(p, c, x, 0, x, height);
      pillBeside(p, c, x, `${marker.toFixed(3)} ppm${shiftHeld || !snap ? ' exact' : ''}`);
    }
    if (pointing && handSet) {
      const zeroY = Math.round(pyOfRow(0, active, activePosition)) + 0.5;
      if (pointsPreview) {
        p.line([0, zeroY, width, zeroY], c.muted);
      } else {
        const { curve, start, end } = handSet;
        const line = curve.flatMap((v, k) => {
          const index = start + ((end - start) * k) / (curve.length - 1);
          return [
            pxOfPpm(view, active.firstPpm + index * active.deltaPpm, width),
            pyOfRow(v / traceScale(active), active, activePosition),
          ];
        });
        p.line(line, c.baselineModel);
      }
      // Filled on the spectrum, hollow where placed freely.
      pointing.points.forEach((point, k) => {
        const at = pointPx(k);
        if (!at) return;
        const fill = point.y === undefined ? c.baselineModel : c.bg;
        p.chip(at[0] - 4, at[1] - 4, 8, 8, fill, c.baselineModel, 2);
      });
    }
    if (phasing) {
      // Zero intensity, which the feet of a well phased line come down to.
      const y = Math.round(pyOfRow(0, active, activePosition)) + 0.5;
      p.line([0, y, width, y], c.muted);
      const x = Math.round(pxOfPpm(view, pivotPpm, width)) + 0.5;
      guide(p, c, x, 0, x, height);
      pillBeside(p, c, x, '↔ pivot');
    }
    paintAnalysis(p, c);
    if (cursor && cursorKind)
      paintCursor(p, cursor.x, cursor.y, cursorKind, cursorAxis(), c.fg, c.bg);
    if (cursor && crosshair) {
      const [x, y] = [Math.round(cursor.x) + 0.5, Math.round(cursor.y) + 0.5];
      p.line([x, 0, x, height], c.muted);
      p.line([0, y, width, y], c.muted);
    }
    // Last, so that what the hand is doing lies over everything it is done to.
    if (selection) {
      const s = selection;
      const x0 = selectionAxis === 'y' ? 0 : Math.min(s.x0, s.x1);
      const x1 = selectionAxis === 'y' ? width : Math.max(s.x0, s.x1);
      const y0 = selectionAxis === 'x' ? 0 : Math.min(s.y0, s.y1);
      const y1 = selectionAxis === 'x' ? height : Math.max(s.y0, s.y1);
      p.rect(x0, y0, x1 - x0, y1 - y0, c.muted, 0.12);
    }
  }

  /** Shows `from`..`to` with a margin either side, and fits the height to what is
      then in view: how the multiplet window takes the plot to its multiplet. */
  export function showRange(from: number, to: number) {
    const [hi, lo] = from > to ? [from, to] : [to, from];
    const margin = Math.max((hi - lo) * 0.6, 0.02);
    commit({ ...view, x0: hi + margin, x1: lo - margin });
    // The trace for the new range arrives a moment later; the height is fitted
    // once it has, or it is fitted to the lines of the view just left.
    fitWhenLoaded = true;
  }

  /** The plot as it is drawn now, set on a page of `widthMm` by `heightMm`
   *  with `margin` mm clear round it: the same drawing, sized in millimetres,
   *  so the page on screen and the file are one and the same. */
  export function exportPage(widthMm: number, heightMm: number, margin: number): string {
    const inner = exportSvg();
    const perMm = width / (widthMm - 2 * margin);
    const [w, h] = [widthMm * perMm, heightMm * perMm];
    const nested = inner.replace(
      '<svg xmlns="http://www.w3.org/2000/svg" ',
      `<svg x="${margin * perMm}" y="${margin * perMm}" `,
    );
    return `<svg xmlns="http://www.w3.org/2000/svg" width="${widthMm}mm" height="${heightMm}mm" viewBox="0 0 ${w} ${h}"><rect width="100%" height="100%" fill="${colors().bg}"/>${nested}</svg>\n`;
  }

  /** Waits until every spectrum shown has its trace and integral curves for the
   *  view as it is now, or a few seconds have passed: what an export of several
   *  pages, each shown in turn, has to wait for. */
  export async function settled(limit = 5000): Promise<void> {
    const start = performance.now();
    const done = () =>
      width > 0 &&
      docs.every((d) => loaded[d.id]?.revision === d.revision && !inflight[d.id]) &&
      docs.every(
        (d) =>
          d.analysis.integrationMethod === 'fitted' ||
          d.analysis.integrals.every((range) => curves[curveKey(d, range)]),
      );
    while (!done() && performance.now() - start < limit) {
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    // One frame more, so the overlay is painted with what arrived last.
    await new Promise((resolve) => requestAnimationFrame(resolve));
  }

  /** The current view as SVG: spectra, integrals, peak labels and the ppm axis. */
  export function exportSvg(): string {
    const p = new SvgPainter(colors().font);
    const c = colors();
    docs.forEach((d, k) => {
      const have = loaded[d.id];
      const xy = buffers[d.id];
      if (!have || !xy) return;
      const [s, offset] = [traceScale(d), baseOffset(d, k)];
      const shift = shiftPpmOf(d);
      const points: number[] = [];
      for (let j = 0; j < xy.length; j += 2) {
        const ppm = d.firstPpm + (have.start + xy[j]) * d.deltaPpm - shift;
        if (ppm > view.x0 || ppm < view.x1) continue;
        const y = Math.min(Math.max(pyOf(xy[j + 1] / s + offset), 0), height);
        points.push(pxOfPpm(view, ppm, width), y);
      }
      p.line(points, colorOf(d, k));
    });
    paintAnalysis(p, c);
    p.line([0, height + 0.5, width, height + 0.5], c.axis ?? c.muted);
    paintAxis(p, height, c.axis ?? c.muted);
    return p.toString(width, height + AXIS_HEIGHT, c.bg);
  }

  /** The current view as PNG at the screen's pixel density. */
  export async function exportPng(): Promise<Uint8Array> {
    const canvas = document.createElement('canvas');
    canvas.width = glCanvas.width;
    canvas.height = glCanvas.height + axis.height;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Could not create an image of the view.');
    ctx.fillStyle = colors().bg;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(glCanvas, 0, 0);
    ctx.drawImage(overlay, 0, 0);
    ctx.drawImage(axis, 0, glCanvas.height);
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
    if (!blob) throw new Error('Could not encode the view as PNG.');
    return new Uint8Array(await blob.arrayBuffer());
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const turn = wheelAction(e);
    // Alt scales one spectrum on its own: in a stack, whichever row the pointer
    // is over.
    if (e.altKey && stack !== 'split' && docs.length > 1) {
      const target = stack === 'stacked' ? docs[rowAt(e.offsetY / zoom)] : active;
      onscaletrace(target.id, 1 / turn.factor);
    } else if (turn.kind === 'zoom') {
      view = zoomAt(
        view,
        e.offsetX / zoom,
        e.offsetY / zoom,
        width,
        height,
        turn.factor,
        'x',
        limits,
      );
    } else if (turn.kind === 'pan') {
      view = pan(view, -turn.amount, 0, width, height, limits);
    } else {
      scaleIntensity(turn.factor);
    }
  }

  function onpointerdown(e: PointerEvent) {
    onfocus?.();
    const secondary = e.button === 2 || (isMac && e.ctrlKey);
    if (e.button !== 0 && !(phasing && e.button === 2)) return;
    if (pointing && tool !== 'pan') {
      overlay.setPointerCapture(e.pointerId);
      const [x, y] = [e.offsetX / zoom, e.offsetY / zoom];
      drag = { kind: 'point', index: pointNear(x, y), x, y, start: view, moved: false };
      return;
    }
    // Hydrogens are waiting for a signal: one clicked anywhere in a multiplet's
    // region, its chip included, is the one they go to. Elsewhere the click is
    // an ordinary one, so the view can still be zoomed while they wait.
    if (armed && onassign && tool === 'pointer' && !e.shiftKey && !phasing) {
      const at =
        multipletChip(e.offsetX / zoom, e.offsetY / zoom)?.index ??
        multipletRow(e.offsetX / zoom, e.offsetY / zoom);
      if (at !== undefined) {
        onassign(at);
        return;
      }
    }
    overlay.setPointerCapture(e.pointerId);
    const hit = hitTest(e.offsetX / zoom, e.offsetY / zoom, e.shiftKey);
    if (hit) {
      const held = hit.kind === 'baseline' ? docs[hit.index ?? 0] : undefined;
      // Taking hold of a baseline picks that spectrum, as clicking its trace does.
      if (held) {
        highlighted = true;
        if (held.id !== active.id) onactivate(held.id);
      }
      drag = {
        ...hit,
        x: e.offsetX / zoom,
        y: e.offsetY / zoom,
        start: view,
        adjust: held && { ...adjustOf(held) },
        moved: false,
      };
      dragPpm = ppmAt(view, e.offsetX / zoom, width) + shiftPpmOf(active);
      return;
    }
    // Pointing at a row of a stack picks that spectrum, so an integration or a
    // peak pick lands on the spectrum under the pointer rather than on whichever
    // one happened to be active, and the panel follows what was clicked.
    closeMenu();
    const picks = tool === 'pointer' || RANGE_TOOLS.includes(tool) || tool === 'peakByPeak';
    if (picks && stack === 'stacked' && docs.length > 1 && !adjusting) {
      // Only a click near a trace picks a spectrum out; empty space lets go of
      // the one held, so every spectrum is drawn in its own colour.
      // Near the trace itself, not merely somewhere in its row: the row covers
      // the whole height between baselines, so that let blank space select.
      const row = rowAt(e.offsetY / zoom);
      const at = traceYAt(docs[row], row, e.offsetX / zoom);
      const onTrace = at !== undefined && Math.abs(e.offsetY / zoom - at) <= TRACE_GRAB_PX;
      if (onTrace) {
        highlighted = true;
        const picked = docs[row];
        if (picked && picked.id !== active.id) onactivate(picked.id);
      } else if (tool === 'pointer') {
        highlighted = false;
      }
    }
    // Only while adjusting items.
    if (adjusting && tool === 'pointer' && stack !== 'split' && docs.length > 1) {
      drag = {
        kind: 'trace',
        x: e.offsetX / zoom,
        y: e.offsetY / zoom,
        start: view,
        adjust: { ...adjustOf(active) },
        moved: false,
      };
      return;
    }
    let kind: 'view' | 'p0' | 'p1' | 'pivot' = 'view';
    if (phasing && tool !== 'pan') {
      const nearPivot =
        Math.abs(e.offsetX / zoom - pxOfPpm(view, pivotPpm, width)) <= PIVOT_GRAB_PX;
      kind = nearPivot ? 'pivot' : secondary ? 'p1' : 'p0';
    }
    const phase = phasing && $state.snapshot(phasing);
    drag = { kind, x: e.offsetX / zoom, y: e.offsetY / zoom, start: view, phase, moved: false };
  }

  /** The line a click would take, which is what the preview has to show: the
      nearest one, or the pointer itself with Shift held. Showing the nearest
      while taking the pointer is what made picking with Shift confusing. */
  function previewAt(x: number, exactly: boolean): number | undefined {
    if (!PREVIEW_TOOLS.includes(tool) || referencing !== undefined) return undefined;
    const exact = ppmAt(view, x, width) + shiftPpmOf(active);
    return exactly || !snap ? exact : (nearestTop(x) ?? exact);
  }

  function onpointermove(e: PointerEvent) {
    cursor = { x: e.offsetX / zoom, y: e.offsetY / zoom };
    if (!drag) {
      const hit = hitTest(e.offsetX / zoom, e.offsetY / zoom, e.shiftKey);
      grabAxis = axisOfGrab(hit, e.shiftKey);
      hovered = hit?.kind === 'peak' ? hit.index : undefined;
      hoveredIntegral = integralRow(e.offsetX / zoom, e.offsetY / zoom);
      hoveredMultiplet =
        hit?.kind === 'multiplet' || hit?.kind === 'multipletEnd'
          ? hit.index
          : multipletRow(e.offsetX / zoom, e.offsetY / zoom);
      // The line under the pointer is shown before it is taken, as it is taken.
      preview = previewAt(e.offsetX / zoom, e.shiftKey || shiftHeld);
      return;
    }
    if (MOVED.includes(drag.kind as Moved)) {
      // A chip is clicked to open what it stands for, so moving its band asks for
      // Shift as the other annotations do: without it the drag does nothing and
      // the release still opens the window, which forgives a slipped hand.
      if (drag.kind === 'multiplet' && !e.shiftKey) return;
      // Up and down only: where an annotation sits along the axis belongs to the
      // signal it is about, not to the pointer.
      dragDy = e.offsetY / zoom - drag.y;
      drag.moved = true;
      return;
    }
    if (drag.kind === 'peak') {
      // A label moves up and down only: where it sits along the axis is its
      // peak's business, not the pointer's.
      dragDy = e.offsetY / zoom - drag.y;
      drag.moved = true;
      return;
    }
    if (drag.kind === 'integral' || drag.kind === 'multipletEnd') {
      dragPpm = ppmAt(view, e.offsetX / zoom, width) + shiftPpmOf(active);
      drag.moved = true;
      return;
    }
    if (drag.kind === 'point') {
      const [x, y] = [e.offsetX / zoom, e.offsetY / zoom];
      drag.moved ||= Math.hypot(x - drag.x, y - drag.y) > DRAG_THRESHOLD;
      const k = drag.index;
      const held = k === undefined ? undefined : pointing?.points[k];
      if (!pointing || !held || !drag.moved) return;
      // A point placed freely goes where it is put; one on the spectrum only
      // moves along it.
      const y0 = held.y === undefined || !handSet?.top ? undefined : valueAt(y) / handSet.top;
      const moved = { at: pointAt(x), ...(y0 === undefined ? {} : { y: y0 }) };
      setPoints({ points: pointing.points.map((p, i) => (i === k ? moved : p)) });
      return;
    }
    if (drag.kind === 'threshold') {
      const level =
        intensityAt(view, e.offsetY / zoom, height) - baseOffset(active, activePosition);
      onthreshold(Math.max(0.5, (level * traceScale(active)) / (active.noise || 1)));
      drag.moved = true;
      return;
    }
    if (drag.kind === 'baseline' && drag.index !== undefined && drag.adjust) {
      const held = docs[drag.index];
      const moved = e.offsetY / zoom - drag.y;
      // A click that wavers a pixel only picks the spectrum.
      if (!drag.moved && Math.abs(moved) < DRAG_THRESHOLD) return;
      // A row is measured in rows; a single spectrum in the view's own units.
      const units =
        stack === 'stacked' ? (-moved / height) * rows : (-moved / height) * (view.y1 - view.y0);
      onmovetrace(held.id, drag.adjust.dx, drag.adjust.dy + units);
      drag.moved = true;
      return;
    }
    if (drag.kind === 'trace' && drag.adjust) {
      const ppm = ppmAt(view, e.offsetX / zoom, width) - ppmAt(view, drag.x, width);
      const level = intensityAt(view, e.offsetY / zoom, height) - intensityAt(view, drag.y, height);
      onmovetrace(active.id, drag.adjust.dx + ppm, drag.adjust.dy + level);
      drag.moved = true;
      return;
    }
    const dx = e.offsetX / zoom - drag.x;
    const dy = e.offsetY / zoom - drag.y;
    drag.moved ||= Math.hypot(dx, dy) > DRAG_THRESHOLD;
    if (drag.kind !== 'view' && drag.phase) {
      // Ctrl (Cmd on Mac, where Ctrl means right button) for fine steps.
      const fine = (isMac ? e.metaKey : e.ctrlKey) ? FINE_DRAG : 1;
      if (drag.kind === 'p0') setPhase({ p0: drag.phase.p0 - dy * P0_PER_PX * fine });
      else if (drag.kind === 'p1') setPhase({ p1: drag.phase.p1 - dy * P1_PER_PX * fine });
      else movePivot(indexOf(active, ppmAt(view, e.offsetX / zoom, width)) / active.points);
    } else if (tool === 'pan') {
      const [px, py] = [panAxis === 'y' ? 0 : dx, panAxis === 'x' ? 0 : dy];
      view = pan(drag.start, px, py, width, height, limits);
    } else if (RANGE_TOOLS.includes(tool) && drag.moved) {
      selection = { x0: drag.x, y0: drag.y, x1: e.offsetX / zoom, y1: e.offsetY / zoom };
    }
  }

  function onpointerup(e: PointerEvent) {
    if (!drag) return;
    const [px, py] = [e.offsetX / zoom, e.offsetY / zoom];
    const range: Range | undefined = selection && [
      ppmAt(view, selection.x0, width),
      ppmAt(view, selection.x1, width),
    ];
    if (drag.kind === 'point') {
      // A second click on a point just put down, before its height is in, is
      // the same point: a double-click is meant to remove one, not add two.
      const near = pointing?.points.some(
        (p) => Math.abs(pxOfPpm(view, pointPpm(p.at), width) - px) <= POINT_GRAB_PX,
      );
      if (pointing && !drag.moved && drag.index === undefined && !near) {
        const free = freePoints && handSet?.top ? { y: valueAt(py) / handSet.top } : {};
        setPoints({ points: [...pointing.points, { at: pointAt(px), ...free }] });
      }
    } else if (drag.kind === 'multiplet' && !drag.moved) {
      // A chip is clicked to open its window; the rule is dragged to move it.
      if (drag.index !== undefined) onopenmultiplet(drag.index);
    } else if (MOVED.includes(drag.kind as Moved)) {
      const what = drag.kind as Moved;
      if (drag.moved && dragDy !== undefined) {
        onshift(active.id, what, (shifts[active.id]?.[what] ?? 0) + dragDy);
      }
    } else if (drag.kind === 'multipletEnd' && drag.index !== undefined && dragPpm !== undefined) {
      const range = active.analysis.multiplets[drag.index];
      if (drag.moved && range) {
        onmove(
          'multiplet',
          drag.index,
          drag.edge === 0 ? dragPpm : range.from,
          drag.edge === 1 ? dragPpm : range.to,
        );
      }
    } else if (drag.kind === 'integral' && drag.index !== undefined && dragPpm !== undefined) {
      const range = active.analysis.integrals[drag.index];
      if (drag.moved && range) {
        onmove(
          'integral',
          drag.index,
          drag.edge === 0 ? dragPpm : range[0],
          drag.edge === 1 ? dragPpm : range[1],
        );
      }
    } else if (drag.kind !== 'view') {
      // Phase drags change only the phase, not the view.
    } else if (tool === 'pan' && drag.moved) {
      commit(view);
    } else if (tool === 'zoom') {
      commit(
        selection
          ? zoomBox(view, selection, width, height, zoomAxis, limits)
          : zoomAt(view, px, py, width, height, LENS, zoomAxis, limits),
      );
    } else if (tool === 'zoomOut') {
      commit(zoomAt(view, px, py, width, height, 1 / LENS, zoomAxis, limits));
    } else if ((tool === 'integrate' || tool === 'multiplet' || tool === 'peaks') && range) {
      onrange(tool, ...range);
    } else if (tool === 'reference' && !drag.moved) {
      // The peak that is clicked is the one whose shift is about to be named.
      // Shift names the shift under the pointer rather than the nearest line,
      // which is what it means everywhere else it is offered.
      const at = ppmAt(view, px, width) + shiftPpmOf(active);
      referencing = e.shiftKey || !snap ? at : (nearestTop(px) ?? at);
      preview = undefined;
    } else if (tool === 'pickValue' && !drag.moved) {
      // The top of the line clicked, which is the shift being asked for.
      // Shift takes the pointer where it is, as it does picking peak by peak.
      const exact = ppmAt(view, px, width) + shiftPpmOf(active);
      onpickvalue(e.shiftKey || !snap ? exact : (nearestTop(px) ?? exact));
    } else if (tool === 'peakByPeak' && !drag.moved) {
      const pointsPerPx = Math.abs((view.x0 - view.x1) / width / active.deltaPpm);
      const radius = Math.max(1, Math.round(PEAK_CLICK_PX * pointsPerPx));
      onpickpoint(ppmAt(view, px, width), e.shiftKey || !snap, radius);
    }
    drag = undefined;
    dragPpm = undefined;
    dragDy = undefined;
    selection = undefined;
  }
</script>

<div class="plot" class:unfocused={!focused}>
  {#if zoomTo}
    <dialog
      class="window zoom"
      bind:this={zoomDialog}
      onclose={() => (zoomTo = undefined)}
      aria-label="Manual zoom"
    >
      <header>
        <h2>Set range</h2>
        <button
          class="icon"
          tabindex="-1"
          title="Close (Esc)"
          aria-label="Close"
          onclick={(e) => e.currentTarget.closest('dialog')?.close()}><X size={16} /></button
        >
      </header>
      <div class="window-body fields">
        <label>
          From
          <input type="number" step="0.01" bind:value={zoomTo.from} />
          <span class="suffix">ppm</span>
        </label>
        <label>
          to
          <input type="number" step="0.01" bind:value={zoomTo.to} />
          <span class="suffix">ppm</span>
        </label>
      </div>
      <footer>
        <button title="Leave the view as it is (Esc)" onclick={() => zoomDialog?.close()}
          >Cancel</button
        >
        <button class="primary" title="Zoom to this range (Enter)" onclick={applyZoomTo}
          >Show</button
        >
      </footer>
    </dialog>
  {/if}
  {#if referencing !== undefined}
    <dialog
      class="window solvents"
      bind:this={solventDialog}
      onclose={() => (referencing = undefined)}
      aria-label="Reference the axis"
    >
      <header>
        <h2>Reference the axis</h2>
        <button
          class="icon"
          tabindex="-1"
          title="Close (Esc)"
          aria-label="Close"
          onclick={(e) => e.currentTarget.closest('dialog')?.close()}><X size={16} /></button
        >
      </header>
      <div class="window-body">
        <!-- Any value at all, for a reference that is not a solvent: an internal
             standard, a known signal of the sample, a value from a paper. -->
        <form
          class="manual"
          onsubmit={(e) => {
            e.preventDefault();
            const shift = Number(manualShift.replace(',', '.'));
            if (Number.isFinite(shift)) applyReference(shift);
          }}
        >
          <label for="reference-shift"
            >Line at <strong>{referencing.toFixed(3)}</strong> ppm becomes</label
          >
          <span class="control">
            <!-- svelte-ignore a11y_autofocus -->
            <input
              id="reference-shift"
              inputmode="decimal"
              autofocus
              bind:value={manualShift}
              onfocus={(e) => e.currentTarget.select()}
            />
            <span class="unit">ppm</span>
          </span>
          <button class="primary" type="submit" title="Set the axis (Enter)">Apply</button>
        </form>
        <div class="list" role="group" aria-label="Solvent residual peaks">
          <h3 class="section-label">Or a solvent's residual peak</h3>
          {#each solventOrder as solvent (solvent.formula)}
            <button
              class="row"
              class:suggested={solvent.shift === suggestedShift}
              title="Call this line {solvent.shift.toFixed(2)} ppm"
              onclick={() => applyReference(solvent.shift)}
            >
              <span class="name">{solvent.name}</span>
              <span class="formula">{formatSolvent(solvent.formula)}</span>
              {#if solvent.shift === suggestedShift}<span class="tag">This sample</span>{/if}
              <span class="ppm">{solvent.shift.toFixed(2)}</span>
            </button>
          {/each}
        </div>
      </div>
    </dialog>
  {/if}
  {#if phasing}
    <div class="tool-bar" role="group" aria-label="Phase correction">
      <strong>Phase correction</strong>
      <span class="hint">
        Drag up or down for PH0, right-drag{isMac ? ' (or Ctrl+drag)' : ''} for PH1
      </span>
      <label
        title="Zero-order phase at the pivot, in degrees. Drag up or down on the spectrum; hold Ctrl (Cmd on Mac) for fine steps"
      >
        PH0
        <input
          type="number"
          step="0.1"
          value={phasing.p0.toFixed(1)}
          onchange={(e) => setPhase({ p0: Number(e.currentTarget.value) })}
        />
      </label>
      <label
        title="First-order phase across the spectrum, in degrees. Right-drag on the spectrum (Ctrl+drag on Mac)"
      >
        PH1
        <input
          type="number"
          step="0.1"
          value={phasing.p1.toFixed(1)}
          onchange={(e) => setPhase({ p1: Number(e.currentTarget.value) })}
        />
      </label>
      <span title="PH1 does not change the phase here. Drag the dashed marker to move it"
        >Pivot {pivotPpm.toFixed(3)} ppm</span
      >
      <button title="Put the pivot on the largest peak" onclick={biggestPivot}>Biggest</button>
      <button title="Add 180° to PH0" onclick={() => setPhase({ p0: (phasing?.p0 ?? 0) + 180 })}
        >+180°</button
      >
      <button title="Automatic phase correction" onclick={autoPhasing}>Auto</button>
      <span class="spacer"></span>
      <button class="primary" title="Apply the phase correction (Enter)" onclick={applyPhase}
        >Apply</button
      >
      <button title="Discard these phase changes (Esc)" onclick={() => (phasing = undefined)}
        >Cancel</button
      >
    </div>
  {/if}
  {#if pointing}
    <div class="tool-bar" role="group" aria-label="Baseline through points">
      <strong>Baseline points</strong>
      <span class="hint">Click to add, drag to move, double-click to remove</span>
      <label title="How the baseline joins the points">
        Function
        <select
          value={pointing.through}
          onchange={(e) =>
            setPoints({ through: e.currentTarget.value as PointsOperation['through'] })}
        >
          {#each Object.entries(THROUGH_NAMES) as [value, name] (value)}
            <option {value}>{name}</option>
          {/each}
        </select>
      </label>
      {#if pointing.through === 'polynomial'}
        <label title="From 1, a straight line, to 20; never more than the points allow">
          Order
          <input
            type="number"
            min="1"
            max="20"
            value={pointing.order}
            onchange={(e) =>
              setPoints({
                order: Math.min(20, Math.max(1, Math.round(Number(e.currentTarget.value)))),
              })}
          />
        </label>
      {:else if pointing.through === 'whittaker'}
        <label title="Higher is stiffer: the baseline passes further from points that disagree">
          Smoothness
          <input
            type="range"
            min="1"
            max="11"
            step="0.5"
            value={Math.log10(pointing.lambda)}
            onchange={(e) => setPoints({ lambda: 10 ** Number(e.currentTarget.value) })}
          />
        </label>
      {/if}
      <label
        title="Each point stands at the mean of this many points either side, so noise does not move it; 0 takes the spectrum where it is clicked"
      >
        Span
        <input
          type="number"
          min="0"
          max="500"
          value={pointing.span}
          onchange={(e) =>
            setPoints({ span: Math.max(0, Math.round(Number(e.currentTarget.value))) })}
        />
        points
      </label>
      <button
        aria-pressed={freePoints}
        title="New points stay at the height they are put, off the spectrum"
        onclick={() => (freePoints = !freePoints)}>Free points</button
      >
      <button title="Points through the stretches free of signal" onclick={automaticPoints}
        >Automatic</button
      >
      <button title="A point at each end of the spectrum" onclick={borderPoints}>Ends</button>
      <button title="Take every point away" onclick={() => setPoints({ points: [] })}>Clear</button>
      <button
        aria-pressed={pointsPreview}
        title="Show the spectrum with this baseline taken off"
        onclick={() => (pointsPreview = !pointsPreview)}>Preview</button
      >
      <span class="spacer"></span>
      <button class="primary" title="Take this baseline off (Enter)" onclick={applyPoints}
        >Apply</button
      >
      <button title="Leave the baseline as it was (Esc)" onclick={() => (pointing = undefined)}
        >Cancel</button
      >
    </div>
  {/if}
  <div
    class="area"
    bind:this={area}
    data-tool={tool}
    data-cursor={cursorKind ? 'drawn' : undefined}
    data-axis={selectionAxis}
    data-phasing={!!phasing}
    data-pointing={!!pointing}
    data-grab={grabAxis}
  >
    <canvas bind:this={glCanvas}></canvas>
    <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
    <canvas
      class="overlay"
      bind:this={overlay}
      tabindex="0"
      role="application"
      aria-label="Spectrum plot. Left and right arrows step through peaks, up and down change to integrals or multiplets, Delete removes the one reached"
      onkeydown={onplotkey}
      {onpointerdown}
      {onpointermove}
      {onpointerup}
      {ondblclick}
      onpointerleave={() => (
        (cursor = undefined),
        (hovered = undefined),
        (hoveredIntegral = undefined),
        (hoveredMultiplet = undefined)
      )}
      oncontextmenu={(e) => {
        const hit = hitTest(e.offsetX / zoom, e.offsetY / zoom);
        const peak =
          hit?.kind === 'peak'
            ? peakLabels().find((entry) => entry.index === hit.index)
            : undefined;
        const chip = multipletChip(e.offsetX / zoom, e.offsetY / zoom);
        const row = integralRow(e.offsetX / zoom, e.offsetY / zoom);
        const shift = shiftPpmOf(active);
        if (peak) {
          menu = peakMenu(peak);
        } else if (chip?.index !== undefined) {
          const box = chips.find((c) => c.index === chip.index);
          menu = {
            kind: 'multiplet',
            index: chip.index,
            x: box ? box.x + box.w / 2 : e.offsetX / zoom,
            y: box ? box.y + box.h + 6 : e.offsetY / zoom,
            label: '',
            from: '',
          };
        } else if (row !== undefined) {
          const range = active.analysis.integrals[row];
          const ends = [range[0], range[1]].map((edge) => pxOfPpm(view, edge - shift, width));
          menu = {
            kind: 'integral',
            index: row,
            x: (ends[0] + ends[1]) / 2,
            y: bracketY(active, activePosition) + 14,
            label: '',
            from: '',
          };
        } else {
          // Nothing of ours under the pointer, so the system's own menu stands
          // rather than being swallowed for no one.
          return;
        }
        e.preventDefault();
      }}
    ></canvas>
    <div class="sr-only" aria-live="polite">{announced}</div>
    <!-- A stack already names every row beside its own trace. -->
    {#if docs.length > 1 && stack !== 'stacked'}
      <ul class="legend">
        {#each [...docs].reverse() as d, i (d.id)}
          {@const color = colorOf(d, docs.length - 1 - i)}
          <li class:active={d.id === active.id} style:color>
            <span class="swatch" style:background={color}></span>{nameOf(d)}
          </li>
        {/each}
      </ul>
    {/if}
    {#if menu}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="menu-dim" onclick={() => closeMenu()}></div>
      <div
        class="menu-box"
        style:left="{menu.x * zoom}px"
        style:top="{menu.y * zoom}px"
        role="menu"
      >
        {#if menu.kind === 'peak'}
          <input
            class="name"
            title="What to call this peak; empty shows nothing"
            aria-label="Label for this peak"
            value={menu.label}
            oninput={(e) => {
              // Kept as it is typed, so however the menu is closed the name is
              // there to be kept rather than waiting on a change that may not come.
              if (menu) menu.label = e.currentTarget.value;
            }}
            onkeydown={(e) => {
              if (e.key === 'Enter') closeMenu();
              else if (e.key === 'Escape') closeMenu(false);
            }}
          />
        {/if}
        <button
          onclick={() => {
            if (menu) onremove(menu.kind, menu.index);
            menu = undefined;
          }}>Remove {menu.kind}</button
        >
        <button onclick={() => closeMenu(false)}>Cancel</button>
      </div>
    {/if}
  </div>
  <!-- The row is as tall as its axis, which a page letters larger and titles;
       at a fixed height the title fell off the foot of the page. -->
  <div class="axis-row" style:height="{AXIS_HEIGHT * zoom}px" style:border-top-color={ink?.axis}>
    <canvas class="axis" bind:this={axis} style:height="{AXIS_HEIGHT * zoom}px"></canvas>
  </div>
</div>

<style>
  .plot {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  /* In split panes, the one that has the keyboard keeps full contrast. */
  .plot.unfocused .area {
    opacity: 0.72;
  }

  .tool-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }

  .tool-bar {
    gap: 8px;
    background: var(--panel);
  }

  .tool-bar button,
  .tool-bar input {
    padding: 2px 8px;
  }

  .tool-bar label {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .spacer {
    flex: 1;
  }

  .area {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .area canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .overlay {
    color: var(--muted);
    touch-action: none;
  }

  /* A chosen tool's pointer is drawn on the overlay (`paintCursor`): the
     system's is hidden, since WebKit reloads an image cursor whenever the plot
     repaints, which is on every move. */
  .area[data-cursor='drawn'] {
    cursor: none;
  }

  /* Last and equally specific, so grabbing wins over the tool's own pointer. The
     cursor says which way the thing under it moves: an integral's end runs along
     the axis, everything else up and down. */
  .area[data-grab='y'][data-tool] {
    cursor: ns-resize;
  }

  .area[data-grab='x'][data-tool] {
    cursor: ew-resize;
  }

  .area[data-grab='click'][data-tool] {
    cursor: pointer;
  }

  /* The rest steps back while the menu is open, as the dialogs do. */
  .menu-dim {
    position: absolute;
    inset: 0;
    z-index: 1;
    background: rgb(0 0 0 / 35%);
  }

  .menu-box {
    position: absolute;
    z-index: 2;
    /* Centred on its peak, and never off the edge of the plot. */
    transform: translateX(-50%);
    max-width: 90%;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-overlay);
    background: var(--panel);
  }

  .menu-box .name {
    width: 9rem;
  }

  .solvents .manual {
    display: flex;
    gap: var(--space-4);
    align-items: center;
  }

  .solvents .manual label {
    flex: 1;
    color: var(--muted);
  }

  .solvents .manual strong {
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }

  .solvents .control {
    position: relative;
    display: flex;
    align-items: center;
  }

  .solvents .control input {
    width: 8rem;
    padding-right: 3rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .solvents .unit {
    position: absolute;
    right: var(--space-4);
    color: var(--muted);
    font-size: var(--text-label);
    pointer-events: none;
  }

  /* A list of rows, not a stack of cards: no gap between them. */
  .solvents .list {
    display: flex;
    flex-direction: column;
  }

  .solvents .list h3 {
    margin-bottom: var(--space-3);
  }

  .solvents .row {
    display: grid;
    grid-template-columns: 1fr auto auto 3rem;
    gap: var(--space-4);
    align-items: baseline;
    min-height: 0;
    padding: var(--space-3) var(--space-4);
    border-color: transparent;
    background: none;
    text-align: left;
  }

  .solvents .row:hover {
    background: var(--hover);
  }

  .solvents .row.suggested {
    background: var(--accent-soft);
  }

  .solvents .name {
    grid-column: 1;
  }

  .solvents .formula {
    grid-column: 2;
    color: var(--muted);
  }

  .solvents .tag {
    grid-column: 3;
    color: var(--accent);
    font-size: var(--text-label);
    font-weight: 600;
  }

  .solvents .ppm {
    grid-column: 4;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .zoom .fields {
    flex-direction: row;
    gap: 10px;
    align-items: center;
  }

  .zoom label {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .zoom .suffix {
    color: var(--muted);
  }

  .legend {
    position: absolute;
    top: 8px;
    right: 12px;
    margin: 0;
    padding: 6px 8px;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    pointer-events: none;
  }

  .legend li {
    display: flex;
    gap: 6px;
    align-items: center;
    opacity: 0.75;
  }

  .legend li.active {
    opacity: 1;
    font-weight: 600;
  }

  .swatch {
    width: 14px;
    height: 3px;
    border-radius: 2px;
  }

  .axis-row {
    position: relative;
    height: 28px;
    border-top: 1px solid var(--border);
  }

  .axis {
    display: block;
    width: 100%;
    height: 28px;
    color: var(--muted);
  }
</style>
