<script lang="ts">
  import { ChevronDown, ChevronUp, Plus, SlidersHorizontal, Waypoints, X } from '@lucide/svelte';
  import type { DocumentInfo, Operation } from './api';
  import Menu, { type MenuItem } from './Menu.svelte';
  import {
    DEFAULT_AIRPLS,
    DEFAULT_WHITTAKER,
    DEFAULT_POLYNOMIAL,
    describe,
    isBaseline,
    moveOperation,
    setParameter,
    THROUGH_NAMES,
    withBaseline,
    withStep,
    type BaselineOperation,
  } from './operations';

  let {
    doc,
    onchange,
    onmanualphase,
    onbaselinepoints,
  }: {
    doc: DocumentInfo;
    onchange: (ops: Operation[]) => void;
    /** Opens the manual phase bar over the plot. */
    onmanualphase?: () => void;
    /** Opens the bar for drawing the baseline through points on the plot. */
    onbaselinepoints?: () => void;
  } = $props();

  /** Everything up to and including the Fourier transform is one card, "FID to
      spectrum": its few numbers (line broadening, zero filling) as fields, and
      the technical steps named on one quiet line. Listed one by one they were
      half the panel and all of its jargon, and a window step between two of them
      split them into two cards. */
  type Block =
    | { kind: 'conversion'; steps: { op: Operation; i: number }[] }
    | { kind: 'step'; op: Operation; i: number };

  const blocks = $derived.by(() => {
    const ft = doc.operations.findIndex((o) => o.op === 'ft');
    const out: Block[] = [];
    const conversion: { op: Operation; i: number }[] = [];
    doc.operations.forEach((op, i) => {
      // Conjugating is a correction of how the dataset stores its data, done
      // silently by every other program; it is not processing to be chosen.
      if (op.op === 'conjugate') return;
      if (i <= ft) conversion.push({ op, i });
      else out.push({ kind: 'step', op, i });
    });
    return conversion.length ? [{ kind: 'conversion' as const, steps: conversion }, ...out] : out;
  });

  /** What the technical steps are called here: short, and in the order they run. */
  const SHORT: Partial<Record<Operation['op'], string>> = {
    remove_digital_filter: 'Digital filter',
    digital_filter: 'Digital filter',
    ft: 'Fourier transform',
    dc_offset: 'DC offset',
    magnitude: 'Magnitude',
    power: 'Power',
    reverse: 'Reverse',
  };

  const TITLES: Partial<Record<Operation['op'], string>> = {
    exponential: 'Line broadening',
    gaussian: 'Gaussian window',
    sine_bell: 'Sine-bell window',
    zero_fill: 'Zero filling',
    phase: 'Phase',
    reference: 'Reference',
    baseline_whittaker: 'Baseline',
    baseline_airpls: 'Baseline',
    baseline_polynomial: 'Baseline',
    baseline_points: 'Baseline',
  };

  interface Parameter {
    key: string;
    label: string;
    title: string;
    step: number;
    unit?: string;
    /** Decimals shown; the value kept is never rounded. */
    digits: number;
  }

  const PARAMETERS: Partial<Record<Operation['op'], Parameter[]>> = {
    exponential: [
      { key: 'lb_hz', label: 'LB', title: 'Line broadening', step: 0.1, unit: 'Hz', digits: 2 },
    ],
    gaussian: [
      {
        key: 'g1_hz',
        label: 'G1',
        title: 'Inverse exponential width',
        step: 0.1,
        unit: 'Hz',
        digits: 2,
      },
      { key: 'g2_hz', label: 'G2', title: 'Gaussian width', step: 0.1, unit: 'Hz', digits: 2 },
      { key: 'g3', label: 'G3', title: 'Position of the maximum, 0 to 1', step: 0.05, digits: 2 },
    ],
    sine_bell: [
      {
        key: 'offset',
        label: 'Shift',
        title: 'Start of the bell, 0 to 1 (0.5 = cosine)',
        step: 0.05,
        digits: 2,
      },
      {
        key: 'power',
        label: 'Power',
        title: '1 = sine-bell, 2 = squared sine-bell',
        step: 1,
        digits: 0,
      },
    ],
    zero_fill: [
      { key: 'size', label: 'Zero fill', title: 'Points after zero filling', step: 1, digits: 0 },
    ],
    phase: [
      {
        key: 'p0',
        label: 'PH0',
        title: 'Zero-order phase at the pivot',
        step: 0.1,
        unit: '°',
        digits: 1,
      },
      {
        key: 'p1',
        label: 'PH1',
        title: 'First-order phase across the spectrum',
        step: 0.1,
        unit: '°',
        digits: 1,
      },
    ],
    reference: [
      {
        key: 'to_ppm',
        label: 'Shown as',
        title: 'What the reference line is called',
        step: 0.01,
        unit: 'ppm',
        digits: 3,
      },
    ],
  };

  const valueOf = (op: Operation, key: string) =>
    (op as unknown as Record<string, number>)[key] ?? 0;

  const shown = (value: number, digits: number) => Number(value.toFixed(digits));

  /** Steps that can be added by hand. Zero filling is here rather than in the
      default pipeline, since it decides how many points every later step sees. */
  const ADDABLE: { label: string; make: () => Operation }[] = [
    {
      label: 'Zero filling',
      make: () => ({
        op: 'zero_fill',
        size: 2 ** Math.ceil(Math.log2(Math.max(doc.points, 2))) * 2,
      }),
    },
    { label: 'Line broadening', make: () => ({ op: 'exponential', lb_hz: 0.3 }) },
    { label: 'Sine-bell window', make: () => ({ op: 'sine_bell', offset: 0.5, end: 1, power: 2 }) },
    { label: 'DC offset correction', make: () => ({ op: 'dc_offset' }) },
    { label: 'Magnitude', make: () => ({ op: 'magnitude' }) },
  ];

  /** A step already in the list is offered disabled: a second one would only
      repeat it, and the first can be changed in its card. */
  const addItems = $derived<MenuItem[]>(
    ADDABLE.map((entry) => {
      const kind = entry.make().op;
      const present = doc.operations.some((o) => o.op === kind);
      return {
        label: present ? `${entry.label} (already applied)` : entry.label,
        disabled: present,
        run: () => onchange(withStep(doc.operations, entry.make())),
      };
    }),
  );

  const remove = (i: number) => onchange(doc.operations.filter((_, k) => k !== i));

  let draft = $state<BaselineOperation>(DEFAULT_WHITTAKER);
  const applied = $derived(doc.operations.find(isBaseline));

  function setBaseline(next: BaselineOperation) {
    if (applied) onchange(withBaseline(doc.operations, next));
    else draft = next;
  }

  const methodOf = (value: string): BaselineOperation =>
    value === 'baseline_polynomial'
      ? DEFAULT_POLYNOMIAL
      : value === 'baseline_airpls'
        ? DEFAULT_AIRPLS
        : DEFAULT_WHITTAKER;
