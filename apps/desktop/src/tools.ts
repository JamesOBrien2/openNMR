import type { Axis } from './view';

/** The plot tools, shared by the tool bar and every pane below it. */
export type Tool =
  | 'pointer'
  | 'zoom'
  | 'zoomOut'
  | 'pan'
  | 'integrate'
  | 'multiplet'
  | 'peaks'
  | 'peakByPeak'
  | 'pickValue'
  | 'reference';

/** Pressing Z or P again cycles through these. */
export const ZOOM_AXES: Axis[] = ['x', 'y', 'xy'];
export const PAN_AXES: Axis[] = ['xy', 'x', 'y'];
export const AXIS_LABEL: Record<Axis, string> = { x: '↔', y: '↕', xy: '⤢' };

/** Tools whose dragged range is handed on: integrated, read as a multiplet, picked for peaks. */
export type RangeTool = 'integrate' | 'multiplet' | 'peaks';

/** Tools that act on a dragged range rather than a click. */
export const RANGE_TOOLS: Tool[] = ['zoom', 'integrate', 'multiplet', 'peaks'];

export const nextAxis = (axes: Axis[], current: Axis) =>
  axes[(axes.indexOf(current) + 1) % axes.length];

/** Z: the zoom tool along the axis, or, while it is chosen, its next direction. */
export const pressZoom = (mode: Tool, axis: Axis): [Tool, Axis] =>
  mode === 'zoom' ? ['zoom', nextAxis(ZOOM_AXES, axis)] : ['zoom', 'x'];

/** P: the pan tool both ways, or, while it is chosen, its next direction. */
export const pressPan = (mode: Tool, axis: Axis): [Tool, Axis] =>
  mode === 'pan' ? ['pan', nextAxis(PAN_AXES, axis)] : ['pan', 'xy'];

const WHEEL_SPEED = 0.002;
const FINE_WHEEL = 0.1;

/** What a turn of the wheel asks for, as every plot reads it: Cmd zooms about
 * the pointer, Cmd with Shift finely; Shift, or a turn along the other axis,
 * runs along the axis; the wheel alone is the plot's own (a spectrum's
 * intensity). Ctrl was the obvious key for zoom and macOS keeps it for its own,
 * so it never arrived. `factor` scales for a zoom or the wheel alone, `amount`
 * is pixels for a pan. */
export function wheelAction(e: Pick<WheelEvent, 'deltaX' | 'deltaY' | 'shiftKey' | 'metaKey'>): {
  kind: 'zoom' | 'pan' | 'plain';
  amount: number;
  factor: number;
} {
  const speed = WHEEL_SPEED * (e.metaKey && e.shiftKey ? FINE_WHEEL : 1);
  // A mouse sends Shift as a turn along the other axis, so the turn is taken
  // from whichever axis it arrived on.
  const along = Math.abs(e.deltaX) > Math.abs(e.deltaY);
  const amount = along ? e.deltaX : e.deltaY;
  const kind = e.metaKey ? 'zoom' : e.shiftKey || along ? 'pan' : 'plain';
  return { kind, amount, factor: Math.exp(amount * speed) };
}

/** What the keys every plot shares ask of its view. */
export type ViewAction =
  | 'zoom'
  | 'zoomOut'
  | 'pan'
  | 'full'
  | 'fit'
  | 'larger'
  | 'smaller'
  | 'back'
  | 'forward'
  | 'range'
  | 'crosshair'
  | 'pointer';

/** The view keys: Z zoom and Shift+Z zoom out, P pan, F full, H fit to
 * highest, + and - intensity, M type the range, C crosshair, Shift with ← or →
 * the zoom history, Esc the pointer.
 * Undefined for any other key, or one held with Cmd, Ctrl or Alt, which a plot
 * of its own may still answer. */
export function viewAction(
  e: Pick<KeyboardEvent, 'key' | 'shiftKey' | 'metaKey' | 'ctrlKey' | 'altKey'>,
): ViewAction | undefined {
  if (e.metaKey || e.ctrlKey || e.altKey) return undefined;
  if (e.shiftKey && e.key === 'ArrowLeft') return 'back';
  if (e.shiftKey && e.key === 'ArrowRight') return 'forward';
  switch (e.key.toLowerCase()) {
    case 'z':
      return e.shiftKey ? 'zoomOut' : 'zoom';
    // Shift+P is manual phasing, which only a spectrum has.
    case 'p':
      return e.shiftKey ? undefined : 'pan';
    case 'f':
      return 'full';
    case 'h':
      return 'fit';
    case '+':
    case '=':
      return 'larger';
    case '-':
      return 'smaller';
    case 'm':
      return 'range';
    case 'c':
      return 'crosshair';
    case 'escape':
      return 'pointer';
  }
  return undefined;
}

/**
 * What to do with each tool, said in the bar under the ribbon: a tool that
 * changes nothing on screen until it is used is otherwise a guess. `undefined`
 * where something else already says it (picking a value has its own bar).
 */
export function toolHint(tool: Tool, snap: boolean): string | undefined {
  switch (tool) {
    case 'pointer':
      return undefined;
    case 'zoom':
      return 'Drag over the part to enlarge · Z again changes direction';
    case 'zoomOut':
      return 'Click to zoom out';
    case 'pan':
      return 'Drag to move the view · P again changes direction';
    case 'integrate':
      return 'Drag across a signal to integrate it';
    case 'multiplet':
      return 'Drag across all the lines of one signal';
    case 'peaks':
      return 'Drag over peaks to pick them · move the pill to set the threshold';
    case 'peakByPeak':
      return snap ? 'Click a peak to pick its top' : 'Click to pick a peak where you point';
    case 'reference':
      return 'Click the solvent line to set the axis by it';
    case 'pickValue':
      return undefined;
  }
}
