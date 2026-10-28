import { mount, unmount, type Component } from 'svelte';
import {
  Brackets,
  Hand,
  MousePointerClick,
  Pipette,
  ScanLine,
  Sigma,
  SlidersHorizontal,
  Target,
  Waypoints,
  ZoomIn,
  ZoomOut,
} from '@lucide/svelte';
import type { Painter } from './painter';
import type { Tool } from './tools';
import type { Axis } from './view';

/** What the pointer can be over the plot besides the plain arrow: a tool, or the
 *  manual phase and the baseline points, which take the plot over too. */
export type CursorKind = Exclude<Tool, 'pointer'> | 'phase' | 'points';

/** The icon each one shows: the one on its button, so the two are recognised
 *  as the same thing. */
const ICONS: Record<CursorKind, Component> = {
  zoom: ZoomIn,
  zoomOut: ZoomOut,
  pan: Hand,
  integrate: Sigma,
  multiplet: Brackets,
  peaks: ScanLine,
  peakByPeak: MousePointerClick,
  pickValue: Pipette,
  reference: Target,
  phase: SlidersHorizontal,
  points: Waypoints,
};

const n = (value: string | null) => Number(value ?? 0);

/** One element of a Lucide icon as SVG path data, in the icon's own units. */
export function elementPath(tag: string, attr: (name: string) => string | null): string {
  switch (tag) {
    case 'path':
      return attr('d') ?? '';
    case 'line':
      return `M${n(attr('x1'))} ${n(attr('y1'))}L${n(attr('x2'))} ${n(attr('y2'))}`;
    case 'circle':
    case 'ellipse': {
      const [cx, cy] = [n(attr('cx')), n(attr('cy'))];
      const rx = n(attr(tag === 'circle' ? 'r' : 'rx'));
      const ry = tag === 'circle' ? rx : n(attr('ry'));
      return `M${cx - rx} ${cy}a${rx} ${ry} 0 1 0 ${2 * rx} 0a${rx} ${ry} 0 1 0 ${-2 * rx} 0`;
    }
    case 'polyline':
    case 'polygon': {
      const xy = (attr('points') ?? '')
        .trim()
        .split(/[\s,]+/)
        .map(Number);
      const pairs = [];
      for (let k = 0; k + 1 < xy.length; k += 2) pairs.push(`${xy[k]} ${xy[k + 1]}`);
      return `M${pairs.join('L')}${tag === 'polygon' ? 'Z' : ''}`;
    }
    case 'rect': {
      const [x, y, w, h] = ['x', 'y', 'width', 'height'].map((k) => n(attr(k)));
      const r = Math.min(n(attr('rx') ?? attr('ry')), w / 2, h / 2);
      if (r === 0) return `M${x} ${y}h${w}v${h}h${-w}Z`;
      return (
        `M${x + r} ${y}h${w - 2 * r}a${r} ${r} 0 0 1 ${r} ${r}v${h - 2 * r}` +
        `a${r} ${r} 0 0 1 ${-r} ${r}h${-(w - 2 * r)}a${r} ${r} 0 0 1 ${-r} ${-r}` +
        `v${-(h - 2 * r)}a${r} ${r} 0 0 1 ${r} ${-r}Z`
      );
    }
    default:
      return '';
  }
}

const paths = new Map<CursorKind, string[]>();

/** The icon's shapes, read once from the component the buttons use: the same
 *  drawing whatever version of the icons is installed. */
export function iconPaths(kind: CursorKind): string[] {
  const known = paths.get(kind);
  if (known) return known;
  const host = document.createElement('div');
  const app = mount(ICONS[kind], { target: host });
  const svg = host.querySelector('svg');
  const shapes = svg
    ? [...svg.querySelectorAll('path, line, circle, ellipse, polyline, polygon, rect')]
        .map((el) => elementPath(el.tagName.toLowerCase(), (name) => el.getAttribute(name)))
        .filter((d) => d !== '')
    : [];
  void unmount(app);
  paths.set(kind, shapes);
  return shapes;
}

/** Where the icon sits from the point the pointer is at, and how big it is. */
const ICON_OFFSET = 9;
const ICON_SIZE = 16;

/** The mark at the exact point, in the same 24 units and stroke as the icons,
 *  centred on 12, 12: a cross, or a double arrow along the axis a tool is held to. */
const MARKS: Record<Axis, string[]> = {
  xy: ['M4 12H20', 'M12 4V20'],
  x: ['M3 12H21', 'M6 9L3 12L6 15', 'M18 9L21 12L18 15'],
  y: ['M12 3V21', 'M9 6L12 3L15 6', 'M9 18L12 21L15 18'],
};

/**
 * The pointer over the plot while a tool is chosen, drawn on the overlay: the
 * system cursor is hidden there (`cursor: none`), because WebKit reloads an
 * image cursor whenever what is under it repaints and the plot repaints on every
 * move. A mark at the exact point a click takes, and the tool's icon beside it.
 */
export function paintCursor(
  p: Painter,
  x: number,
  y: number,
  kind: CursorKind,
  axis: Axis,
  ink: string,
  halo: string,
) {
  p.icon(MARKS[axis], x - ICON_SIZE / 2, y - ICON_SIZE / 2, ICON_SIZE, ink, halo);
  p.icon(iconPaths(kind), x + ICON_OFFSET, y + ICON_OFFSET, ICON_SIZE, ink, halo);
}
