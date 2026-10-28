import type { Molecule } from './api';
import { layout } from './molecule';

/** How a structure is drawn, in the terms a drawing program's document settings
 * use: points, with a bond length they are measured against. Only their ratios
 * matter on screen, where the drawing is scaled to the pane, but they are what
 * a journal's guidelines quote. */
export interface StructureStyle {
  bondLength: number;
  lineWidth: number;
  /** Width of a wedge at its wide end. */
  boldWidth: number;
  /** Gap between the lines of a double bond, as a share of the bond length. */
  spacing: number;
  hashSpacing: number;
  /** Air left between a bond and the symbol it ends at. */
  margin: number;
  fontSize: number;
  font: string;
  /** Heteroatom symbols in their usual colours instead of the text colour. */
  elementColours: boolean;
  /** Write the hydrogens of a terminal carbon (CH3) too. */
  terminalCarbons: boolean;
}

/** ACS Document 1996, the settings journals of the American Chemical Society ask
 * drawings to follow. */
export const ACS_1996: StructureStyle = {
  bondLength: 14.4,
  lineWidth: 0.6,
  boldWidth: 2,
  spacing: 0.18,
  hashSpacing: 2.5,
  margin: 1.6,
  fontSize: 10,
  font: 'Arial',
  elementColours: false,
  terminalCarbons: false,
};

/** A symbol is written in its element's usual colour when the style asks. */
const ELEMENT_COLOURS: Record<string, string> = {
  N: '#3050f8',
  O: '#e02020',
  S: '#a08000',
  P: '#e07000',
  F: '#20a030',
  Cl: '#20a030',
  Br: '#a03020',
  I: '#8020a0',
};

export type Item =
  | { kind: 'line'; x1: number; y1: number; x2: number; y2: number; w: number }
  | { kind: 'poly'; points: string }
  | {
      kind: 'text';
      x: number;
      y: number;
      text: string;
      size: number;
      anchor: 'start' | 'middle' | 'end';
      colour?: string;
    };

export interface Picture {
  /** In bond lengths, y down. */
  items: Item[];
  points: { x: number; y: number }[];
  /** Where each atom's hydrogens are, and how far a click on them reaches. */
  hits: { x: number; y: number; r: number }[];
  box: { x: number; y: number; w: number; h: number };
}

/** Widths of the characters a label is made of, in ems: reckoned rather than
 * measured, since the SVG painter cannot measure and the export has to agree. */
const em = (text: string) =>
  [...text].reduce((sum, c) => sum + (/[A-Z]/.test(c) ? 0.7 : /[0-9]/.test(c) ? 0.56 : 0.55), 0);

const SUBSCRIPT = 0.7;

export function labelled(m: Molecule, i: number, style: StructureStyle): boolean {
  const atom = m.atoms[i];
  const degree = m.bonds.filter((b) => b.a === i || b.b === i).length;
  return (
    atom.symbol !== 'C' ||
    atom.charge !== 0 ||
    degree === 0 ||
    (style.terminalCarbons && degree === 1 && atom.h > 0)
  );
}

