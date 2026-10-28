<script lang="ts" module>
  import { systemFonts } from './api';

  // Asked for once: the back end reads every font file to answer.
  let fonts: Promise<string[]> | undefined;
  const available = () => (fonts ??= systemFonts().catch(() => []));
</script>

<script lang="ts">
  interface Props {
    value: string;
    onchange: (font: string) => void;
    /** Choices before the fonts, such as the system's own. */
    first?: string[];
    label?: string;
  }

  let { value, onchange, first = [], label = 'Font' }: Props = $props();

  const uid = $props.id();
  const list = `fonts-${uid}`;
  let names = $state<string[]>([]);
  $effect(() => {
    void available().then((found) => (names = found));
  });
</script>

<!-- The fonts of this computer are offered as it is typed in, and any name at
     all is taken: a font installed later, or kept under another name. -->
<input
  class="font-name"
  aria-label={label}
  {list}
  placeholder="Font name"
  {value}
  style:font-family={first.includes(value) ? undefined : `"${value}"`}
  onfocus={(e) => e.currentTarget.select()}
  onchange={(e) => {
    const name = e.currentTarget.value.trim();
    if (name) onchange(name);
    else e.currentTarget.value = value;
  }}
/>
<datalist id={list}>
  {#each [...first, ...names] as name (name)}
    <option value={name}></option>
  {/each}
</datalist>

<style>
  input {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
  }
</style>
