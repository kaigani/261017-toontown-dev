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
  /** Device clock (see `NativeStartInfo.deviceClock`); never compared with `hostEpochMs`. */
  deviceMs: number | null;
  x: number;
  y: number;
  pressure: number | null;
  tiltX: number | null;
  tiltY: number | null;
  twist: number | null;
  tool: string;
  /** Host sequence number (contiguous per listener); present on every packet received over the channel. */
  seq?: number;
  /** Raw OS tilt (macOS `NSEvent.tilt`, -1..1) next to the converted degrees. */
  rawTiltX?: number | null;
  rawTiltY?: number | null;
  /** OS device identity (macOS `deviceID`), if any. */
  deviceId?: string | null;
  /** macOS `windowNumber` of the event (conversion input for `locationInWindow`). */
  windowNumber?: number | null;
}

/** A packet as received over the channel: every extra is present. */
export interface WirePacket extends NativePacket {
  seq: number;
  rawTiltX: number | null;
  rawTiltY: number | null;
  deviceId: string | null;
  windowNumber: number | null;
}

export interface NativeBatch {
  packets: WirePacket[];
  /** Packets the host discarded because its buffer was full, cumulative. */
  hostDropped: number;
  /** Channel sends that failed, cumulative. */
  sendFailures: number;
}

/** Main-window geometry in physical pixels plus the backing scale factor. */
export interface WindowGeometry {
  scaleFactor: number;
  innerX: number;
  innerY: number;
  innerWidth: number;
  innerHeight: number;
}

export interface NativeStartInfo {
  backend: 'octotablet-ink' | 'appkit-nsevent';
  tablets: string[];
  timestampGranularityMs: number | null;
  /** Clock domain of `deviceMs`. */
  deviceClock: string;
  /** Main-window geometry when the listener started (conversion inputs). */
  window: WindowGeometry | null;
}

/** Host facts recorded in the log (`spike_environment`). */
export interface SpikeEnvironment {
  os: string;
  arch: string;
  osVersion: string | null;
  webviewVersion: string | null;
  cpuCount: number | null;
  window: WindowGeometry | null;
}

/** `stationary` keeps the position but varies pressure, tilt and rotation; `pressureOnly` varies pressure only. */
export type PenPattern = 'sine' | 'zigzag' | 'stationary' | 'pressureOnly';

export interface PenSimRequest {
  left: number;
  top: number;
  width: number;
  height: number;
  hz: number;
  frames: number;
  pattern?: PenPattern;
}

export interface PenSimReport {
  method: string;
  frames: number;
  hz: number;
  /** Host estimator `frames * 1e9 / (contactEndNs - contactStartNs)`. */
  achievedHz: number;
  /** Monotonic ns (process anchor) before the first contact frame / after the last frame period (log version 3). */
  contactStartNs?: number;
  contactEndNs?: number;
  /** Wall clock read with each monotonic endpoint. */
  contactStartEpochMs?: number;
  contactEndEpochMs?: number;
  /** Foreground and focus windows at the start and end of the stroke. */
  focus?: FocusSnapshot[];
  /** Host wall clock immediately before each contact frame's inject call, ms since the Unix epoch. */
  injectEpochMs: number[];
  /** Host wall clock when each inject call returned (round 1 and later). */
  injectReturnEpochMs?: number[];
  /** Screen position of each contact frame, physical pixels. */
  screenX?: number[];
  screenY?: number[];
  /** Target rectangle on screen `[left, top, width, height]`, physical pixels. */
  screenRect?: number[];
  pattern?: PenPattern;
  /** What was targeted: this app's canvas or the probe window. */
  target?: 'webview' | 'nativeWindow';
  pressure: number[];
  tiltX: number[];
  tiltY: number[];
  rotation: number[];
}

/** Foreground/focus state recorded by the host around each injected stroke (Windows). */
export interface FocusSnapshot {
  when: 'start' | 'end';
  ownPid: number;
  targetHwnd: number;
  foregroundHwnd: number;
  foregroundPid: number;
  foregroundClass: string;
  focusHwnd: number;
  focusPid: number;
  focusClass: string;
  activeHwnd: number;
  captureHwnd: number;
}

/**
 * Result of the native-window probe or of the raw-input sink (C02-R1-04). Plain JSON that is logged and read by the
 * analyzer; the page reads only a few counters from it.
 */
export type ProbeResult = Record<string, unknown>;

export interface SavedLog {
  path: string;
  bytes: number;
}