/** Turns a molecule into the lines, wedges and letters it is drawn with. */
export function draw(m: Molecule, style: StructureStyle): Picture {
  const { points } = layout(m);
  const scale = (pt: number) => pt / style.bondLength;
  const lineW = scale(style.lineWidth);
  const font = scale(style.fontSize);
  const items: Item[] = [];
  const near = m.atoms.map((_, i) =>
    m.bonds.flatMap((b) => (b.a === i ? [b.b] : b.b === i ? [b.a] : [])),
  );
  const written = m.atoms.map((_, i) => labelled(m, i, style));

  /** How far along `u` a bond leaves the box round the symbol at `i`, and a margin. */
  const clip = (i: number, ux: number, uy: number) => {
    if (!written[i]) return 0;
    const halfW = (em(m.atoms[i].symbol) * font) / 2;
    const halfH = font * 0.36;
    const out = Math.min(
      halfW / Math.max(Math.abs(ux), 1e-6),
      halfH / Math.max(Math.abs(uy), 1e-6),
    );
    return out + scale(style.margin);
  };

  /** The side of a bond the ring it is in lies on: +1 or -1 by the sign of the
   * cross product, or 0 when it is in no ring, which is shortest path back
   * from one end to the other without the bond itself. */
  const ringSide = (a: number, b: number, ux: number, uy: number): number => {
    const from = new Map<number, number>([[a, -1]]);
    const queue = [a];
    for (let head = 0; head < queue.length; head++) {
      const at = queue[head];
      for (const next of near[at]) {
        if ((at === a && next === b) || from.has(next)) continue;
        from.set(next, at);
        if (next === b) {
          const path = [b];
          for (let k = from.get(b) ?? -1; k >= 0; k = from.get(k) ?? -1) path.push(k);
          if (path.length > 8) return 0;
          const cx = path.reduce((s, k) => s + points[k].x, 0) / path.length;
          const cy = path.reduce((s, k) => s + points[k].y, 0) / path.length;
          const mx = (points[a].x + points[b].x) / 2;
          const my = (points[a].y + points[b].y) / 2;
          return Math.sign(ux * (cy - my) - uy * (cx - mx));
        }
        queue.push(next);
      }
    }
    return 0;
  };

  /** Substituents lie on one side of a bond or the other. */
  const crowdedSide = (a: number, b: number, ux: number, uy: number): number => {
    let total = 0;
    for (const [end, other] of [
      [a, b],
      [b, a],
    ]) {
      for (const k of near[end]) {
        if (k === other) continue;
        total += Math.sign(ux * (points[k].y - points[end].y) - uy * (points[k].x - points[end].x));
      }
    }
    return Math.sign(total);
  };

  m.bonds.forEach((bond) => {
    const { a, b } = bond;
    const [p, q] = [points[a], points[b]];
    const len = Math.hypot(q.x - p.x, q.y - p.y) || 1;
    const [ux, uy] = [(q.x - p.x) / len, (q.y - p.y) / len];
    const [nx, ny] = [-uy, ux];
    const from = clip(a, ux, uy);
    const to = clip(b, ux, uy);
    const line = (offset: number, start: number, end: number) =>
      items.push({
        kind: 'line',
        x1: p.x + ux * start + nx * offset,
        y1: p.y + uy * start + ny * offset,
        x2: q.x - ux * end + nx * offset,
        y2: q.y - uy * end + ny * offset,
        w: lineW,
      });

    if (bond.stereo === 1) {
      const [s, e] = [from, len - to];
      const half = scale(style.boldWidth) / 2;
      const at = (t: number, side: number) =>
        `${p.x + ux * t + nx * side},${p.y + uy * t + ny * side}`;
      items.push({
        kind: 'poly',
        points: [at(s, 0), at(e, half * (e / len)), at(e, -half * (e / len))].join(' '),
      });
      return;
    }
    if (bond.stereo === 6) {
      const step = scale(style.hashSpacing);
      const [s, e] = [from, len - to];
      for (let t = s + step; t <= e + 1e-6; t += step) {
        const half = (scale(style.boldWidth) / 2) * (t / len);
        items.push({
          kind: 'line',
          x1: p.x + ux * t + nx * half,
          y1: p.y + uy * t + ny * half,
          x2: p.x + ux * t - nx * half,
          y2: p.y + uy * t - ny * half,
          w: lineW,
        });
      }
      return;
    }

    if (bond.order === 3) {
      line(0, from, to);
      line(style.spacing, from, to);
      line(-style.spacing, from, to);
    } else if (bond.order === 2 || bond.order === 4) {
      // One line on the axis and one beside it, on the side of the ring, shortened
      // where the two meet the neighbouring bonds. Centred when there is no side:
      // a carbonyl or a bond with nothing either side of it.
      const terminal = near[a].length === 1 || near[b].length === 1;
      const side = ringSide(a, b, ux, uy) || (terminal ? 0 : crowdedSide(a, b, ux, uy));
      if (side === 0) {
        line(style.spacing / 2, from, to);
        line(-style.spacing / 2, from, to);
      } else {
        const inset = style.spacing * 0.577;
        line(0, from, to);
        line(side * style.spacing, Math.max(from, inset), Math.max(to, inset));
      }
    } else {
      line(0, from, to);
    }
  });

  // Symbols. The hydrogens stand on the side away from the bonds (HO, H2N), so the
  // symbol itself stays where the atom is.
  const extent = { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity };
  const grow = (x: number, y: number) => {
    extent.x0 = Math.min(extent.x0, x);
    extent.x1 = Math.max(extent.x1, x);
    extent.y0 = Math.min(extent.y0, y);
    extent.y1 = Math.max(extent.y1, y);
  };
  points.forEach((p) => grow(p.x, p.y));
  const text = (
    x: number,
    y: number,
    label: string,
    size: number,
    anchor: 'start' | 'middle' | 'end',
    colour?: string,
  ) => {
    items.push({ kind: 'text', x, y, text: label, size, anchor, colour });
    const w = em(label) * size;
    const x0 = anchor === 'start' ? x : anchor === 'end' ? x - w : x - w / 2;
    grow(x0, y - size / 2);
    grow(x0 + w, y + size / 2);
  };
  // Where a hydrogen is taken from: the atom itself for a bare carbon, the letters
  // that stand for the hydrogens when they are written.
  const hits = points.map((p) => ({ x: p.x, y: p.y, r: 0.45 }));
  m.atoms.forEach((atom, i) => {
    if (!written[i]) return;
    const p = points[i];
    const colour = style.elementColours ? ELEMENT_COLOURS[atom.symbol] : undefined;
    const half = (em(atom.symbol) * font) / 2;
    // Where the bonds leave decides where the hydrogens stand: opposite them, below
    // when two bonds both go up (the NH of a ring), else to the side.
    let sx = 0;
    let sy = 0;
    for (const k of near[i]) {
      const d = Math.hypot(points[k].x - p.x, points[k].y - p.y) || 1;
      sx += (points[k].x - p.x) / d;
      sy += (points[k].y - p.y) / d;
    }
    const vertical = near[i].length > 1 && Math.abs(sy) > Math.abs(sx);
    const left = !vertical && sx > 0.15;
    text(p.x, p.y, atom.symbol, font, 'middle', colour);
    let right = p.x + half;
    if (atom.h > 0) {
      const count = atom.h > 1 ? String(atom.h) : '';
      const countW = em(count) * font * SUBSCRIPT;
      const hW = em('H') * font;
      const drop = font * 0.25;
      let cx: number;
      let cy = p.y;
      if (vertical) {
        // The group is centred under (or over) the symbol.
        cy = p.y + (sy < 0 ? 1 : -1) * font * 0.98;
        cx = p.x;
        text(p.x - countW / 2, cy, 'H', font, 'middle', colour);
        if (count)
          text(p.x - countW / 2 + hW / 2, cy + drop, count, font * SUBSCRIPT, 'start', colour);
      } else if (left) {
        const edge = p.x - half;
        if (count) text(edge, p.y + drop, count, font * SUBSCRIPT, 'end', colour);
        text(edge - countW, p.y, 'H', font, 'end', colour);
        cx = edge - (hW + countW) / 2;
      } else {
        text(right, p.y, 'H', font, 'start', colour);
        if (count) text(right + hW, p.y + drop, count, font * SUBSCRIPT, 'start', colour);
        cx = right + (hW + countW) / 2;
        right += hW + countW;
      }
      hits[i] = { x: cx, y: cy, r: Math.max(font * 0.6, (hW + countW) / 2 + 0.1) };
    }
    if (atom.charge !== 0) {
      const c = Math.abs(atom.charge);
      const sign = atom.charge > 0 ? '+' : '−';
      text(
        right,
        p.y - font * 0.35,
        c > 1 ? `${c}${sign}` : sign,
        font * SUBSCRIPT,
        'start',
        colour,
      );
    }
  });

  const pad = 0.3;
  return {
    items,
    points,
    hits,
    box: {
      x: extent.x0 - pad,
      y: extent.y0 - pad,
      w: extent.x1 - extent.x0 + 2 * pad,
      h: extent.y1 - extent.y0 + 2 * pad,
    },
  };
}
