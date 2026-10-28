<script lang="ts">
  import { fixed } from './traces';
  import { integralTotal, peakFitSummary } from './quantification';
  import { Ellipsis, X } from '@lucide/svelte';
  import {
    normalizeIntegral,
    removeIntegral,
    removeMultiplet,
    removePeak,
    setMultipletName,
    setMultipletNuclei,
    setPeakLabel,
  } from './analysis';
  import type { Analysis, DocumentInfo } from './api';
  import { suggestName } from './impurities';
  import { letterOf, nucleiOf, report } from './multiplets';
  import { findSolvent } from './solvents';

  let {
    doc,
    part,
    decimals,
    onedit,
    oncopy,
    onchange,
  }: {
    doc: DocumentInfo;
    /** Which table to show. The ribbon tab decides, so the panel has no switcher
     * of its own and the tools for each live in their own tab. */
    part: 'peaks' | 'integrals' | 'multiplets';
    /** Opens everything about multiplet `index`, where it can be said by hand. */
    onedit: (index: number) => void;
    /** Takes the written-out spectrum away, to be pasted where it is wanted. */
    oncopy: () => void;
    /** Decimal places for a shift, shared with the plot. */
    decimals: number;
    onchange: (analysis: Analysis) => void;
  } = $props();

  /** The solvent's own table of impurities, when the dataset names one we know. */
  const solvent = $derived(findSolvent(doc.solvent, doc.nucleus));
  const total = $derived(integralTotal(doc.integralValues));

  /** Scales the normalisation so every integral together reads `wanted`, which is
      how a total is set to the protons in the molecule. */
  function setTotal(wanted: number) {
    const first = doc.integralValues[0];
    if (total === null || !(total > 0) || !(wanted > 0) || first == null) return;
    onchange(normalizeIntegral(doc.analysis, 0, (first * wanted) / total));
  }
</script>

