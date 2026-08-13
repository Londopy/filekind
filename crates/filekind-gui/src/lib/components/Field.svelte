<script lang="ts">
  import type { Diagnostic } from '../types';

  interface Props {
    label: string;
    field: string;
    value: string;
    placeholder?: string;
    hint?: string;
    mono?: boolean;
    diagnostics?: Diagnostic[];
    oninput?: (v: string) => void;
  }

  let {
    label,
    field,
    value = $bindable(),
    placeholder = '',
    hint = '',
    mono = false,
    diagnostics = [],
    oninput
  }: Props = $props();

  // Diagnostics are addressed to a dotted field path, which is what lets the
  // red underline land on the box that caused it rather than in a list at the
  // bottom of the screen.
  const mine = $derived(diagnostics.filter((d) => d.field === field));
  const worst = $derived(
    mine.some((d) => d.severity === 'error') ? 'error' : mine.length ? 'warning' : ''
  );
</script>

<label class="field" data-state={worst}>
  <span class="label">{label}</span>
  <input
    class:mono
    type="text"
    bind:value
    {placeholder}
    oninput={(e) => oninput?.((e.currentTarget as HTMLInputElement).value)}
  />
  {#if hint && !mine.length}
    <span class="hint">{hint}</span>
  {/if}
  {#each mine as d (d.field + d.message)}
    <span class="diag {d.severity}">
      {d.message}
      {#if d.hint}<em>{d.hint}</em>{/if}
    </span>
  {/each}
</label>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin-bottom: 1rem;
  }
  .label {
    font-size: 0.78rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-dim);
  }
  input {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.65rem;
    color: var(--fg);
    font: inherit;
    font-size: 0.92rem;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-faint);
  }
  input.mono {
    font-family: var(--mono);
  }
  [data-state='error'] input {
    border-color: var(--error);
  }
  [data-state='warning'] input {
    border-color: var(--warning);
  }
  .hint,
  .diag {
    font-size: 0.78rem;
    line-height: 1.4;
    color: var(--fg-dim);
  }
  .diag.error {
    color: var(--error);
  }
  .diag.warning {
    color: var(--warning);
  }
  .diag em {
    display: block;
    font-style: normal;
    color: var(--fg-dim);
  }
</style>
