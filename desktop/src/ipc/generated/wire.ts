// GENERATED FILE: do not edit.
// Source: Rust types via schemars (crates/engine/src/wire.rs, crates/model/src/schema.rs).
// Regenerate: TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas
import type { Schema } from '../schema';

export const FORMAT_VERSION = "1.0";

export const REGISTRY_VERSION = "0.0.0";

export const COMMAND_ID_PATTERN = "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$";

export const WIRE_DEFS: Readonly<Record<string, Schema>> = {
  EngineInfo: {
    type: "object",
    additionalProperties: false,
    properties: {
      name: {
        type: "string",
        description: "Application name.",
      },
      version: {
        type: "string",
        description: "Application version.",
      },
      platform: {
        type: "string",
        description: "Operating system the engine was compiled for (`windows`, `macos`, `linux`).",
      },
      arch: {
        type: "string",
        description: "CPU architecture (`x86_64`, `aarch64`).",
      },
      formatVersion: {
        type: "string",
        description: "Project format / document schema version (10 §6.6).",
        pattern: "^[0-9]+[.][0-9]+$",
      },
      registryVersion: {
        type: "string",
        description: "Command-registry version (11 §4).",
        pattern: "^[0-9]+[.][0-9]+[.][0-9]+$",
      },
      runMode: {
        $ref: "#/$defs/RunMode",
        description: "Run mode.",
      },
    },
    required: [
      "name",
      "version",
      "platform",
      "arch",
      "formatVersion",
      "registryVersion",
      "runMode",
    ],
    description: "Internal bootstrap information about the running engine: the engine-level facts the MCP `initialize` server info\nneeds (11 §4: name, version, registry version, document schema version, run mode) plus platform and architecture.\n\nThis is **not** the result of the spec query `app.info.get` (01 §12), which also reports `locale` and\n`languages[]`; those depend on settings and localisation, which do not exist yet. `app.info.get` is a registered\nquery from C1.4 (ticket in `plan/traceability.csv`) and will be built on this type, not replace it.",
  },
  ErrorInfo: {
    type: "object",
    additionalProperties: false,
    properties: {
      code: {
        type: "string",
        description: "Stable dotted code.",
      },
      message: {
        type: "string",
        description: "Human-readable message.",
      },
      details: {
        description: "Structured details (code-specific); absent = `{}`.",
        default: null,
        "x-omitDefault": true,
      },
    },
    required: [
      "code",
      "message",
    ],
    description: "Serializable error envelope: `{code, message, details}` (11 §10).",
  },
  MenuCommandEvent: {
    type: "object",
    additionalProperties: false,
    properties: {
      command: {
        type: "string",
        description: "Command id.",
        pattern: "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$",
      },
      params: {
        type: "object",
        additionalProperties: true,
        description: "Fixed parameters from the manifest.",
      },
      handler: {
        $ref: "#/$defs/MenuHandler",
        description: "`frontend` or `pending`.",
      },
      milestone: {
        type: [
          "string",
          "null",
        ],
        description: "Milestone of a pending command.",
      },
    },
    required: [
      "command",
      "params",
      "handler",
      "milestone",
    ],
    description: "Payload sent to the front end when a menu item it handles (or a pending one) is chosen.",
  },
  MenuHandler: {
    oneOf: [
      {
        type: "string",
        const: "frontend",
        description: "The front end handles it.",
      },
      {
        type: "string",
        const: "pending",
        description: "Not implemented yet (the milestone that delivers it is named).",
      },
    ],
    description: "Who handles a forwarded menu command.",
  },
  PingRequest: {
    type: "object",
    additionalProperties: false,
    properties: {
      seq: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Caller-chosen sequence number, echoed back.",
      },
      payload: {
        type: "string",
        description: "Optional payload, echoed back (lets the caller measure size-dependent cost).",
        default: null,
        "x-omitDefault": true,
      },
    },
    required: [
      "seq",
    ],
    description: "Parameters of a ping.",
  },
  PingResponse: {
    type: "object",
    additionalProperties: false,
    properties: {
      seq: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "The request's sequence number.",
      },
      payload: {
        type: [
          "string",
          "null",
        ],
        description: "The request's payload.",
      },
      engineUptimeNs: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Engine uptime when the ping was handled, in nanoseconds.",
      },
      handlerNs: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Time spent inside the engine handling the ping, in nanoseconds.",
      },
    },
    required: [
      "seq",
      "payload",
      "engineUptimeNs",
      "handlerNs",
    ],
    description: "Result of a ping.",
  },
  RunMode: {
    oneOf: [
      {
        type: "string",
        const: "attached",
        description: "GUI running; agents attach to the live session.",
      },
      {
        type: "string",
        const: "headless",
        description: "No window; the full engine runs offscreen with virtual views.",
      },
      {
        type: "string",
        const: "batchScript",
        description: "Headless replay of a batch file, then exit.",
      },
    ],
    description: "How the engine was started (11 §3).",
  },
};

/** Internal bootstrap information about the running engine: the engine-level facts the MCP `initialize` server info needs (11 §4: name, version, registry version, document schema version, run mode) plus platform and architecture.  This is **not** the result of the spec query `app.info.get` (01 §12), which also reports `locale` and `languages[]`; those depend on settings and localisation, which do not exist yet. `app.info.get` is a registered query from C1.4 (ticket in `plan/traceability.csv`) and will be built on this type, not replace it. */
export interface EngineInfo {
  /** Application name. */
  name: string;
  /** Application version. */
  version: string;
  /** Operating system the engine was compiled for (`windows`, `macos`, `linux`). */
  platform: string;
  /** CPU architecture (`x86_64`, `aarch64`). */
  arch: string;
  /** Project format / document schema version (10 §6.6). */
  formatVersion: string;
  /** Command-registry version (11 §4). */
  registryVersion: string;
  /** Run mode. */
  runMode: RunMode;
}

/** Serializable error envelope: `{code, message, details}` (11 §10). */
export interface ErrorInfo {
  /** Stable dotted code. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (code-specific); absent = `{}`. */
  details?: unknown;
}

/** Payload sent to the front end when a menu item it handles (or a pending one) is chosen. */
export interface MenuCommandEvent {
  /** Command id. */
  command: string;
  /** Fixed parameters from the manifest. */
  params: Record<string, unknown>;
  /** `frontend` or `pending`. */
  handler: MenuHandler;
  /** Milestone of a pending command. */
  milestone: string | null;
}

/** Who handles a forwarded menu command. */
export type MenuHandler = 'frontend' | 'pending';

/** Parameters of a ping. */
export interface PingRequest {
  /** Caller-chosen sequence number, echoed back. */
  seq: number | bigint;
  /** Optional payload, echoed back (lets the caller measure size-dependent cost). */
  payload?: string;
}

/** Result of a ping. */
export interface PingResponse {
  /** The request's sequence number. */
  seq: number | bigint;
  /** The request's payload. */
  payload: string | null;
  /** Engine uptime when the ping was handled, in nanoseconds. */
  engineUptimeNs: number | bigint;
  /** Time spent inside the engine handling the ping, in nanoseconds. */
  handlerNs: number | bigint;
}

/** How the engine was started (11 §3). */
export type RunMode = 'attached' | 'headless' | 'batchScript';
