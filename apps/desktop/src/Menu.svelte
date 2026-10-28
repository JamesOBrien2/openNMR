<script lang="ts" module>
  /** One entry of a menu; `separator` draws a rule before it. */
  export interface MenuItem {
    label: string;
    shortcut?: string;
    disabled?: boolean;
    separator?: boolean;
    title?: string;
    run: () => unknown;
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    items: MenuItem[];
    /** What the button that opens the menu shows. */
    children: Snippet;
    title: string;
    label?: string;
    disabled?: boolean;
    class?: string;
  }

  let { items, children, title, label, disabled = false, class: kind = 'ghost' }: Props = $props();
  let open = $state(false);
  let root = $state<HTMLElement>();
  let popup = $state<HTMLElement>();
  /** Which way the popup opens, so it stays inside the window: from the end of
      its button when it would run off the right, upwards when off the bottom. */
  let flip = $state({ end: false, up: false });

  $effect(() => {
    if (!open || !popup || !root) return;
    const button = root.getBoundingClientRect();
    const { width, height } = popup.getBoundingClientRect();
    flip = {
      end: button.left + width > window.innerWidth && button.right - width >= 0,
      up: button.bottom + height > window.innerHeight && button.top - height >= 0,
    };
  });

  function choose(item: MenuItem) {
    open = false;
    void item.run();
  }

  // A press anywhere else, or Escape, closes it: a menu that has to be closed by
  // its own button is one the user has to remember is open.
  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (root && !root.contains(e.target as Node)) open = false;
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') open = false;
    };
    window.addEventListener('pointerdown', away, true);
    window.addEventListener('keydown', escape);
    return () => {
      window.removeEventListener('pointerdown', away, true);
      window.removeEventListener('keydown', escape);
    };
  });
</script>

<div class="menu" bind:this={root}>
  <button
    class={kind}
    {title}
    aria-label={label ?? title}
    aria-haspopup="menu"
    aria-expanded={open}
    {disabled}
    onclick={() => {
      flip = { end: false, up: false };
      open = !open;
    }}>{@render children()}</button
  >
  {#if open}
    <div class="popup" class:end={flip.end} class:up={flip.up} role="menu" bind:this={popup}>
      {#each items as item, i (i)}
        {#if item.separator}<hr />{/if}
        <button
          role="menuitem"
          title={item.title}
          disabled={item.disabled}
          onclick={() => choose(item)}
        >
          <span class="text">{item.label}</span>
          {#if item.shortcut}<kbd>{item.shortcut}</kbd>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
    display: inline-flex;
  }

  .popup {
    position: absolute;
    top: calc(100% + var(--space-2));
    left: 0;
    z-index: 20;
    display: grid;
    min-width: 14rem;
    max-width: 28rem;
    /* With room neither below nor above, it scrolls rather than leaving the window. */
    max-height: min(70vh, 32rem);
    overflow-y: auto;
    padding: var(--space-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-overlay);
    background: var(--bg);
    box-shadow: var(--shadow-overlay);
  }

  .popup.end {
    right: 0;
    left: auto;
  }

  .popup.up {
    top: auto;
    bottom: calc(100% + var(--space-2));
  }

  .popup button {
    display: flex;
    gap: var(--space-6);
    align-items: center;
    justify-content: space-between;
    border-color: transparent;
    background: none;
    text-align: left;
  }

  .popup button:hover:not(:disabled) {
    background: var(--hover);
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  kbd {
    color: var(--muted);
    font-family: inherit;
    font-size: var(--text-label);
  }

  hr {
    width: 100%;
    margin: var(--space-2) 0;
    border: none;
    border-top: 1px solid var(--border);
  }
</style>