</script>

{#snippet baselineFields(baseline: BaselineOperation)}
  <label
    title="Whittaker smoother (Cobas et al. 2006) spans the signals it recognises; airPLS reweights iteratively; polynomial fits a smooth curve"
  >
    <span>Method</span>
    <select
      value={baseline.op}
      onchange={(e) => {
        // Points are set on the plot; the list keeps the step it had until then.
        if (e.currentTarget.value === 'baseline_points') {
          e.currentTarget.value = baseline.op;
          onbaselinepoints?.();
        } else setBaseline(methodOf(e.currentTarget.value));
      }}
    >
      <option value="baseline_whittaker">Whittaker smoother</option>
      <option value="baseline_airpls">airPLS</option>
      <option value="baseline_polynomial">Polynomial</option>
      {#if onbaselinepoints || baseline.op === 'baseline_points'}
        <option value="baseline_points">Through points</option>
      {/if}
    </select>
  </label>
  {#if baseline.op === 'baseline_airpls' || baseline.op === 'baseline_whittaker'}
    <label title="Higher is stiffer: the baseline ignores broader features">
      <span>Smooth Factor</span>
      <input
        type="number"
        min="1"
        step="any"
        value={baseline.lambda}
        onchange={(e) => {
          if (e.currentTarget.validity.valid && Number(e.currentTarget.value) > 0)
            setBaseline({ ...baseline, lambda: Number(e.currentTarget.value) });
        }}
      />
    </label>
    <label title="Higher is stiffer: the baseline ignores broader features">
      <span>Adjust factor</span>
      <input
        type="range"
        aria-label="Adjust Smooth Factor"
        min="3"
        max="11"
        step="0.5"
        value={Math.log10(baseline.lambda)}
        onchange={(e) => setBaseline({ ...baseline, lambda: 10 ** Number(e.currentTarget.value) })}
      />
    </label>
  {/if}
  {#if baseline.op === 'baseline_whittaker'}
    <label title="Automatically widen the filter to preserve detected broad signals">
      <span>Filter mode</span>
      <select
        value={baseline.auto_filter === false ? 'manual' : 'auto'}
        onchange={(e) =>
          setBaseline({ ...baseline, auto_filter: e.currentTarget.value === 'auto' })}
      >
        <option value="auto">Automatic</option>
        <option value="manual">Manual</option>
      </select>
    </label>
    <label
      title="Signal-recognition width in Hz. Automatic mode may widen it; manual mode uses this value, rounded to spectrum points (at least two)."
    >
      <span>{baseline.auto_filter === false ? 'Filter' : 'Minimum filter'}</span>
      <span class="value">
        <input
          type="number"
          min="0.1"
          step="any"
          value={baseline.filter_hz}
          onchange={(e) => {
            if (e.currentTarget.validity.valid && Number(e.currentTarget.value) > 0)
              setBaseline({ ...baseline, filter_hz: Number(e.currentTarget.value) });
          }}
        />
        <span class="unit">Hz</span>
      </span>
    </label>
    <p class="hint">
      Manual mode lets a smaller filter follow more background, but can also remove broad signals.
      Smooth Factor uses OpenNMR's own scale.
    </p>
  {:else if baseline.op === 'baseline_points'}
    <p class="hint">
      {baseline.points.length} points, {THROUGH_NAMES[baseline.through].toLowerCase()}
    </p>
  {:else if baseline.op === 'baseline_polynomial'}
    <label title="From 1, a straight line, to 20">
      <span>Order</span>
      <input
        type="number"
        min="1"
        max="20"
        value={baseline.order}
        onchange={(e) =>
          setBaseline({
            op: 'baseline_polynomial',
            order: Math.min(20, Math.max(1, Math.round(Number(e.currentTarget.value)))),
          })}
      />
    </label>
  {/if}
{/snippet}

{#snippet field(op: Operation, i: number, parameter: Parameter)}
  <label title={parameter.title}>
    <span>{parameter.label}</span>
    <span class="value">
      <input
        type="number"
        step={parameter.step}
        value={shown(valueOf(op, parameter.key), parameter.digits)}
        onchange={(e) => {
          const value = Number(e.currentTarget.value);
          if (Number.isFinite(value)) {
            onchange(setParameter(doc.operations, i, parameter.key, value));
          }
        }}
      />
      {#if parameter.unit}<span class="unit">{parameter.unit}</span>{/if}
    </span>
  </label>
{/snippet}

{#snippet controls(i: number, name: string)}
  <span class="controls">
    <button
      class="icon"
      title="Run this step earlier"
      aria-label="Move {name} up"
      disabled={i === 0}
      onclick={() => onchange(moveOperation(doc.operations, i, -1))}><ChevronUp size={14} /></button
    >
    <button
      class="icon"
      title="Run this step later"
      aria-label="Move {name} down"
      disabled={i === doc.operations.length - 1}
      onclick={() => onchange(moveOperation(doc.operations, i, 1))}
      ><ChevronDown size={14} /></button
    >
    <button
      class="icon"
      title="Remove this step"
      aria-label="Remove {name}"
      onclick={() => remove(i)}><X size={14} /></button
    >
  </span>
{/snippet}

<div class="steps">
  {#each blocks as block (block.kind === 'step' ? block.i : `c${block.steps[0].i}`)}
    {#if block.kind === 'conversion'}
      <section class="card">
        <header><span class="title">FID to spectrum</span></header>
        <div class="fields">
          {#each block.steps as { op, i } (i)}
            {#each PARAMETERS[op.op] ?? [] as parameter (parameter.key)}
              {@render field(op, i, parameter)}
            {/each}
          {/each}
        </div>
        <details>
          <summary class="hint">
            {block.steps
              .filter((s) => SHORT[s.op.op])
              .map((s) => SHORT[s.op.op])
              .join(' · ')}
          </summary>
          <ul>
            {#each block.steps as { op, i } (i)}
              <li>
                <span>{describe(op)}</span>
                {@render controls(i, describe(op))}
              </li>
            {/each}
          </ul>
        </details>
      </section>
    {:else}
      {@const { op, i } = block}
      {@const title = TITLES[op.op] ?? describe(op)}
      <section class="card">
        <header>
          <span class="title">{title}</span>
          {@render controls(i, title)}
        </header>
        <div class="fields">
          {#if op.op === 'reference'}
            <p class="hint">The line at {op.from_ppm.toFixed(3)} ppm</p>
          {:else if op.op === 'phase'}
            <p class="hint">Phase pivot: {(op.pivot * 100).toFixed(1)}% from the first point</p>
          {/if}
          {#each PARAMETERS[op.op] ?? [] as parameter (parameter.key)}
            {@render field(op, i, parameter)}
          {/each}
          {#if isBaseline(op)}
            {@render baselineFields(op)}
          {/if}
        </div>
        {#if op.op === 'phase' && onmanualphase}
          <button
            class="ghost adjust"
            title="Phase by hand on the plot (Shift+P)"
            onclick={onmanualphase}><SlidersHorizontal size={16} /> Adjust on the plot…</button
          >
        {:else if op.op === 'baseline_points' && onbaselinepoints}
          <button
            class="ghost adjust"
            title="Move, add or remove the points on the plot (Shift+B)"
            onclick={onbaselinepoints}><Waypoints size={16} /> Adjust on the plot…</button
          >
        {/if}
      </section>
    {/if}
  {/each}

  {#if !applied}
    <section class="card missing">
      <header><span class="title">Baseline</span><span class="hint">not corrected</span></header>
      <div class="fields">{@render baselineFields(draft)}</div>
      <button
        class="primary"
        title="Subtract a baseline with this method (B)"
        onclick={() => onchange(withBaseline(doc.operations, draft))}>Correct baseline</button
      >
    </section>
  {/if}

  <Menu items={addItems} title="Add a processing step">
    <Plus size={16} /> Add step
  </Menu>
</div>

<style>
  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-5);
  }

  /* Each step is one card: its name, and the few numbers it is set by, with
     their units beside them. */
  /* A step is told apart from the panel by its tint alone: a frame inside a
     tinted panel inside a framed column was three boxes for one thing. */
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: var(--bg);
  }

  /* A step not taken yet is a place for one, so it alone keeps an outline. */
  .card.missing {
    border: 1px dashed var(--border);
    background: none;
  }

  header,
  summary,
  li {
    display: flex;
    gap: var(--space-4);
    align-items: center;
    min-height: var(--control-height);
  }

  .title {
    font-weight: 600;
  }

  header .hint {
    flex: 1;
  }

  /* Reordering and removing are there on the step being pointed at. */
  .controls {
    display: flex;
    margin-left: auto;
    opacity: 0;
  }

  .card:hover > header .controls,
  .card:focus-within > header .controls,
  li:hover .controls,
  li:focus-within .controls {
    opacity: 1;
  }

  summary {
    cursor: pointer;
    list-style: none;
  }

  summary::-webkit-details-marker {
    display: none;
  }

  summary {
    font-size: var(--text-label);
  }

  details ul {
    display: flex;
    flex-direction: column;
    margin: var(--space-2) 0 0;
    padding: 0;
    list-style: none;
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .fields:empty {
    display: none;
  }

  .fields label {
    display: grid;
    grid-template-columns: 6rem 1fr;
    gap: var(--space-4);
    align-items: center;
    color: var(--muted);
  }

  .value {
    display: flex;
    gap: var(--space-3);
    align-items: center;
  }

  .value input {
    width: 6rem;
  }

  .unit {
    color: var(--muted);
  }

  /* A tool like the ribbon's, with the same icon, set where its step is. */
  .adjust {
    align-self: flex-start;
    margin-left: calc(-1 * var(--space-5) + var(--space-2));
  }

  .missing .primary {
    align-self: flex-start;
  }
</style>
