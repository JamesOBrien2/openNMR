import { niceTicks } from './view';

/** Room along the axis each number is given. */
const PX_PER_TICK = 60;

/** A ppm scale hanging below the line `y = top`: `from` at the left end and `to`
 * `length` pixels on, with ticks, numbers `gap` below the line and, for a page
 * (`title` given), a smaller tick between each two and what it measures under
 * the numbers, as a paper's figure is lettered. `vertical` stands it beside the
 * line `x = top` instead, `from` at the top and the numbers to its right, as a
 * 2D's F1 axis is drawn. */
export function paintScale(
  p: Painter,
  s: {
    from: number;
    to: number;
    length: number;
    top: number;
    color: string;
    size: number;
    gap: number;
    title?: string;
    vertical?: boolean;
  },
) {
  const { from, to, length, top, color, size } = s;
  if (s.vertical) {
    const ticks = niceTicks(from, to, Math.max(2, Math.floor(length / (PX_PER_TICK / 2))));
    const at = (value: number) => Math.round(((value - from) / (to - from)) * length) + 0.5;
    for (const value of ticks.values) {
      const y = at(value);
      p.line([top, y, top + 5, y], color);
      p.text(top + s.gap, y, value.toFixed(ticks.decimals), {
        color,
        size,
        align: 'left',
        baseline: 'middle',
      });
    }
    return;
  }
  const ticks = niceTicks(from, to, Math.max(2, Math.floor(length / PX_PER_TICK)));
  const at = (value: number) => Math.round(((value - from) / (to - from)) * length) + 0.5;
  const label = { color, size, align: 'center', baseline: 'top' } as const;
  if (s.title) {
    const step = ticks.values.length > 1 ? ticks.values[1] - ticks.values[0] : 0;
    for (const value of ticks.values) {
      for (const half of [value - step / 2, value + step / 2]) {
        const x = at(half);
        if (x > 0 && x < length) p.line([x, top, x, top + 3], color);
      }
    }
    p.text(length / 2, top + s.gap + size * 1.35, s.title, label);
  }
  for (const value of ticks.values) {
    const x = at(value);
    p.line([x, top, x, top + 5], color);
    p.text(x, top + s.gap, value.toFixed(ticks.decimals), label);
  }
}

export interface TextStyle {
  color: string;
  size: number;
  align?: 'left' | 'center' | 'right';
  baseline?: 'top' | 'middle' | 'alphabetic';
  /** Rotated 90° counter-clockwise around the anchor, for vertical labels. */
  vertical?: boolean;
  /** Heavier, for the one word a label is about. */
  bold?: boolean;
}

/** The drawing operations used for annotations, on screen and in SVG export. */
export interface Painter {
  /** A connected line through flat `x, y` pairs. */
  line(points: ArrayLike<number>, color: string, dash?: number[]): void;
  rect(x: number, y: number, w: number, h: number, color: string, fillOpacity: number): void;
  /** A filled shape through flat `x, y` pairs, closed back to the first: an area
   * rather than a line, which a rectangle cannot follow the outline of. */
  shape(points: ArrayLike<number>, color: string, fillOpacity: number): void;
  /** A rounded box, filled and outlined in different colours: an annotation that
   * is a thing to be clicked rather than a mark to be read. */
  chip(
    x: number,
    y: number,
    w: number,
    h: number,
    fill: string,
    stroke: string,
    radius: number,
  ): void;
  text(x: number, y: number, text: string, style: TextStyle): void;
  /** An icon given as SVG path data in a 24-unit box (Lucide's), its top left
   * at `x, y` and `size` across, stroked in `color` over a wider `halo` so it
   * reads on any trace. */
  icon(paths: string[], x: number, y: number, size: number, color: string, halo: string): void;
}

/** Lucide's stroke, and the halo drawn under it, in the icon's 24 units. */
const ICON_STROKE = 2;
const ICON_HALO = 5;

/** The plot writes in the interface's own typeface (`--font`), passed in by
 *  whoever paints; this is only what is used when none is. */
const FONT = 'system-ui, sans-serif';

export class CanvasPainter implements Painter {
  constructor(
    private readonly ctx: CanvasRenderingContext2D,
    private readonly font = FONT,
  ) {}

  line(points: ArrayLike<number>, color: string, dash: number[] = []) {
    const ctx = this.ctx;
    if (points.length < 4) return;
    ctx.save();
    ctx.strokeStyle = color;
    ctx.setLineDash(dash);
    ctx.beginPath();
    ctx.moveTo(points[0], points[1]);
    for (let k = 2; k < points.length; k += 2) ctx.lineTo(points[k], points[k + 1]);
    ctx.stroke();
    ctx.restore();
  }

  rect(x: number, y: number, w: number, h: number, color: string, fillOpacity: number) {
    const ctx = this.ctx;
    ctx.save();
    ctx.fillStyle = ctx.strokeStyle = color;
    ctx.globalAlpha = fillOpacity;
    ctx.fillRect(x, y, w, h);
    ctx.globalAlpha = 1;
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
    ctx.restore();
  }

  icon(paths: string[], x: number, y: number, size: number, color: string, halo: string) {
    const ctx = this.ctx;
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(size / 24, size / 24);
    ctx.lineCap = ctx.lineJoin = 'round';
    const shapes = paths.map((d) => new Path2D(d));
    for (const [width, ink] of [
      [ICON_HALO, halo],
      [ICON_STROKE, color],
    ] as const) {
      ctx.lineWidth = width;
      ctx.strokeStyle = ink;
      shapes.forEach((shape) => ctx.stroke(shape));
    }
    ctx.restore();
  }

