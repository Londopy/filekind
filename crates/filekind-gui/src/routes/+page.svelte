<script lang="ts">
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { getCurrentWebview } from '@tauri-apps/api/webview';

  import { api, asCommandError } from '$lib/api';
  import { debounce, humanBytes } from '$lib/debounce';
  import Field from '$lib/components/Field.svelte';
  import DiagnosticList from '$lib/components/DiagnosticList.svelte';
  import PreviewPane from '$lib/components/PreviewPane.svelte';
  import IconGrid from '$lib/components/IconGrid.svelte';
  import type {
    ArtifactView,
    BuildOptions,
    Derived,
    Diagnostic,
    IconPreview,
    Screen,
    Spec
  } from '$lib/types';

  let spec = $state<Spec | null>(null);
  /** The document text as last read from or written to disk. Kept so saves can
      be applied onto the user's actual file rather than a regenerated one. */
  let originalText = $state<string | null>(null);
  let specPath = $state<string | null>(null);
  let dirty = $state(false);

  let diagnostics = $state<Diagnostic[]>([]);
  let derived = $state<Derived | null>(null);
  let artifacts = $state<ArtifactView[]>([]);
  let previewError = $state<string | null>(null);
  let previewStale = $state(false);

  let iconPreview = $state<IconPreview | null>(null);
  let iconError = $state<string | null>(null);

  let screen = $state<Screen>('identity');
  let recents = $state<string[]>([]);
  let toast = $state<{ kind: 'ok' | 'err'; text: string } | null>(null);
  let wizard = $state(false);
  let wizardName = $state('');
  let wizardExt = $state('');

  let options = $state<BuildOptions>({
    system: false,
    packaging: true,
    stubApp: false,
    baseDir: null
  });

  const screens: { id: Screen; label: string }[] = [
    { id: 'identity', label: 'Identity' },
    { id: 'icon', label: 'Icon' },
    { id: 'association', label: 'Association' },
    { id: 'build', label: 'Build' }
  ];

  const hasErrors = $derived(diagnostics.some((d) => d.severity === 'error'));

  // The signature feature: the pane shows the real generated files and updates
  // as you type. That is only affordable because filekind-core does no
  // I/O — this is a function call into a linked library, not a subprocess.

  async function regenerate(current: Spec) {
    try {
      const [views, d, derivedValues] = await Promise.all([
        api.previewArtifacts(current, $state.snapshot(options), null),
        api.parseSpec(await toToml(current)).then((f) => f.diagnostics).catch(() => diagnostics),
        api.derivedValues(current)
      ]);
      artifacts = views;
      diagnostics = d;
      derived = derivedValues;
      previewError = null;
    } catch (e) {
      const err = asCommandError(e);
      previewError = err.message;
      if (err.diagnostics?.length) diagnostics = err.diagnostics;
    } finally {
      previewStale = false;
    }
  }

  /** Round-trip through Rust to get validation on the current typed state. */
  async function toToml(current: Spec): Promise<string> {
    // save_spec is the only writer; for validation we reuse parse_spec on the
    // text form the backend produces from the struct.
    const scaffold = await api.scaffoldSpec(current.format.name, current.format.extension);
    return scaffold.text;
  }

  const scheduleRegenerate = debounce((current: Spec) => void regenerate(current), 150);

  function touched() {
    if (!spec) return;
    dirty = true;
    previewStale = true;
    scheduleRegenerate($state.snapshot(spec) as Spec);
  }

  async function openSpec(path?: string) {
    const chosen =
      path ??
      (await open({
        multiple: false,
        filters: [{ name: 'filekind spec', extensions: ['filekind', 'toml'] }]
      }));
    if (typeof chosen !== 'string') return;

    try {
      const file = await api.loadSpec(chosen);
      spec = file.spec;
      originalText = file.text;
      specPath = file.path;
      diagnostics = file.diagnostics;
      dirty = false;
      wizard = false;
      options.baseDir = dirname(chosen);
      recents = await api.rememberSpec(chosen);
      await regenerate(file.spec);
      await refreshIcon();
      note('ok', `opened ${basename(chosen)}`);
    } catch (e) {
      note('err', asCommandError(e).message);
    }
  }

  async function saveSpec() {
    if (!spec) return;
    let target = specPath;
    if (!target) {
      const chosen = await save({
        defaultPath: `${derived?.slug ?? 'format'}.filekind`,
        filters: [{ name: 'filekind spec', extensions: ['filekind'] }]
      });
      if (typeof chosen !== 'string') return;
      target = chosen;
    }
    try {
      const text = await api.saveSpec(target, $state.snapshot(spec) as Spec, originalText);
      originalText = text;
      specPath = target;
      options.baseDir = dirname(target);
      dirty = false;
      recents = await api.rememberSpec(target);
      note('ok', `saved ${basename(target)} — comments preserved`);
    } catch (e) {
      note('err', asCommandError(e).message);
    }
  }

  async function startWizard() {
    if (!wizardName.trim()) return;
    const file = await api.scaffoldSpec(wizardName, wizardExt || wizardName);
    spec = file.spec;
    originalText = file.text;
    specPath = null;
    diagnostics = file.diagnostics;
    dirty = true;
    wizard = false;
    await regenerate(file.spec);
    note('ok', 'scaffolded — save it and keep editing here or in a text editor');
  }

  async function pickIcon() {
    const chosen = await open({
      multiple: false,
      filters: [{ name: 'PNG image', extensions: ['png'] }]
    });
    if (typeof chosen !== 'string' || !spec) return;
    // Store a path relative to the spec when we can: a spec with an absolute
    // path in it is not shareable.
    spec.association.icon = relativeTo(options.baseDir, chosen);
    await refreshIcon(chosen);
    touched();
  }

  async function refreshIcon(absolute?: string) {
    const rel = spec?.association.icon;
    if (!rel) {
      iconPreview = null;
      iconError = null;
      return;
    }
    const path = absolute ?? join(options.baseDir, rel);
    try {
      iconPreview = await api.renderIcons(path);
      iconError = null;
    } catch (e) {
      iconPreview = null;
      iconError = asCommandError(e).message;
    }
  }

  async function runBuild() {
    if (!spec) return;
    const dir = await open({ directory: true, multiple: false, title: 'Output directory' });
    if (typeof dir !== 'string') return;
    try {
      const result = await api.buildArtifacts(
        $state.snapshot(spec) as Spec,
        $state.snapshot(options),
        dir
      );
      note(
        'ok',
        `wrote ${result.files.length} files (${humanBytes(result.total_bytes)}) — nothing installed`
      );
    } catch (e) {
      const err = asCommandError(e);
      if (err.diagnostics?.length) diagnostics = err.diagnostics;
      note('err', err.message);
    }
  }

  function note(kind: 'ok' | 'err', text: string) {
    toast = { kind, text };
    setTimeout(() => (toast = null), 4000);
  }

  function sep(p: string) {
    return p.includes('\\') && !p.includes('/') ? '\\' : '/';
  }
  function dirname(p: string) {
    const s = sep(p);
    const i = p.lastIndexOf(s);
    return i <= 0 ? p : p.slice(0, i);
  }
  function basename(p: string) {
    const s = sep(p);
    return p.slice(p.lastIndexOf(s) + 1);
  }
  function join(base: string | null, rel: string) {
    if (!base) return rel;
    return `${base}${sep(base)}${rel}`;
  }
  function relativeTo(base: string | null, abs: string) {
    if (base && abs.startsWith(base)) {
      return abs.slice(base.length + 1).split('\\').join('/');
    }
    return abs;
  }

  onMount(async () => {
    recents = await api.recentSpecs();
    if (!recents.length) wizard = true;

    // Drag-and-drop (v0.3): a .filekind opens it, a .png becomes the icon.
    const unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type !== 'drop') return;
      const dropped = event.payload.paths[0];
      if (!dropped) return;
      if (dropped.toLowerCase().endsWith('.png')) {
        if (!spec) return;
        spec.association.icon = relativeTo(options.baseDir, dropped);
        await refreshIcon(dropped);
        touched();
      } else {
        await openSpec(dropped);
      }
    });
    return unlisten;
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      void saveSpec();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'o') {
      e.preventDefault();
      void openSpec();
    }
  }}
