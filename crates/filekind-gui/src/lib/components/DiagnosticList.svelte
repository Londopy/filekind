<script lang="ts">
  import type { Diagnostic } from '../types';

  interface Props {
    diagnostics: Diagnostic[];
  }
  let { diagnostics }: Props = $props();

  const errors = $derived(diagnostics.filter((d) => d.severity === 'error').length);
  const warnings = $derived(diagnostics.length - errors);
</script>

{#if diagnostics.length}
  <section class="diags">
    <header>
      {#if errors}<span class="pill error">{errors} error{errors === 1 ? '' : 's'}</span>{/if}
      {#if warnings}
        <span class="pill warning">{warnings} warning{warnings === 1 ? '' : 's'}</span>
      {/if}
    </header>
    <ul>
      {#each diagnostics as d (d.field + d.message)}
        <li class={d.severity}>
          <code>{d.field}</code>
          <span>{d.message}</span>
          {#if d.hint}<em>{d.hint}</em>{/if}
        </li>
      {/each}
    </ul>
  </section>
{:else}
  <p class="clean">No problems found.</p>
{/if}

<style>
  .diags {
    border-top: 1px solid var(--border);
    padding-top: 0.75rem;
  }
  header {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  .pill {
    font-size: 0.72rem;
    padding: 0.15rem 0.5rem;
    border-radius: 999px;
    font-weight: 600;
  }
  .pill.error {
    background: var(--error-faint);
    color: var(--error);
  }
  .pill.warning {
    background: var(--warning-faint);
    color: var(--warning);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  li {
    font-size: 0.82rem;
    line-height: 1.45;
    border-left: 2px solid var(--border);
    padding-left: 0.6rem;
  }
  li.error {
    border-left-color: var(--error);
  }
  li.warning {
    border-left-color: var(--warning);
  }
  code {
    font-family: var(--mono);
    color: var(--fg-dim);
    margin-right: 0.4rem;
  }
  em {
    display: block;
    font-style: normal;
    color: var(--fg-dim);
  }
  .clean {
    font-size: 0.82rem;
    color: var(--ok);
  }
</style>