{#if (part === 'peaks' && (doc.analysis.peakFit || doc.analysis.peakMethod === 'deconvolution')) || (part === 'integrals' && doc.analysis.integrationMethod === 'fitted')}
  <p class="hint" role="status">{peakFitSummary(doc.analysis.peakFit)}</p>
{/if}

{#if part === 'integrals'}
  <section>
    {#if doc.analysis.integrals.length === 0}
      <p class="hint">Press I and drag over a signal, or use Auto integrals.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th class="range" colspan="3">Range (ppm)</th>
            <th class="num">Value</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each doc.analysis.integrals as [from, to], i (i)}
            <tr>
              <td class="num">{fixed(from, decimals)}</td>
              <td class="dash">–</td>
              <td class="num">{fixed(to, decimals)}</td>
              <td class="num">
                {#if doc.integralValues[i] == null}
                  <span title="The selected area method could not quantify this range"
                    >Unavailable</span
                  >
                {:else}
                  <input
                    type="number"
                    step="any"
                    class="quiet-number"
                    title="Type a value to normalise all integrals to this one"
                    value={doc.integralValues[i]?.toFixed(2)}
                    onchange={(e) =>
                      onchange(normalizeIntegral(doc.analysis, i, Number(e.currentTarget.value)))}
                  />
                {/if}
              </td>
              <td>
                <button
                  class="icon"
                  title="Remove this integral"
                  aria-label="Remove integral {i + 1}"
                  onclick={() => onchange(removeIntegral(doc.analysis, i))}><X size={14} /></button
                >
              </td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <td colspan="3">Total</td>
            <td class="num">
              {#if total === null}
                <span title="A total requires an available value for every integral"
                  >Unavailable</span
                >
              {:else}
                <input
                  type="number"
                  step="any"
                  class="quiet-number"
                  title="Type what every integral should add up to, such as the protons in the molecule"
                  aria-label="Total of every integral"
                  value={total.toFixed(2)}
                  onchange={(e) => setTotal(Number(e.currentTarget.value))}
                />
              {/if}
            </td>
            <td></td>
          </tr>
        </tfoot>
      </table>
    {/if}
  </section>
{:else if part === 'multiplets'}
  <section>
    {#if doc.analysis.multiplets.length === 0}
      <p class="hint">Press J and drag over a signal, or use Auto multiplets.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th class="who">Name</th>
            <th class="num">δ</th>
            <th class="kind">Class</th>
            <th class="num couplings">J (Hz)</th>
            <th class="num">H</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each doc.analysis.multiplets as m, i (i)}
            {@const info = doc.multipletInfo[i]}
            <tr>
              <td>
                <input
                  class="quiet-field"
                  title="What this multiplet is called in the report"
                  aria-label="Name of multiplet {i + 1}"
                  value={m.name ?? letterOf(i)}
                  onchange={(e) =>
                    onchange(setMultipletName(doc.analysis, i, e.currentTarget.value.trim()))}
                />
              </td>
              <td class="num">{fixed(info?.ppm ?? 0, decimals)}</td>
              <td class="kind">{info?.kind ?? ''}</td>
              <td class="num couplings">
                {(info?.jsHz ?? []).map((j) => j.toFixed(1)).join(', ') || '–'}
              </td>
              <td class="num">
                <input
                  type="number"
                  min="0"
                  step="1"
                  title="Nuclei this multiplet stands for; 0 follows its integral again"
                  aria-label="Nuclei of multiplet {i + 1}"
                  class="nuclei quiet-number"
                  value={nucleiOf(m, info) ?? undefined}
                  placeholder="Unavailable"
                  onchange={(e) =>
                    onchange(setMultipletNuclei(doc.analysis, i, Number(e.currentTarget.value)))}
                />
              </td>
              <td class="ends">
                <button
                  class="icon"
                  title="Everything about this multiplet"
                  aria-label="Everything about multiplet {i + 1}"
                  onclick={() => onedit(i)}><Ellipsis size={14} /></button
                >
                <button
                  class="icon"
                  title="Remove this multiplet"
                  aria-label="Remove multiplet {i + 1}"
                  onclick={() => onchange(removeMultiplet(doc.analysis, i))}><X size={14} /></button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <!-- What the table is for: the line a paper quotes, kept where it is read
           rather than only behind a button. -->
      <div class="report">
        <p class="quote">{report(doc, decimals)}</p>
        <button title="Copy it, to paste where it is wanted" onclick={oncopy}>Copy</button>
      </div>
    {/if}
  </section>
{:else}
  <section>
    {#if doc.analysis.peaks.length === 0}
      <p class="hint">Press K and drag over a region, or use Auto peaks.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th class="num">δ (ppm)</th>
            <th class="num">δ (Hz)</th>
            <th class="num">Width (Hz)</th>
            <th>Label</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each doc.analysis.peaks as ppm, i (i)}
            <tr>
              <td class="num">{fixed(ppm, decimals)}</td>
              <!-- In Hz as well, since a coupling is the difference between two
                   of them and that is the sum a chemist does with this table. -->
              <td class="num">{(ppm * doc.frequencyMhz).toFixed(1)}</td>
              <!-- How wide the line is: a broad one is exchange or a bad shim, and
                   a narrow one beside it is a different thing. Nothing when it
                   could not be measured, rather than a zero that reads as a width. -->
              <td class="num"
                >{(doc.peakWidths[i] ?? 0) > 0 ? (doc.peakWidths[i] ?? 0).toFixed(1) : '–'}</td
              >
              <td>
                <input
                  class="quiet-field"
                  title="What to call this peak; empty shows nothing"
                  aria-label="Label for the peak at {ppm.toFixed(2)} ppm"
                  placeholder={(doc.analysis.labels[i] ?? null) === null
                    ? (suggestName(ppm, solvent) ?? '')
                    : ''}
                  value={doc.analysis.labels[i] ?? suggestName(ppm, solvent) ?? ''}
                  onchange={(e) => {
                    const text = e.currentTarget.value.trim();
                    onchange(setPeakLabel(doc.analysis, i, text));
                  }}
                />
              </td>
              <td>
                <button
                  class="icon"
                  title="Remove this peak"
                  aria-label="Remove peak at {ppm.toFixed(2)} ppm"
                  onclick={() => onchange(removePeak(doc.analysis, i))}><X size={14} /></button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/if}

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-variant-numeric: tabular-nums;
  }

  /* A heading names its column; it does not have to sit over the last digit of
     the numbers beneath it. */
  th {
    padding-bottom: var(--space-1);
    text-align: center;
    font-weight: normal;
    color: var(--muted);
  }

  th,
  td {
    padding-right: var(--space-5);
  }

  th:last-child,
  td:last-child {
    padding-right: 0;
  }

  /* Shifts line up on their last digit, so 10.76 reads against 9.21, and the
     columns take only the room their digits need rather than spreading. */
  .num {
    width: 1%;
    white-space: nowrap;
  }

  td.num {
    text-align: right;
  }

  .range {
    text-align: center;
  }

  .dash {
    padding: 0 var(--space-2);
    color: var(--muted);
  }

  /* Each column its own room: a class of one letter left its heading hanging
     over the shift beside it, and an empty J column left that heading over
     nothing at all. */
  .who {
    width: 3.5rem;
  }

  .kind {
    width: 2.5rem;
  }

  td.kind {
    text-align: center;
  }

  .couplings {
    min-width: 4.5rem;
  }

  .ends {
    white-space: nowrap;
  }

  td input {
    width: 4.5rem;
    text-align: right;
  }

  /* A name is text and reads from the left; only figures line up on the right. */
  td input.quiet-field {
    text-align: left;
  }

  /* A row's own buttons are there on the row being pointed at, as a step's are. */
  tbody tr .icon {
    opacity: 0;
  }

  tbody tr:hover .icon,
  tbody tr:focus-within .icon {
    opacity: 1;
  }

  .nuclei {
    width: 3rem;
  }

  .report {
    display: flex;
    gap: var(--space-4);
    align-items: flex-start;
  }

  .report .quote {
    flex: 1;
    line-height: 1.5;
  }

  tfoot td {
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
    color: var(--muted);
  }
</style>
