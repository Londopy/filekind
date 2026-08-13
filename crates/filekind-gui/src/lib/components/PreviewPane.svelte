<script lang="ts">
  import type { ArtifactView } from '../types';
  import { humanBytes } from '../debounce';

  interface Props {
    artifacts: ArtifactView[];
    error: string | null;
    stale: boolean;
  }
  let { artifacts, error, stale }: Props = $props();

  let selectedPath = $state<string | null>(null);

  // Keep the selection across regenerations: the pane re-renders on every
  // keystroke, and losing your place each time would make it unusable.
  const selected = $derived(
    artifacts.find((a) => a.path === selectedPath) ?? artifacts[0] ?? null
  );

  const grouped = $derived.by(() => {
    const map = new Map<string, ArtifactView[]>();
    for (const a of artifacts) {
      const list = map.get(a.platform) ?? [];
      list.push(a);
      map.set(a.platform, list);
    }
    return [...map.entries()];
  });
</script>

<aside class="preview" class:stale>
  <header>
    <h2>Preview</h2>
    <span class="note">
      {#if error}
        <span class="err">spec does not parse</span>
      {:else}
        {artifacts.length} artifact{artifacts.length === 1 ? '' : 's'}
      {/if}
    </span>
  </header>

  {#if error}
    <pre class="error-body">{error}</pre>
  {:else}
    <nav>
      {#each grouped as [platform, list] (platform)}
        <div class="group">
          <span class="group-label">{platform}</span>
          {#each list as a (a.path)}
            <button
              class:active={selected?.path === a.path}
              onclick={() => (selectedPath = a.path)}
              title={a.description}
            >
              <span class="name">{a.path.split('/').pop()}</span>
              <span class="size">{humanBytes(a.size)}</span>
            </button>
          {/each}
        </div>
      {/each}
    </nav>

    {#if selected}
      <div class="body">
        <div class="body-head">
          <code>{selected.path}</code>
          <span>{selected.description}</span>
        </div>
        {#if selected.text !== null}
          <pre>{selected.text}</pre>
        {:else if selected.base64 !== null && selected.path.endsWith('.png')}
          <div class="image">
            <img src={'data:image/png;base64,' + selected.base64} alt={selected.path} />
            <p>{humanBytes(selected.size)} PNG</p>
          </div>
        {:else}
          <div class="image">
            <p>
              Binary artifact, {humanBytes(selected.size)}.
              {#if selected.path.endsWith('.ico')}
                A multi-resolution Windows icon; the individual sizes are on the Icon screen.
              {:else if selected.path.endsWith('.icns')}
                A macOS icon family; the individual sizes are on the Icon screen.
              {/if}
            </p>
          </div>
        {/if}
      </div>
    {/if}
  {/if}
</aside>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
    background: var(--bg-panel);
    min-width: 0;
    transition: opacity 120ms ease;
  }
  .preview.stale {
    opacity: 0.55;
  }
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0.85rem 1rem;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 0.8rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-dim);
  }
  .note {
    font-size: 0.75rem;
    color: var(--fg-dim);
  }
  .err {
    color: var(--error);
  }
  nav {
    display: flex;
    gap: 1.25rem;
    overflow-x: auto;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid var(--border);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: max-content;
  }
  .group-label {
    font-size: 0.66rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-faint);
    margin-bottom: 0.15rem;
  }
  nav button {
    display: flex;
    gap: 0.6rem;
    justify-content: space-between;
    background: none;
    border: none;
    border-radius: 4px;
    padding: 0.2rem 0.4rem;
    color: var(--fg-dim);
    font: inherit;
    font-size: 0.78rem;
    cursor: pointer;
    text-align: left;
  }
  nav button:hover {
    background: var(--bg-hover);
  }
  nav button.active {
    background: var(--accent-faint);
    color: var(--accent);
  }
  .size {
    color: var(--fg-faint);
    font-variant-numeric: tabular-nums;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .body-head {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.6rem 1rem;
    font-size: 0.75rem;
    color: var(--fg-dim);
    border-bottom: 1px solid var(--border);
  }
  .body-head code {
    font-family: var(--mono);
    color: var(--fg);
  }
  pre {
    flex: 1;
    margin: 0;
    padding: 1rem;
    overflow: auto;
    font-family: var(--mono);
    font-size: 0.78rem;
    line-height: 1.55;
    white-space: pre;
    tab-size: 4;
  }
  .error-body {
    color: var(--error);
    padding: 1rem;
    white-space: pre-wrap;
  }
  .image {
    padding: 2rem;
    text-align: center;
    color: var(--fg-dim);
    font-size: 0.8rem;
  }
  .image img {
    image-rendering: pixelated;
    max-width: 256px;
    background: repeating-conic-gradient(var(--bg-hover) 0% 25%, transparent 0% 50%) 50% / 16px 16px;
  }
</style>
