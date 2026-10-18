// Review fixture: outside desktop/, but reachable from a Vite frontend import.
import { invoke } from '../../../desktop/node_modules/@tauri-apps/api/core';

export const outsideBypass = () => invoke<unknown>('app_info');
