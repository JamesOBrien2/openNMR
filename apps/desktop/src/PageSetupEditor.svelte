<script lang="ts">
  import { ChevronRight } from '@lucide/svelte';
  import FontChoice from './FontChoice.svelte';
  import { PAGE_SETUP, PAGES, type PageSetup } from './page';

  interface Props {
    setup: PageSetup;
    onchange: (setup: PageSetup) => void;
    /** Whether an exported PDF carries its project. */
    attach: boolean;
    onattach: (attach: boolean) => void;
  }

  let { setup, onchange, attach, onattach }: Props = $props();

  const set = (patch: Partial<PageSetup>) => onchange({ ...setup, ...patch });
  /** A number typed into a field, a decimal comma included; anything else, or
   *  anything outside `min`..`max`, puts back what was there. */
  function number(e: Event, key: keyof PageSetup, min: number, max = Infinity) {
    const input = e.currentTarget as HTMLInputElement;
    const value = Number(input.value.replace(',', '.'));
    if (Number.isFinite(value) && value >= min && value <= max) set({ [key]: value });
    else input.value = String(setup[key]);
  }

  const preset = $derived(
    PAGES.find((p) => p.width === setup.width && p.height === setup.height)?.name ?? '',
  );

  /** The things on the page, each with its text size and colour, as a
   *  properties sheet has them: what is changed is found under what it changes. */
  const PARTS = [
    ['Axis', 'textSize', 'axisColour'],
    ['Peak labels', 'peakSize', 'peakColour'],
    ['Integrals', 'integralSize', 'integralColour'],
    ['Multiplets', 'multipletSize', 'multipletColour'],
    ['Spectrum names', 'nameSize', 'nameColour'],
  ] as const;
</script>

<div class="sheet">
  <details open>
    <summary><ChevronRight size={14} /> Paper</summary>
    <div class="rows">
      <label>
        <span>Size</span>
        <select
          value={preset}
          onchange={(e) => {
            const page = PAGES.find((p) => p.name === e.currentTarget.value);
            if (page) set({ width: page.width, height: page.height });
          }}
        >
          {#each PAGES as page (page.name)}
            <option value={page.name}>{page.name}</option>
          {/each}
          <option value="" disabled>Custom</option>
        </select>
      </label>
      {#each [['width', 'Width', 20], ['height', 'Height', 20], ['margin', 'Margin', 0]] as const as [key, label, min] (key)}
        <label>
          <span>{label}</span>
          <span class="field">
            <input inputmode="decimal" value={setup[key]} onchange={(e) => number(e, key, min)} />
            <span class="unit">mm</span>
          </span>
        </label>
      {/each}
      <label>
        <span>Font</span>
        <FontChoice value={setup.font} onchange={(font) => set({ font })} />
      </label>
      <label>
        <span>Paper colour</span>
        <input
          type="color"
          value={setup.background}
          onchange={(e) => set({ background: e.currentTarget.value })}
        />
      </label>
      <label class="check" title="Opening such a PDF here gives the project back, with its data">
        <input
          type="checkbox"
          checked={attach}
          onchange={(e) => onattach(e.currentTarget.checked)}
        />
        <span>PDF carries the project</span>
      </label>
    </div>
  </details>

  <details open>
    <summary><ChevronRight size={14} /> Spectra</summary>
    <div class="rows">
      <label>
        <span>Colour</span>
        <select
          value={setup.traces}
          onchange={(e) => set({ traces: e.currentTarget.value as PageSetup['traces'] })}
        >
          <option value="black">Black</option>
          <option value="spectra">Their own</option>
          <option value="one">One colour</option>
        </select>
      </label>
      {#if setup.traces === 'one'}
        <label>
          <span>Which</span>
          <input
            type="color"
            value={setup.traceColour}
            onchange={(e) => set({ traceColour: e.currentTarget.value })}
          />
        </label>
      {/if}
      <label
        title="How far above the axis the lowest thing sits - the baseline, or the integrals under it"
      >
        <span>Above the axis</span>
        <span class="slider">
          <input
            type="range"
            min="0"
            max="60"
            step="1"
            value={setup.baselineGap}
            oninput={(e) => set({ baselineGap: Number(e.currentTarget.value) })}
          />
          <output>{setup.baselineGap} %</output>
        </span>
      </label>
    </div>
  </details>

  {#each PARTS as [label, size, colour] (label)}
    <details>
      <summary><ChevronRight size={14} /> {label}</summary>
      <div class="rows">
        <label title={size === 'textSize' ? 'The page is laid out in proportion to it' : undefined}>
          <span>Text size</span>
          <span class="field">
            <input inputmode="decimal" value={setup[size]} onchange={(e) => number(e, size, 3)} />
            <span class="unit">pt</span>
          </span>
        </label>
        <label>
          <span>Colour</span>
          <input
            type="color"
            value={setup[colour]}
            onchange={(e) => set({ [colour]: e.currentTarget.value })}
          />
        </label>
        {#if size === 'textSize'}
          <label>
            <span>Title</span>
            <input
              value={setup.axisTitle}
              placeholder="None"
              onchange={(e) => set({ axisTitle: e.currentTarget.value })}
            />
          </label>
        {/if}
      </div>
    </details>
  {/each}

  <button class="ghost reset" onclick={() => onchange({ ...PAGE_SETUP })}
    >Reset to the default page</button
  >
</div>

<style>
  .sheet {
    display: flex;
    flex-direction: column;
    padding: var(--space-3) var(--space-5) var(--space-6) var(--space-4);
  }

  details {
    border-bottom: 1px solid var(--border);
  }

  /* A Lucide chevron, as everywhere else, not the browser's own triangle. */
  summary {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: var(--space-4) 0;
    cursor: pointer;
    font-weight: 600;
    list-style: none;
  }

  summary::-webkit-details-marker {
    display: none;
  }

  details[open] > summary :global(svg) {
    transform: rotate(90deg);
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding-bottom: var(--space-5);
  }

  label {
    display: grid;
    grid-template-columns: 6.5rem 1fr;
    gap: var(--space-4);
    align-items: center;
    color: var(--muted);
  }

  label > select,
  label > input:not([type='color']):not([type='checkbox']),
  label > :global(input.font-name) {
    width: 100%;
    min-width: 0;
  }

  label.check {
    display: flex;
    gap: var(--space-3);
    color: var(--fg);
  }

  /* A number and its unit, the unit quiet inside the box. */
  .field {
    position: relative;
    display: flex;
    align-items: center;
  }

  .field input {
    width: 100%;
    min-width: 0;
    padding-right: 2.5rem;
    color: var(--fg);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .unit {
    position: absolute;
    right: var(--space-4);
    font-size: var(--text-label);
    pointer-events: none;
  }

  .slider {
    display: flex;
    gap: var(--space-3);
    align-items: center;
  }

  .slider input {
    flex: 1;
    min-width: 0;
  }

  .slider output {
    width: 3rem;
    color: var(--fg);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  input[type='color'] {
    width: 2.25rem;
    height: var(--control-height);
    padding: var(--space-1);
  }

  .reset {
    align-self: flex-start;
    margin-top: var(--space-5);
  }
</style>
