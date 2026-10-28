<script lang="ts">
  import { onMount } from 'svelte';
  import {
    fetchContours,
    fetchProjection,
    fetchTrace,
    type Document2DInfo,
    type DocumentInfo,
  } from './api';
  import { hexColor, TraceRenderer, type Vec4 } from './gl';
  import { isEditable, windowOpen } from './keys';
  import { CanvasPainter, paintScale } from './painter';
  import { paintCursor } from './cursors';
  import { PLOT_TEXT } from './page';
  import {
    pressPan,
    pressZoom,
    viewAction,
    wheelAction,
    type Tool,
    type ViewAction,
  } from './tools';
  import { nucleusLabel } from './traces';
  import {
    amend,
    contourLevels,
    historyOf,
    intensityAt,
    pan,
    ppmAt,
    record,
    step,
    zoomAt,
    zoomBox,
    type Axis,
    type History,
    type Limits,
    type View,
  } from './view';

  let {
    doc,
    projections = [undefined, undefined],
    palette,
    decimals,
    onerror,
    onreadout,
    onpickpeak,
    onremovepeak,
    armed = false,
    marked,
    onassignpeak,
    onhoverpeak,
    mode = $bindable(),
    zoomAxis = $bindable(),
    panAxis = $bindable(),
    crosshair = $bindable(),
  }: {
    doc: Document2DInfo;
    /** What is drawn beside the contours, along F2 above and along F1 at the
     *  left: an open spectrum, the 2D's own projection (`own`), or nothing. */
    projections?: [DocumentInfo | 'own' | undefined, DocumentInfo | 'own' | undefined];
    /** Positive contours in the first colour, negative in the second. */
    palette: string[];
    /** Decimal places for a shift, as everywhere else. */
    decimals: number;
    onerror: (message: string) => void;
    onreadout?: (text: string) => void;
    /** A click picking a cross peak, `[F2, F1]` in ppm, to be found within
     *  `radius` points of it along F2 and along F1. */
    onpickpeak?: (at: [number, number], radius: [number, number]) => void;
    /** A picked cross peak's own menu asked for it to go. */
    onremovepeak?: (index: number) => void;
    /** Atoms of the structure are waiting for a cross peak: a click on one's
     *  mark gives it them. */
    armed?: boolean;
    /** A cross peak lit from outside, when the pointer is over its atoms. */
    marked?: number;
    onassignpeak?: (index: number) => void;
    /** The cross peak the pointer is over, or none: its atoms light up. */
    onhoverpeak?: (index: number | undefined) => void;
    mode: Tool;
    zoomAxis: Axis;
    panAxis: Axis;
    crosshair: boolean;
  } = $props();

  /** Room for the F1 numbers and title at the right, and F2's below. */
  const AXIS_WIDTH = 64;
  const AXIS_HEIGHT = 44;
  /** How tall the spectrum above is, and how wide the one at the left. */
  const PROJECTION = 72;
  /** How near a click a cross peak's top is looked for, and how near its
   *  mark a right click takes hold of it, in pixels. */
  const PICK_PX = 6;
  /** Half the size of a picked cross peak's mark. */
  const MARK = 4;
  const LENS = 0.8;
  /** Each contour level this much above the one below it. */
  const LEVEL_STEP = 1.4;
  const LEVEL_COUNT = 16;
  /** The lowest contour this many times the noise, clear of it. */
  const NOISE_LEVELS = 6;
  /** Where the levels start: clear of the noise, and high enough that the
   *  ladder reaches the tallest peak. A magnitude COSY in testdata spans
   *  29 400 times its noise, and from 6 times it contoured 13 % of the plot,
   *  against 1 to 6 % for every other experiment there. */
  const startLevel = (d: Document2DInfo) =>
    Math.max(d.noise * NOISE_LEVELS, d.max / LEVEL_STEP ** (LEVEL_COUNT - 1));
  /** What + and - move the lowest level by. */
  const INTENSITY_STEP = 1.25;

  let area: HTMLDivElement;
  let glCanvas: HTMLCanvasElement;
  let overlay: HTMLCanvasElement;
  let renderer: TraceRenderer | undefined;
  let areaWidth = $state(0);
  let areaHeight = $state(0);
  // Where the contours start, past the spectra beside them.
  const left = $derived(projections[1] ? PROJECTION : 0);
  const top = $derived(projections[0] ? PROJECTION : 0);
  const width = $derived(Math.max(0, areaWidth - AXIS_WIDTH - left));
  const height = $derived(Math.max(0, areaHeight - AXIS_HEIGHT - top));
  /** A pointer event where the contours are: from their top left corner. */
  const at = (e: MouseEvent) => ({ x: e.offsetX - left, y: e.offsetY - top });
  const dpr = typeof devicePixelRatio === 'number' ? devicePixelRatio : 1;

  const pointOf = (dim: 0 | 1, ppm: number) => (ppm - doc.firstPpm[dim]) / doc.deltaPpm[dim];
  const limitsOf = (d: Document2DInfo): Limits => {
    const axis = (dim: 0 | 1) => ({
      left: d.firstPpm[dim],
      right: d.firstPpm[dim] + (d.points[dim] - 1) * d.deltaPpm[dim],
      minSpan: 16 * Math.abs(d.deltaPpm[dim]),
    });
    return { ...axis(0), y: axis(1) };
  };
  const limits = $derived(limitsOf(doc));
  // F1 runs high at the foot, so the view's y0 is its high end.
  const fullView = (l: Limits): View => ({
    x0: l.left,
    x1: l.right,
    y0: l.y?.left ?? 0,
    y1: l.y?.right ?? 0,
  });

  let view = $state<View>({ x0: 1, x1: 0, y0: 1, y1: 0 });
  let history = $state<History>(historyOf({ x0: 1, x1: 0, y0: 1, y1: 0 }));
  let lowest = $state(1);
  let shownId: number | undefined;

  // A different experiment starts from the whole of it and its own levels.
  $effect.pre(() => {
    if (doc.id === shownId) return;
    shownId = doc.id;
    view = fullView(limitsOf(doc));
    history = historyOf(view);
    lowest = startLevel(doc);
  });

  /** The lowest level, held between twice the noise and the tallest peak:
   *  further down every cell crosses every level, the lines run to hundreds of
   *  megabytes, and a level's count of values no longer fits the f32 it is
   *  sent in (2²⁴). */
  function setLowest(level: number) {
    lowest = Math.min(Math.max(level, doc.noise * 2), doc.max);
  }

  const levels = $derived(contourLevels(lowest, doc.max, LEVEL_STEP, LEVEL_COUNT));
  let loaded = $state(0);

  // One request at a time: a turn of the wheel asks for many sets of levels,
  // and only the last one wanted is worth working out.
  let fetching = false;
  let wanted: { id: number; levels: number[]; signed: boolean } | undefined;

  function load() {
    if (fetching || !wanted) return;
    const { id, levels: positive, signed } = wanted;
    wanted = undefined;
    fetching = true;
    const negative = signed ? positive.map((l) => -l) : [];
    fetchContours(id, [...positive, ...negative])
      .then(
        (lines) => {
          if (!renderer || id !== doc.id) return;
          renderer.setData(0, join(lines.slice(0, positive.length)), true);
          renderer.setData(1, join(lines.slice(positive.length)), true);
          loaded++;
        },
        (e: unknown) => onerror(String(e)),
      )
      .finally(() => {
        fetching = false;
        load();
      });
  }

  $effect(() => {
    wanted = { id: doc.id, levels, signed: doc.signed };
    load();
  });

  function join(parts: Float32Array[]) {
    const all = new Float32Array(parts.reduce((n, p) => n + p.length, 0));
    let at = 0;
    for (const p of parts) {
      all.set(p, at);
      at += p.length;
    }
    return all;
  }

  /** The spectra beside, as `ppm, intensity` pairs over what is in view. */
  let traces = $state<[Float32Array | undefined, Float32Array | undefined]>([undefined, undefined]);
  const asked = [0, 0];

  $effect(() => {
    const spans: [number, number, number][] = [
      [view.x0, view.x1, width],
      [view.y1, view.y0, height],
    ];
    for (const dim of [0, 1] as const) {
      const beside = projections[dim];
      const [a, b, pixels] = spans[dim];
      const n = ++asked[dim];
      if (!beside || pixels < 1) {
        traces[dim] = undefined;
        continue;
      }
      // A spectrum's own axis and points, or the 2D's along this side.
      const d =
        beside === 'own'
          ? {
              first: doc.firstPpm[dim],
              delta: doc.deltaPpm[dim],
              points: doc.points[dim],
              fetch: (s: number, e: number, c: number) => fetchProjection(doc.id, dim, s, e, c),
            }
          : {
              first: beside.firstPpm,
              delta: beside.deltaPpm,
              points: beside.points,
              // A spectrum reprocessed is fetched again.
              revision: beside.revision,
              fetch: (s: number, e: number, c: number) => fetchTrace(beside.id, s, e, c),
            };
      const [i0, i1] = [a, b].map((ppm) => (ppm - d.first) / d.delta).sort((p, q) => p - q);
      const start = Math.min(Math.max(Math.floor(i0), 0), d.points);
      const end = Math.min(Math.max(Math.ceil(i1) + 1, start), d.points);
      if (end - start < 2) {
        traces[dim] = undefined;
        continue;
      }
      // One column per device pixel, as the spectrum's own plot asks: in CSS
      // pixels a Retina screen drew the side at half its resolution.
      d.fetch(start, end, Math.round(pixels * dpr)).then(
        (xy) => {
          if (n !== asked[dim]) return;
          const pairs = new Float32Array(xy.length);
          for (let k = 0; k < xy.length; k += 2) {
            pairs[k] = d.first + (start + xy[k]) * d.delta;
            pairs[k + 1] = xy[k + 1];
          }
          traces[dim] = pairs;
        },
        (e: unknown) => onerror(String(e)),
      );
    }
  });

  let spaceHeld = $state(false);
  const tool = $derived<Tool>(spaceHeld ? 'pan' : mode);
  let cursor = $state<{ x: number; y: number }>();
  /** The cross peak whose mark the pointer is over. */
  let hovered = $state<number>();
  let drag = $state<{ x: number; y: number; start: View; moved: boolean }>();
  let box = $state<{ x0: number; y0: number; x1: number; y1: number }>();

  const inside = (x: number, y: number) => x >= 0 && x <= width && y >= 0 && y <= height;

  const readout = $derived.by(() => {
    if (!cursor || !inside(cursor.x, cursor.y) || width === 0) return '';
    const f2 = ppmAt(view, cursor.x, width).toFixed(decimals);
    const f1 = intensityAt(view, cursor.y, height).toFixed(decimals);
    return `F2 ${f2}   F1 ${f1} ppm`;
  });
  $effect(() => onreadout?.(readout));

  function commit(next: View) {
    view = next;
    history = record(history, next);
  }

  function runView(action: ViewAction) {
    switch (action) {
      case 'zoom':
        return void ([mode, zoomAxis] = pressZoom(mode, zoomAxis));
      case 'zoomOut':
        return void (mode = 'zoomOut');
      case 'pan':
        return void ([mode, panAxis] = pressPan(mode, panAxis));
      case 'full':
        return commit(fullView(limits));
      // A 2D's intensity is its contour levels: H puts them back where they
      // started, clear of the noise.
      case 'fit':
        return setLowest(startLevel(doc));
      case 'larger':
        return setLowest(lowest / INTENSITY_STEP);
      case 'smaller':
        return setLowest(lowest * INTENSITY_STEP);
      case 'back':
      case 'forward':
        history = step(history, action === 'back' ? -1 : 1);
        view = history.views[history.at];
        return;
      case 'crosshair':
        return void (crosshair = !crosshair);
      case 'pointer':
        return void (mode = 'pointer');
      // Typing the ends of the view (M) is for a spectrum's one axis.
      case 'range':
        return;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (isEditable(e.target) || windowOpen()) return;
    // Cmd+K picks peak by peak, as on a spectrum.
    if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'k') {
      mode = 'peakByPeak';
      e.preventDefault();
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === ' ') {
      spaceHeld = true;
      e.preventDefault();
      return;
    }
    const action = viewAction(e);
    if (!action) return;
    runView(action);
    e.preventDefault();
  }

  function onkeyup(e: KeyboardEvent) {
    if (e.key === ' ') spaceHeld = false;
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const turn = wheelAction(e);
    if (turn.kind === 'zoom') {
      const p = at(e);
      view = zoomAt(view, p.x, p.y, width, height, turn.factor, 'xy', limits);
      history = amend(history, view);
    } else if (turn.kind === 'pan') {
      view = pan(view, -turn.amount, 0, width, height, limits);
      history = amend(history, view);
    } else {
      setLowest(lowest * turn.factor);
    }
  }

  function onpointerdown(e: PointerEvent) {
    const p = at(e);
    if (e.button !== 0 || !inside(p.x, p.y)) return;
    // Atoms waiting for a cross peak go to the one clicked; elsewhere the click
    // is an ordinary one, so the view can still be zoomed while they wait.
    // Picking peak by peak too: that is the tool still in hand right after the
    // cross peaks were picked.
    const takes = tool === 'pointer' || tool === 'peakByPeak';
    const hit = armed && takes ? peakAt(p.x, p.y) : undefined;
    if (hit !== undefined) {
      onassignpeak?.(hit);
      return;
    }
    overlay.setPointerCapture(e.pointerId);
    drag = { ...p, start: view, moved: false };
  }

  function onpointermove(e: PointerEvent) {
    const p = at(e);
    cursor = p;
    if (!drag) {
      const over = peakAt(p.x, p.y);
      if (over !== hovered) onhoverpeak?.((hovered = over));
      return;
    }
    const [dx, dy] = [p.x - drag.x, p.y - drag.y];
    drag.moved ||= Math.hypot(dx, dy) > 3;
    if (tool === 'pan') {
      view = pan(
        drag.start,
        panAxis === 'y' ? 0 : dx,
        panAxis === 'x' ? 0 : dy,
        width,
        height,
        limits,
      );
    } else if (tool === 'zoom' && drag.moved) {
      const clampX = (x: number) => Math.min(Math.max(x, 0), width);
      const clampY = (y: number) => Math.min(Math.max(y, 0), height);
      box = {
        x0: zoomAxis === 'y' ? 0 : drag.x,
        x1: zoomAxis === 'y' ? width : clampX(p.x),
        y0: zoomAxis === 'x' ? 0 : drag.y,
        y1: zoomAxis === 'x' ? height : clampY(p.y),
      };
    }
  }

  function onpointerup(e: PointerEvent) {
    if (!drag) return;
    const p = at(e);
    if (tool === 'pan' && drag.moved) {
      commit(view);
    } else if (tool === 'zoom') {
      commit(
        box && drag.moved
          ? zoomBox(view, box, width, height, zoomAxis, limits)
          : zoomAt(view, p.x, p.y, width, height, LENS, zoomAxis, limits),
      );
    } else if (tool === 'zoomOut') {
      commit(zoomAt(view, p.x, p.y, width, height, 1 / LENS, zoomAxis, limits));
    } else if (tool === 'peakByPeak' && !drag.moved) {
      // The top is looked for within a few pixels, in each axis's own points.
      const points = (span: number, pixels: number, delta: number) =>
        Math.max(1, Math.round((PICK_PX * Math.abs(span / pixels)) / Math.abs(delta)));
      onpickpeak?.(
        [ppmAt(view, p.x, width), intensityAt(view, p.y, height)],
        [
          points(view.x1 - view.x0, width, doc.deltaPpm[0]),
          points(view.y0 - view.y1, height, doc.deltaPpm[1]),
        ],
      );
    }
    drag = undefined;
    box = undefined;
  }

  interface Colours {
    fg: string;
    bg: string;
    muted: string;
    accent: string;
    live: string;
    font: string;
  }
  let painted: Colours | undefined;

  function colours(): Colours {
    if (painted) return painted;
    const style = getComputedStyle(overlay);
    const get = (name: string) => style.getPropertyValue(name).trim();
    painted = {
      fg: get('--fg'),
      bg: get('--bg'),
      muted: get('--muted'),
      accent: get('--accent'),
      live: get('--live') || get('--accent'),
      font: get('--font') || 'system-ui, sans-serif',
    };
    return painted;
  }

  const rgba = (hex: string): Vec4 => (/^#[0-9a-f]{6}$/i.test(hex) ? hexColor(hex) : [0, 0, 0, 1]);

  function drawContours() {
    if (!renderer || width === 0) return;
    // Buffers are in points; the view is in ppm, F1's high end at the foot.
    const inPoints: Vec4 = [
      pointOf(0, view.x0),
      pointOf(0, view.x1),
      pointOf(1, view.y0),
      pointOf(1, view.y1),
    ];
    renderer.draw([
      { key: 0, view: inPoints, color: rgba(palette[0]) },
      { key: 1, view: inPoints, color: rgba(palette[1] ?? palette[0]) },
    ]);
  }

  function drawOverlay() {
    const ctx = overlay?.getContext('2d');
    if (!ctx || areaWidth === 0) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, areaWidth, areaHeight);
    // Everything from here is measured from the contours' top left corner.
    ctx.translate(left, top);
    const c = colours();
    const p = new CanvasPainter(ctx, c.font);
    const size = PLOT_TEXT.axis;
    p.line([0, height + 0.5, width + 0.5, height + 0.5, width + 0.5, 0], c.muted);
    drawProjections(ctx, p, c);
    paintScale(p, {
      from: view.x0,
      to: view.x1,
      length: width,
      top: height + 0.5,
      color: c.fg,
      size,
      gap: 8,
      title: `${nucleusLabel(doc.nuclei[0])} ppm`,
    });
    paintScale(p, {
      from: view.y1,
      to: view.y0,
      length: height,
      top: width + 0.5,
      color: c.fg,
      size,
      gap: 8,
      vertical: true,
    });
    p.text(width + AXIS_WIDTH - 4, height / 2, `${nucleusLabel(doc.nuclei[1])} ppm`, {
      color: c.fg,
      size,
      align: 'center',
      baseline: 'alphabetic',
      vertical: true,
    });
    // Picked cross peaks, each a small cross on its top with its number, which
    // is how the list and the structure name it.
    doc.peaks.forEach(({ at: [f2, f1] }, k) => {
      const [x, y] = markAt(f2, f1);
      if (!inside(x, y)) return;
      const lit = k === marked || k === hovered;
      const r = lit ? MARK * 1.6 : MARK;
      const ink = lit ? c.live : c.accent;
      p.line([x - r, y, x + r, y], ink);
      p.line([x, y - r, x, y + r], ink);
      p.text(x + r + 2, y - r, String(k + 1), {
        color: ink,
        size: PLOT_TEXT.peaks,
        bold: lit,
      });
    });
    if (box) {
      p.rect(
        Math.min(box.x0, box.x1),
        Math.min(box.y0, box.y1),
        Math.abs(box.x1 - box.x0),
        Math.abs(box.y1 - box.y0),
        c.accent,
        0.12,
      );
    }
    // Through the spectra beside as well, which is what a cross peak is read by.
    if (crosshair && cursor && inside(cursor.x, cursor.y)) {
      p.line([cursor.x, -top, cursor.x, height], c.muted, [4, 3]);
      p.line([-left, cursor.y, width, cursor.y], c.muted, [4, 3]);
    }
    // Last, over everything: the pointer of a chosen tool, as in 1D.
    if (cursor && tool !== 'pointer') {
      const axis = tool === 'pan' ? panAxis : tool === 'zoom' ? zoomAxis : 'xy';
      paintCursor(p, cursor.x, cursor.y, tool, axis, c.fg, c.bg);
    }
  }

  /** Where a cross peak's mark is, from the contours' top left corner. */
  function markAt(f2: number, f1: number): [number, number] {
    return [
      ((f2 - view.x0) / (view.x1 - view.x0)) * width,
      ((f1 - view.y1) / (view.y0 - view.y1)) * height,
    ];
  }

  /** The picked cross peak whose mark is under a right click, if any. */
  function peakAt(x: number, y: number) {
    const index = doc.peaks.findIndex(({ at: [f2, f1] }) => {
      const [mx, my] = markAt(f2, f1);
      return Math.hypot(mx - x, my - y) <= PICK_PX;
    });
    return index < 0 ? undefined : index;
  }

  /** The spectra beside the contours, each scaled to its tallest point in
   *  view and kept to its own strip. */
  function drawProjections(ctx: CanvasRenderingContext2D, p: CanvasPainter, c: Colours) {
    const strips = [
      { pairs: traces[0], box: [0, -top, width, top] as const },
      { pairs: traces[1], box: [-left, 0, left, height] as const },
    ];
    strips.forEach(({ pairs, box: [x, y, w, h] }, dim) => {
      if (!pairs || w <= 0 || h <= 0) return;
      let tallest = 0;
      for (let k = 1; k < pairs.length; k += 2) tallest = Math.max(tallest, Math.abs(pairs[k]));
      if (tallest === 0) return;
      const room = (dim === 0 ? h : w) - 10;
      const points = new Float32Array(pairs.length);
      for (let k = 0; k < pairs.length; k += 2) {
        const reach = (pairs[k + 1] / tallest) * room;
        if (dim === 0) {
          points[k] = ((pairs[k] - view.x0) / (view.x1 - view.x0)) * width;
          points[k + 1] = -4 - reach;
        } else {
          points[k] = -4 - reach;
          points[k + 1] = ((pairs[k] - view.y1) / (view.y0 - view.y1)) * height;
        }
      }
      ctx.save();
      ctx.beginPath();
      ctx.rect(x, y, w, h);
      ctx.clip();
      p.line(points, c.fg);
      ctx.restore();
    });
    // Each side says what it is: the 2D's own projection has a tenth of a
    // spectrum's points, and taken for the spectrum it looks badly drawn.
    projections.forEach((beside, dim) => {
      if (!beside || (dim === 0 ? top : left) === 0) return;
      const what = beside === 'own' ? 'projection' : beside.name;
      const label = `${nucleusLabel(doc.nuclei[dim])} · ${what}`;
      const style = { color: c.muted, size: PLOT_TEXT.peaks };
      if (dim === 0) p.text(4, -top + 12, label, style);
      else p.text(-left + 12, height - 4, label, { ...style, vertical: true });
    });
  }

  // The drawing buffer follows the contours' own box, which moves when a
  // spectrum beside them comes or goes.
  $effect(() => {
    const [w, h] = [Math.max(1, width), Math.max(1, height)].map((v) => Math.round(v * dpr));
    // Setting a size, even the same one, clears and reallocates the buffer.
    if (glCanvas.width !== w) glCanvas.width = w;
    if (glCanvas.height !== h) glCanvas.height = h;
    void [view, palette, loaded];
    drawContours();
  });

  $effect(() => {
    void [
      view,
      box,
      cursor,
      crosshair,
      areaWidth,
      areaHeight,
      doc,
      decimals,
      traces,
      marked,
      projections,
    ];
    drawOverlay();
  });

  onMount(() => {
    try {
      renderer = new TraceRenderer(glCanvas);
    } catch (e) {
      onerror(String(e));
    }
    const observer = new ResizeObserver(() => {
      areaWidth = area.clientWidth;
      areaHeight = area.clientHeight;
      overlay.width = Math.round(areaWidth * dpr);
      overlay.height = Math.round(areaHeight * dpr);
      drawOverlay();
    });
    observer.observe(area);
    overlay.addEventListener('wheel', onwheel, { passive: false });
    // A colour changes with the theme, written on <html>, or the system's.
    const forget = () => {
      painted = undefined;
      drawOverlay();
    };
    const themeWatch = new MutationObserver(forget);
    themeWatch.observe(document.documentElement, { attributeFilter: ['style', 'data-theme'] });
    const scheme = matchMedia('(prefers-color-scheme: dark)');
    scheme.addEventListener('change', forget);
    const release = () => (spaceHeld = false);
    window.addEventListener('keydown', onkeydown);
    window.addEventListener('keyup', onkeyup);
    window.addEventListener('blur', release);
    return () => {
      observer.disconnect();
      themeWatch.disconnect();
      scheme.removeEventListener('change', forget);
      overlay.removeEventListener('wheel', onwheel);
      window.removeEventListener('keydown', onkeydown);
      window.removeEventListener('keyup', onkeyup);
      window.removeEventListener('blur', release);
      onreadout?.('');
    };
  });
