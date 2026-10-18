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

/** Superset of `app.info.get` (01 §12). */
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
