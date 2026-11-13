// Public surface of the IPC module. Everything outside src/ipc imports from here.
import { createHostApi, type HostApi } from './hostApi';
import { createMockTransport } from './mockTransport';
import { createTauriTransport, isTauriRuntime } from './tauriTransport';

export { createHostApi, HostError, MENU_COMMAND_EVENT, toHostError, type HostApi } from './hostApi';
export {
  createEngineApi,
  ENGINE_EVENT,
  isCancelled,
  localFailure,
  paramProblems,
  type CommandFailure,
  type EngineApi,
  type EngineEvent,
  type Envelope,
  type KeyOutcome,
  type PickRequest,
  type RunOptions,
  type Running,
  type ShellReport,
} from './engine';
export { createMockTransport, MOCK_ENGINE_INFO, type MockTransport } from './mockTransport';
export type { SpikeApi } from './spikeApi';
export type { HostTransport, Unlisten } from './transport';
export type * from './types';
export { useHost } from './context';
export { HostProvider } from './react';

/** The host API for the current runtime: Tauri inside the app, the in-memory mock in a plain browser. */
export function createDefaultHostApi(): HostApi {
  return createHostApi(isTauriRuntime() ? createTauriTransport() : createMockTransport());
}
