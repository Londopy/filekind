<script lang="ts">
  import type { IconPreview } from '../types';
  import { humanBytes } from '../debounce';

  interface Props {
    preview: IconPreview | null;
    error: string | null;
  }
  let { preview, error }: Props = $props();
</script>

{#if error}
  <p class="error">{error}</p>
{:else if preview}
  <div class="summary">
    <span>source {preview.source_width}×{preview.source_height}</span>
    <span>.ico {humanBytes(preview.ico_bytes)}</span>
    <span>.icns {humanBytes(preview.icns_bytes)}</span>
  </div>

  {#each preview.warnings as w (w)}
    <p class="warn">{w}</p>
  {/each}

  <div class="grid">
    {#each preview.previews as [size, b64] (size)}
      <figure>
        <img src={'data:image/png;base64,' + b64} alt="{size} pixel icon" width={size} height={size} />
        <figcaption>{size}px</figcaption>
      </figure>
    {/each}
  </div>
{:else}
  <p class="empty">Pick a square PNG, 512×512 or larger.</p>
{/if}

<style>
  .summary {
    display: flex;
    gap: 1rem;
    font-size: 0.78rem;
    color: var(--fg-dim);
    margin-bottom: 0.75rem;
  }
  .grid {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 1.25rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: repeating-conic-gradient(var(--bg-hover) 0% 25%, transparent 0% 50%) 50% / 16px 16px;
  }
  figure {
    margin: 0;
    text-align: center;
  }
  img {
    image-rendering: pixelated;
    display: block;
  }
  figcaption {
    margin-top: 0.35rem;
    font-size: 0.68rem;
    color: var(--fg-dim);
  }
  .warn {
    font-size: 0.8rem;
    color: var(--warning);
    margin: 0 0 0.5rem;
  }
  .error {
    color: var(--error);
    font-size: 0.85rem;
  }
  .empty {
    color: var(--fg-dim);
    font-size: 0.85rem;
  }
</style>