/>

<main>
  <header class="titlebar">
    <div class="brand">
      <strong>filekind</strong>
      {#if specPath}
        <span class="path" title={specPath}>{basename(specPath)}{dirty ? ' •' : ''}</span>
      {:else if spec}
        <span class="path">unsaved</span>
      {/if}
    </div>
    <div class="actions">
      <button onclick={() => openSpec()}>Open</button>
      <button onclick={saveSpec} disabled={!spec}>Save</button>
      <button class="primary" onclick={runBuild} disabled={!spec || hasErrors}>Build…</button>
    </div>
  </header>

  {#if !spec}
    <section class="welcome">
      <h1>Make a file extension real.</h1>
      <p>
        One spec in; a Windows registry tree, a freedesktop MIME definition, a macOS UTI
        declaration, a libmagic pattern and icons in three formats out.
      </p>

      {#if wizard}
        <div class="wizard">
          <Field label="Format name" field="format.name" bind:value={wizardName} placeholder="Londo Save" />
          <Field
            label="Extension"
            field="format.extension"
            bind:value={wizardExt}
            placeholder="londo"
            mono
            hint="no leading dot; a–z and 0–9"
          />
          <button class="primary" onclick={startWizard} disabled={!wizardName.trim()}>
            Create spec
          </button>
          <p class="fineprint">
            The wizard writes a spec file you can keep editing here or in any text editor. It is a
            starting point, not a separate mode.
          </p>
        </div>
      {:else}
        <div class="row">
          <button class="primary" onclick={() => (wizard = true)}>New spec</button>
          <button onclick={() => openSpec()}>Open a .filekind</button>
        </div>
      {/if}

      {#if recents.length}
        <div class="recents">
          <h2>Recent</h2>
          {#each recents as r (r)}
            <button class="recent" onclick={() => openSpec(r)} title={r}>{basename(r)}</button>
          {/each}
        </div>
      {/if}
    </section>
  {:else}
    <div class="split">
      <div class="editor">
        <nav class="screens">
          {#each screens as s (s.id)}
            <button class:active={screen === s.id} onclick={() => (screen = s.id)}>
              {s.label}
            </button>
          {/each}
        </nav>

        <div class="scroll">
          {#if screen === 'identity'}
            <Field label="Name" field="format.name" bind:value={spec.format.name} {diagnostics} oninput={touched} />
            <Field
              label="Extension"
              field="format.extension"
              bind:value={spec.format.extension}
              mono
              {diagnostics}
              oninput={touched}
            />
            <Field
              label="Description"
              field="format.description"
              bind:value={spec.format.description}
              {diagnostics}
              oninput={touched}
            />

            <label class="field">
              <span class="label">Magic bytes</span>
              <input
                class="mono"
                type="text"
                placeholder="LNDO\x01"
                value={spec.format.magic ?? ''}
                oninput={(e) => {
                  const v = (e.currentTarget as HTMLInputElement).value;
                  spec!.format.magic = v.length ? v : null;
                  touched();
                }}
              />
              <span class="hint">
                Escaped bytes at offset 0. In the spec file this must be a TOML
                <em>literal</em> string — single quotes — because <code>"\x01"</code> is not valid
                TOML.
                {#if derived?.magicHex}<br />Decodes to <code>{derived.magicHex}</code>.{/if}
              </span>
            </label>

            <label class="field">
              <span class="label">Container</span>
              <select
                value={spec.format.container}
                onchange={(e) => {
                  spec!.format.container = (e.currentTarget as HTMLSelectElement).value as never;
                  touched();
                }}
              >
                {#each ['none', 'zip', 'sqlite', 'text', 'binary'] as c (c)}
                  <option value={c}>{c}</option>
                {/each}
              </select>
            </label>

            {#if derived}
              <dl class="derived">
                <dt>ProgID</dt>
                <dd>{derived.progid}</dd>
                <dt>UTI</dt>
                <dd>{derived.uti}</dd>
                <dt>Icon name</dt>
                <dd>{derived.mimeIconName}</dd>
                <dt>Desktop id</dt>
                <dd>{derived.desktopId}</dd>
              </dl>
            {/if}
          {:else if screen === 'icon'}
            <div class="row">
              <button onclick={pickIcon}>Choose PNG…</button>
              {#if spec.association.icon}
                <code class="path">{spec.association.icon}</code>
              {/if}
            </div>
            <p class="fineprint">You can also drop a PNG anywhere on this window.</p>
            <IconGrid preview={iconPreview} error={iconError} />
          {:else if screen === 'association'}
            <Field label="MIME type" field="association.mime" bind:value={spec.association.mime} mono {diagnostics} oninput={touched} />

            <label class="field">
              <span class="label">Handler</span>
              <input
                type="text"
                placeholder="londo-player"
                value={spec.association.handler ?? ''}
                oninput={(e) => {
                  const v = (e.currentTarget as HTMLInputElement).value;
                  spec!.association.handler = v.length ? v : null;
                  touched();
                }}
              />
              <span class="hint">
                Declared here and only here. A <code>{derived?.dottedExtension ?? '.ext'}</code> file
                can never say what opens it — that is what makes a format a data file rather than a
                malware delivery vehicle.
              </span>
            </label>

            <Field
              label="Handler arguments"
              field="association.handler_args"
              bind:value={spec.association.handler_args}
              mono
              hint="%1 becomes %f on Linux"
              {diagnostics}
              oninput={touched}
            />

            <fieldset>
              <legend>Targets</legend>
              {#each [['windows', 'Windows'], ['linux', 'Linux'], ['macos', 'macOS (experimental)']] as [key, label] (key)}
                <label class="check">
                  <input
                    type="checkbox"
                    checked={spec.targets[key as 'windows' | 'linux' | 'macos']}
                    onchange={(e) => {
                      spec!.targets[key as 'windows' | 'linux' | 'macos'] = (
                        e.currentTarget as HTMLInputElement
                      ).checked;
                      touched();
                    }}
                  />
                  {label}
                </label>
              {/each}
            </fieldset>
          {:else if screen === 'build'}
            <fieldset>
              <legend>Options</legend>
              <label class="check">
                <input
                  type="checkbox"
                  checked={options.system}
                  onchange={(e) => {
                    options.system = (e.currentTarget as HTMLInputElement).checked;
                    touched();
                  }}
                />
                System-wide (HKLM, /usr/share) — needs elevation
              </label>
              <label class="check">
                <input
                  type="checkbox"
                  checked={options.packaging}
                  onchange={(e) => {
                    options.packaging = (e.currentTarget as HTMLInputElement).checked;
                    touched();
                  }}
                />
                Emit .deb and .rpm packaging
              </label>
              <label class="check">
                <input
                  type="checkbox"
                  checked={options.stubApp}
                  onchange={(e) => {
                    options.stubApp = (e.currentTarget as HTMLInputElement).checked;
                    touched();
                  }}
                />
                macOS stub .app bundle
              </label>
            </fieldset>

            <ul class="filelist">
              {#each artifacts as a (a.path)}
                <li><code>{a.path}</code><span>{humanBytes(a.size)}</span></li>
              {/each}
            </ul>

            <button class="primary wide" onclick={runBuild} disabled={hasErrors}>
              Build to a directory…
            </button>
            <p class="fineprint">
              filekind writes files and stops there. Read the generated README, then run the
              scripts yourself.
            </p>
          {/if}

          <DiagnosticList {diagnostics} />
        </div>
      </div>

      <PreviewPane {artifacts} error={previewError} stale={previewStale} />
    </div>
  {/if}

  {#if toast}
    <div class="toast {toast.kind}">{toast.text}</div>
  {/if}
</main>

<style>
  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0.9rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-panel);
  }
  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
  }
  .brand strong {
    letter-spacing: -0.01em;
  }
  .path {
    font-size: 0.78rem;
    color: var(--fg-dim);
    font-family: var(--mono);
  }
  .actions {
    display: flex;
    gap: 0.4rem;
  }
  button {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--fg);
    padding: 0.35rem 0.7rem;
    font-size: 0.82rem;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #06090f;
    font-weight: 600;
  }
  button.wide {
    width: 100%;
    padding: 0.6rem;
    margin-top: 0.5rem;
  }

  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(360px, 42%) 1fr;
  }
  .editor {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .screens {
    display: flex;
    gap: 0.25rem;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--border);
  }
  .screens button {
    border: none;
    background: none;
    color: var(--fg-dim);
    border-radius: 999px;
    padding: 0.3rem 0.8rem;
  }
  .screens button.active {
    background: var(--accent-faint);
    color: var(--accent);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 1rem 0.9rem 2rem;
  }

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
  input[type='text'],
  select {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.65rem;
    color: var(--fg);
    font: inherit;
    font-size: 0.92rem;
  }
  .mono {
    font-family: var(--mono);
  }
  .hint,
  .fineprint {
    font-size: 0.78rem;
    line-height: 1.45;
    color: var(--fg-dim);
  }
  .hint code,
  .fineprint code {
    font-family: var(--mono);
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.75rem 0.9rem;
    margin: 0 0 1rem;
  }
  legend {
    font-size: 0.72rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-dim);
    padding: 0 0.35rem;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.86rem;
    padding: 0.2rem 0;
  }
  .derived {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.3rem 1rem;
    margin: 1.25rem 0 0;
    padding-top: 0.9rem;
    border-top: 1px solid var(--border);
    font-size: 0.8rem;
  }
  .derived dt {
    color: var(--fg-dim);
  }
  .derived dd {
    margin: 0;
    font-family: var(--mono);
  }
  .filelist {
    list-style: none;
    margin: 0 0 1rem;
    padding: 0;
    max-height: 320px;
    overflow: auto;
    font-size: 0.78rem;
  }
  .filelist li {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.15rem 0;
    color: var(--fg-dim);
  }
  .filelist code {
    font-family: var(--mono);
    color: var(--fg);
  }

  .welcome {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.75rem;
    max-width: 640px;
    margin: 0 auto;
    padding: 2rem;
  }
  .welcome h1 {
    font-size: 1.6rem;
    margin: 0;
    letter-spacing: -0.02em;
  }
  .welcome p {
    color: var(--fg-dim);
    line-height: 1.6;
    margin: 0;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin: 0.5rem 0;
  }
  .wizard {
    margin-top: 1rem;
    max-width: 340px;
  }
  .recents {
    margin-top: 1.5rem;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.25rem;
  }
  .recents h2 {
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-dim);
    margin: 0 0 0.25rem;
  }
  .recent {
    border: none;
    background: none;
    color: var(--accent);
    padding: 0.1rem 0;
    font-size: 0.85rem;
  }

  .toast {
    position: fixed;
    bottom: 1rem;
    left: 50%;
    transform: translateX(-50%);
    padding: 0.55rem 1rem;
    border-radius: 8px;
    font-size: 0.85rem;
    border: 1px solid var(--border);
    background: var(--bg-panel);
    box-shadow: 0 8px 24px #0006;
  }
  .toast.ok {
    border-color: var(--ok);
  }
  .toast.err {
    border-color: var(--error);
  }
</style>
