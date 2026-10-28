<script lang="ts">
  import { onMount } from 'svelte';
  import { filterCommands, type PaletteCommand } from './palette';

  let { commands, onclose }: { commands: PaletteCommand[]; onclose: () => void } = $props();

  let input: HTMLInputElement;
  let query = $state('');
  let selected = $state(0);
  const matches = $derived(filterCommands(commands, query));

  onMount(() => input.focus());

  function run(command: PaletteCommand | undefined) {
    if (!command) return;
    onclose();
    void command.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') selected = Math.min(selected + 1, matches.length - 1);
    else if (e.key === 'ArrowUp') selected = Math.max(selected - 1, 0);
    else if (e.key === 'Enter') run(matches[selected]);
    else if (e.key === 'Escape') onclose();
    else return;
    e.preventDefault();
  }
</script>

<button class="backdrop" tabindex="-1" aria-label="Close the command palette" onclick={onclose}
></button>
<div class="palette" role="dialog" aria-label="Command palette">
  <input
    bind:this={input}
    bind:value={query}
    placeholder="Type a command…"
    aria-label="Search commands"
    {onkeydown}
    oninput={() => (selected = 0)}
  />
  <ul>
    {#each matches as command, i (command.label)}
      <li>
        <button
          class:selected={i === selected}
          onmouseenter={() => (selected = i)}
          onclick={() => run(command)}
        >
          <span>{command.label}</span>
          {#if command.shortcut}<kbd>{command.shortcut}</kbd>{/if}
        </button>
      </li>
    {:else}
      <li class="empty">No matching command</li>
    {/each}
  </ul>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    border: none;
    border-radius: 0;
    background: rgb(0 0 0 / 20%);
  }

  .palette {
    position: fixed;
    top: 12vh;
    left: 50%;
    z-index: 11;
    width: min(520px, 90vw);
    transform: translateX(-50%);
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-overlay);
    background: var(--bg);
    box-shadow: var(--shadow-overlay);
  }

  input {
    box-sizing: border-box;
    width: 100%;
    min-height: 44px;
    padding: 0 var(--space-6);
    border: none;
    border-radius: 0;
    border-bottom: 1px solid var(--border);
    font: inherit;
    font-size: var(--text-title);
    color: inherit;
    background: none;
    outline: none;
  }

  ul {
    max-height: 50vh;
    margin: 0;
    padding: var(--space-2);
    overflow-y: auto;
    list-style: none;
  }

  li button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    border-color: transparent;
    background: none;
    text-align: left;
  }

  li button.selected {
    background: var(--live-soft);
  }

  input:focus-visible {
    outline: none;
  }

  kbd {
    color: var(--muted);
    font-family: inherit;
  }

  .empty {
    padding: var(--space-3) 10px;
    color: var(--muted);
  }
</style>
