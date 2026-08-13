// Every call into Rust, in one place.
//
// Nothing here touches the filesystem: the webview cannot read a path even if
// it wanted to. It asks Rust, which does the I/O inside an explicit command.

import { invoke } from '@tauri-apps/api/core';
import type {
  ArtifactView,
  BuildOptions,
  BuildResult,
  CommandError,
  Derived,
  IconPreview,
  Spec,
  SpecFile
} from './types';

/** Narrow an unknown thrown value into the error shape Rust sends. */
export function asCommandError(e: unknown): CommandError {
  if (typeof e === 'object' && e !== null && 'message' in e) {
    return e as CommandError;
  }
  return { message: String(e) };
}

export const api = {
  parseSpec: (text: string) => invoke<SpecFile>('parse_spec', { text }),

  loadSpec: (path: string) => invoke<SpecFile>('load_spec', { path }),

  /**
   * Save through `toml_edit`, so the user's comments and key order survive.
   * `originalText` is what we last read from disk; passing it lets Rust apply
   * the edit onto that exact document rather than re-reading a file that may
   * have changed underneath us.
   */
  saveSpec: (path: string, spec: Spec, originalText: string | null) =>
    invoke<string>('save_spec', { path, spec, originalText }),

  scaffoldSpec: (name: string, extension: string) =>
    invoke<SpecFile>('scaffold_spec', { name, extension }),

  previewArtifacts: (spec: Spec, options: BuildOptions, platform: string | null) =>
    invoke<ArtifactView[]>('preview_artifacts', { spec, options, platform }),

  buildArtifacts: (spec: Spec, options: BuildOptions, outputDir: string) =>
    invoke<BuildResult>('build_artifacts', { spec, options, outputDir }),

  renderIcons: (pngPath: string) => invoke<IconPreview>('render_icons', { pngPath }),

  derivedValues: (spec: Spec) => invoke<Derived>('derived_values', { spec }),

  recentSpecs: () => invoke<string[]>('recent_specs'),

  rememberSpec: (path: string) => invoke<string[]>('remember_spec', { path })
};