</script>

<div
  class="area"
  bind:this={area}
  data-tool={tool}
  data-cursor={tool === 'pointer' ? undefined : 'drawn'}
>
  <canvas
    class="contours"
    bind:this={glCanvas}
    style:left="{left}px"
    style:top="{top}px"
    style:width="{width}px"
    style:height="{height}px"
  ></canvas>
  <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
  <canvas
    class="overlay"
    bind:this={overlay}
    tabindex="0"
    role="application"
    aria-label="2D contour plot of {doc.name}. Z zooms, P pans, F shows the whole of it, plus and minus move the contour levels"
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    onpointerleave={() => {
      cursor = undefined;
      if (hovered !== undefined) onhoverpeak?.((hovered = undefined));
    }}
    oncontextmenu={(e) => {
      const p = at(e);
      const index = peakAt(p.x, p.y);
      // Nothing of ours under the pointer: the system's own menu stands.
      if (index === undefined) return;
      e.preventDefault();
      onremovepeak?.(index);
    }}
  ></canvas>
</div>

<style>
  .area {
    position: relative;
    height: 100%;
    overflow: hidden;
  }

  canvas {
    position: absolute;
    top: 0;
    left: 0;
  }

  .overlay {
    width: 100%;
    height: 100%;
    outline: none;
  }

  /* As in 1D: a chosen tool's pointer is drawn (`paintCursor`), the system's
     hidden, since an image cursor flickers wherever the plot repaints. */
  .area[data-cursor='drawn'] {
    cursor: none;
  }
</style>
