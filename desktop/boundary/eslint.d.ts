// Types for boundary/eslint.js (imported by eslint.config.js and the boundary tests).
import type { ESLint, Linter } from 'eslint';

export declare const EXTENSIONS: string[];
export declare const TRANSPORT_FILE: string;
export declare const IPC_INDEX_FILE: string;
export declare const SAMPLE_FILES: string[];
export declare const TAURI_GLOBALS: string[];
export declare const MAX_SRC_DEPTH: number;
export declare const BOUNDARY_RULE_NAMES: string[];
export declare const ipcBoundaryRules: Linter.RulesRecord;
export declare const ipcBoundaryExemption: Linter.RulesRecord;
export declare function ipcBoundaryConfig(commentsPlugin: ESLint.Plugin): Linter.Config[];
