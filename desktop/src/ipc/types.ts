// Wire types shared with the Rust host (crates/engine, desktop/src-tauri). Hand-written in M0; from M1 they are
// generated from the registry's JSON Schemas so the two sides cannot drift.

/** Error envelope of every failed host call: stable dotted code (11 §10), message, structured details. */
export interface ErrorInfo {
  code: string;
  message: string;
  details?: unknown;
}

export interface PingRequest {
  seq: number;
  payload?: string;
}

export interface PingResponse {
  seq: number;
  payload: string | null;
  engineUptimeNs: number;
  handlerNs: number;
}

export type RunMode = 'attached' | 'headless' | 'batchScript';

/**
 * Internal engine bootstrap information (crates/engine `EngineInfo`). Not the spec query `app.info.get` (01 §12),
 * which adds `locale` and `languages[]` and arrives as a registered query in C1.4.
 */
export interface EngineInfo {
  name: string;
  version: string;
  platform: string;
  arch: string;
  formatVersion: string;
  registryVersion: string;
  runMode: RunMode;
}

/** A main-menu command the host forwarded to the front end (`desktop/src/ui-bindings/menu.json`). */
export interface MenuCommandEvent {
  command: string;
  params: Record<string, unknown>;
  handler: 'frontend' | 'pending';
  milestone: string | null;
}

/** M0 diagnostics: what `toontown --ipc-bench` asks the front end to measure. */
export interface BenchPlan {
  count: number;
  payloadBytes: number[];
}

// ---- M0 spike: C0.2 tablet input (desktop/src-tauri/src/spikes/tablet). Not product API. ----

export interface SpikeMode {
  name: 'tablet';
  logPath: string | null;
  nativeBackend: 'octotablet-ink' | 'appkit-nsevent' | null;
  penSimulation: boolean;
  auto: boolean;
}

export interface NativePacket {
  kind: 'pose' | 'down' | 'up' | 'in' | 'out' | 'frame' | 'proximity';
  hostEpochMs: number;
  deviceMs: number | null;
  x: number;
  y: number;
  pressure: number | null;
  tiltX: number | null;
  tiltY: number | null;
  twist: number | null;
  tool: string;
}

export interface NativeBatch {
  packets: NativePacket[];
}

export interface NativeStartInfo {
  backend: 'octotablet-ink' | 'appkit-nsevent';
  tablets: string[];
  timestampGranularityMs: number | null;
}

export interface PenSimRequest {
  left: number;
  top: number;
  width: number;
  height: number;
  hz: number;
  frames: number;
}

export interface PenSimReport {
  method: string;
  frames: number;
  hz: number;
  achievedHz: number;
  injectEpochMs: number[];
  pressure: number[];
  tiltX: number[];
  tiltY: number[];
  rotation: number[];
}

export interface SavedLog {
  path: string;
  bytes: number;
}
