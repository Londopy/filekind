// Shapes that cross the Tauri IPC boundary.
//
// Do not maintain parallel TypeScript structs. The types below that mirror Rust
// definitions are GENERATED — run
//
//     cargo test -p filekind-core --features typescript
//
// which writes `src/lib/bindings/*.ts` from the `#[derive(TS)]` annotations on
// the real definitions. Import from there; this file only declares the handful
// of shapes that exist solely for the UI and have no Rust counterpart.

import type { Spec } from './bindings/Spec';
import type { Diagnostic } from './bindings/Diagnostic';

export type { Spec, Diagnostic };
export type { Format } from './bindings/Format';
export type { Association } from './bindings/Association';
export type { Targets } from './bindings/Targets';
export type { WindowsOpts } from './bindings/WindowsOpts';
export type { LinuxOpts } from './bindings/LinuxOpts';
export type { MacosOpts } from './bindings/MacosOpts';
export type { SchemaOpts } from './bindings/SchemaOpts';
export type { Container } from './bindings/Container';
export type { PerceivedType } from './bindings/PerceivedType';

/** Mirrors `SpecFile` in src-tauri/src/lib.rs. */
export interface SpecFile {
  path: string | null;
  text: string;
  spec: Spec;
  diagnostics: Diagnostic[];
}

/** Mirrors `ArtifactView`. Binary artifacts carry `base64` instead of `text`. */
export interface ArtifactView {
  path: string;
  platform: 'windows' | 'linux' | 'macos' | 'universal';
  kind: string;
  description: string;
  size: number;
  text: string | null;
  base64: string | null;
  executable: boolean;
}

/** Mirrors `BuildOptions`. */
export interface BuildOptions {
  system: boolean;
  packaging: boolean;
  stubApp: boolean;
  baseDir: string | null;
}

/** Mirrors `IconPreview`. */
export interface IconPreview {
  source_width: number;
  source_height: number;
  warnings: string[];
  previews: [number, string][];
  ico_bytes: number;
  icns_bytes: number;
}

/** Mirrors `BuildResult`. */
export interface BuildResult {
  output_dir: string;
  files: string[];
  total_bytes: number;
}

/** Mirrors `derived_values`. */
export interface Derived {
  progid: string;
  uti: string;
  utiConformsTo: string[];
  mimeIconName: string;
  desktopId: string;
  slug: string;
  dottedExtension: string;
  magicHex: string | null;
}

/** Mirrors `CommandError`. */
export interface CommandError {
  message: string;
  diagnostics?: Diagnostic[];
}

export type Screen = 'identity' | 'icon' | 'association' | 'build';