  shape(points: ArrayLike<number>, color: string, fillOpacity: number) {
    const ctx = this.ctx;
    if (points.length < 6) return;
    ctx.save();
    ctx.fillStyle = color;
    ctx.globalAlpha = fillOpacity;
    ctx.beginPath();
    ctx.moveTo(points[0], points[1]);
    for (let k = 2; k < points.length; k += 2) ctx.lineTo(points[k], points[k + 1]);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  chip(x: number, y: number, w: number, h: number, fill: string, stroke: string, radius: number) {
    const ctx = this.ctx;
    const r = Math.min(radius, w / 2, h / 2);
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.arcTo(x + w, y, x + w, y + h, r);
    ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r);
    ctx.arcTo(x, y, x + w, y, r);
    ctx.closePath();
    ctx.fillStyle = fill;
    ctx.fill();
    ctx.strokeStyle = stroke;
    ctx.stroke();
    ctx.restore();
  }

  text(x: number, y: number, text: string, style: TextStyle) {
    const ctx = this.ctx;
    ctx.save();
    ctx.fillStyle = style.color;
    ctx.font = `${style.bold ? '600 ' : ''}${style.size}px ${this.font}`;
    ctx.textAlign = style.align ?? 'left';
    ctx.textBaseline = style.baseline ?? 'alphabetic';
    ctx.translate(x, y);
    if (style.vertical) ctx.rotate(-Math.PI / 2);
    ctx.fillText(text, 0, 0);
    ctx.restore();
  }
}

const ANCHOR = { left: 'start', center: 'middle', right: 'end' } as const;
const BASELINE = { top: 'hanging', middle: 'central', alphabetic: 'alphabetic' } as const;
const ENTITY: Record<string, string> = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' };
const escape = (s: string) => s.replace(/[&<>"]/g, (c) => ENTITY[c]);
const round = (v: number) => Math.round(v * 10) / 10;

export class SvgPainter implements Painter {
  private readonly parts: string[] = [];

  constructor(private readonly font = FONT) {}

  line(points: ArrayLike<number>, color: string, dash: number[] = []) {
    if (points.length < 4) return;
    const coords: string[] = [];
    for (let k = 0; k < points.length; k += 2) {
      coords.push(`${round(points[k])},${round(points[k + 1])}`);
    }
    const dashes = dash.length ? ` stroke-dasharray="${dash.join(' ')}"` : '';
    this.parts.push(
      `<polyline fill="none" stroke="${color}"${dashes} points="${coords.join(' ')}"/>`,
    );
  }

  rect(x: number, y: number, w: number, h: number, color: string, fillOpacity: number) {
    this.parts.push(
      `<rect x="${round(x)}" y="${round(y)}" width="${round(w)}" height="${round(h)}" fill="${color}" fill-opacity="${fillOpacity}" stroke="${color}"/>`,
    );
  }

  shape(points: ArrayLike<number>, color: string, fillOpacity: number) {
    if (points.length < 6) return;
    const coords: string[] = [];
    for (let k = 0; k < points.length; k += 2) {
      coords.push(`${round(points[k])},${round(points[k + 1])}`);
    }
    this.parts.push(
      `<polygon fill="${color}" fill-opacity="${fillOpacity}" stroke="none" points="${coords.join(' ')}"/>`,
    );
  }

  icon(paths: string[], x: number, y: number, size: number, color: string, halo: string) {
    const strokes = [
      [ICON_HALO, halo],
      [ICON_STROKE, color],
    ]
      .map(([width, ink]) =>
        paths.map((d) => `<path d="${d}" stroke="${ink}" stroke-width="${width}"/>`).join(''),
      )
      .join('');
    this.parts.push(
      `<g transform="translate(${round(x)} ${round(y)}) scale(${size / 24})" fill="none" stroke-linecap="round" stroke-linejoin="round">${strokes}</g>`,
    );
  }

  chip(x: number, y: number, w: number, h: number, fill: string, stroke: string, radius: number) {
    this.parts.push(
      `<rect x="${round(x)}" y="${round(y)}" width="${round(w)}" height="${round(h)}" rx="${round(radius)}" fill="${fill}" stroke="${stroke}"/>`,
    );
  }

  text(x: number, y: number, text: string, style: TextStyle) {
    const [sx, sy] = [round(x), round(y)];
    const rotate = style.vertical ? ` transform="rotate(-90 ${sx} ${sy})"` : '';
    this.parts.push(
      `<text x="${sx}" y="${sy}" fill="${style.color}" font-size="${style.size}" font-family="${escape(this.font)}"${style.bold ? ' font-weight="600"' : ''} text-anchor="${ANCHOR[style.align ?? 'left']}" dominant-baseline="${BASELINE[style.baseline ?? 'alphabetic']}"${rotate}>${escape(text)}</text>`,
    );
  }

  toString(width: number, height: number, background: string) {
    const size = `width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"`;
    const ground = `<rect width="100%" height="100%" fill="${background}"/>`;
    return `<svg xmlns="http://www.w3.org/2000/svg" ${size}>${ground}${this.parts.join('')}</svg>\n`;
  }
}
