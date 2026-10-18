// Wire types shared with the Rust host. The product types are generated from the Rust types (src/ipc/generated/,
// ADR-0012), so the two sides cannot drift; integers that can exceed 2^53 - 1 are typed `number | bigint`. The M0
// diagnostics and spike types below are hand-written (not product API).
import type { EngineInfo as GeneratedEngineInfo } from './generated/wire';

export type {
  ErrorInfo,
  MenuCommandEvent,
  MenuHandler,
  PingRequest,
  PingResponse,
  RunMode,
} from './generated/wire';

/**
 * Internal engine bootstrap information (crates/engine `EngineInfo`). Not the spec query `app.info.get` (01 §12),
 * which adds `locale` and `languages[]` and arrives as a registered query in C1.4. Spelled out field by field so the
 * C0.1 regression (tests/adversarial/c01.rs) can read the key list; `ENGINE_INFO_IS_GENERATED` proves at compile
 * time that it is exactly the type generated from Rust.
 */
export interface EngineInfo {
  name: GeneratedEngineInfo['name'];
  version: GeneratedEngineInfo['version'];
  platform: GeneratedEngineInfo['platform'];
  arch: GeneratedEngineInfo['arch'];
  formatVersion: GeneratedEngineInfo['formatVersion'];
  registryVersion: GeneratedEngineInfo['registryVersion'];
  runMode: GeneratedEngineInfo['runMode'];
}

/** `true` only when A and B are mutually assignable (same keys, same optionality, same value types). */
type Exact<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;

/** Compile-time proof that the spelled-out `EngineInfo` equals the generated one. */
export const ENGINE_INFO_IS_GENERATED: Exact<EngineInfo, GeneratedEngineInfo> &
  Exact<keyof EngineInfo, keyof GeneratedEngineInfo> = true;

/** M0 diagnostics: what `toontown --ipc-bench` asks the front end to measure. */
export interface BenchPlan {
  count: number;
  payloadBytes: number[];
}

// ---- M0 spike: C0.2 tablet input (desktop/src-tauri/src/spikes/tablet). Not product API. ----

export interface SpikeMode {
  name: 'tablet' | 'canvas';
  logPath: string | null;
  nativeBackend: 'octotablet-ink' | 'appkit-nsevent' | null;
  penSimulation: boolean;
  auto: boolean;
  /** Canvas spike: renderer forced with `--spike-renderer` (`null` or absent = automatic). */
  renderer?: 'webgpu' | 'webgl2' | null;
}

// ---- M0 spike: C0.3 canvas throughput (desktop/src-tauri/src/spikes/canvas). Not product API. ----

export type CanvasSceneKind = 'paths100k' | 'groups1k' | 'brush10k';

export interface CanvasSceneRequest {
  scene: CanvasSceneKind;
  seed: number;
  frame: number;
}

/** Header of a `TTRL` v1 render list (format: desktop/src-tauri/src/spikes/canvas/renderlist.rs). */
export interface RenderListHeader {
  version: 1;
  headerLength: 72;
  kind: CanvasSceneKind;
  seed: number;
  frame: number;
  groups: number;
  paths: number;
  verbs: number;
  points: number;
  bounds: number[];
  flags: number;
  encodeNs: number;
  generateNs: number;
  /** Total bytes, equal to the length computed from the counts. */
  byteLength: number;
}

/** A certified render-list envelope and its bytes (the sections are decoded by the page). */
export interface RenderListPayload {
  header: RenderListHeader;
  bytes: ArrayBuffer;
}

export interface PixelFrameHeader {
  width: number;
  height: number;
  fillNs: number;
  byteLength: number;
}

export interface PixelFramePayload {
  header: PixelFrameHeader;
  bytes: ArrayBuffer;
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
