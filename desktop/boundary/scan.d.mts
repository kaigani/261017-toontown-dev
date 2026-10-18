// Types for boundary/scan.mjs.
export declare const TAURI_TEXT: RegExp;
export declare const TRANSPORT_CHUNK: string;
export declare function decodeEscapes(text: string): string;
export declare function textMentions(text: string): string[];
export declare function constantString(node: unknown, depth?: number): string | undefined;
export declare function astMentions(code: string): string[] | null;
export declare function mentions(code: string, options?: { javascript?: boolean }): string[];
export declare function isTransportChunkFile(fileName: string): boolean;
export declare function scanDist(dir: string): {
  problems: { file: string; hits: string[] }[];
  transportFiles: number;
};
