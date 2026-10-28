<script lang="ts">
  import type { Molecule } from './api';
  import { draw, type StructureStyle } from './structure';

  let {
    molecule,
    style,
    mine = [],
    linked = [],
    armed = [],
    assigned = {},
    onpick,
    onhover,
  }: {
    molecule: Molecule;
    style: StructureStyle;
    /** Atoms of the signal being worked on. */
    mine?: number[];
    /** Atoms whose signal the pointer is over on the plot. */
    linked?: number[];
    /** Atoms picked and waiting for a signal to be given to them. */
    armed?: number[];
    /** The signal each other atom is assigned to, by name. */
    assigned?: Record<number, string>;
    /** An atom with hydrogens was taken; without this nothing can be. */
    onpick?: (atom: number) => void;
    /** The atom under the pointer, or none. */
    onhover?: (atom: number | undefined) => void;
  } = $props();

  const picture = $derived(draw(molecule, style));

  /** The class under the pointer: equivalent hydrogens light up together, since one
   * click takes them all. */
  let hover = $state<number>();

  const look = (i: number) =>
    armed.includes(i) ? 'armed' : mine.includes(i) ? 'mine' : i in assigned ? 'taken' : '';
</script>

<svg
  viewBox="{picture.box.x} {picture.box.y} {picture.box.w} {picture.box.h}"
  style:aspect-ratio="{picture.box.w} / {picture.box.h}"
  role="group"
  aria-label="Structure, {molecule.formula}"
>
  {#if onpick}
    {#each molecule.atoms as atom, i (i)}
      {#if atom.h > 0}
        {@const p = picture.hits[i]}
        <circle
          class="h {look(i)}"
          class:near={hover === atom.class || linked.includes(i)}
          cx={p.x}
          cy={p.y}
          r={p.r}
          role="button"
          tabindex="0"
          aria-label="{atom.h} H{i in assigned ? `, signal ${assigned[i]}` : ''}"
          onpointerenter={() => {
            hover = atom.class;
            onhover?.(i);
          }}
          onpointerleave={() => {
            hover = undefined;
            onhover?.(undefined);
          }}
          onclick={() => onpick(i)}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onpick(i)}
        >
          <title>{atom.h} H{i in assigned ? ` · signal ${assigned[i]}` : ''}</title>
        </circle>
      {/if}
    {/each}
  {/if}
  <!-- The drawing is not what is pointed at: the circles under it are, and the letters lie
       over them, so a pointer on an H would otherwise land on the letter. -->
  <g
    pointer-events="none"
    stroke="currentColor"
    stroke-linecap="round"
    fill="currentColor"
    font-family={style.font}
  >
    {#each picture.items as item, k (k)}
      {#if item.kind === 'line'}
        <line x1={item.x1} y1={item.y1} x2={item.x2} y2={item.y2} stroke-width={item.w} />
      {:else if item.kind === 'poly'}
        <polygon points={item.points} stroke="none" />
      {:else}
        <text
          x={item.x}
          y={item.y + item.size * 0.36}
          font-size={item.size}
          text-anchor={item.anchor}
          fill={item.colour ?? 'currentColor'}
          stroke="none">{item.text}</text
        >
      {/if}
    {/each}
  </g>
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    max-height: 100%;
    /* Above the pane's paste layer, which lies over all of it. */
    position: relative;
    z-index: 1;
    /* The theme's own text colour, so the drawing needs no inversion on a dark page. */
    color: var(--fg);
  }
  .h {
    fill: transparent;
    stroke: none;
    cursor: pointer;
    outline: none;
  }
  .h.taken {
    fill: color-mix(in srgb, var(--muted) 30%, transparent);
  }
  .h.mine {
    fill: color-mix(in srgb, var(--live) 30%, transparent);
  }
  .h.armed {
    fill: color-mix(in srgb, var(--live) 45%, transparent);
    stroke: var(--live);
    stroke-width: 0.04;
    stroke-dasharray: 0.1 0.08;
  }
  .h.near,
  .h:focus-visible {
    fill: color-mix(in srgb, var(--live) 40%, transparent);
    stroke: var(--live);
    stroke-width: 0.04;
    stroke-dasharray: none;
  }
</style>
