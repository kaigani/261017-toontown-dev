// Shapes of the mirror-store slices: the result schemas of the queries in store.ts (01 §12.4 and the command rows of
// 01 §12.2). Only the fields the shell reads are spelled out.

export interface AppState {
  readonly project: {
    readonly id: string;
    readonly name: string;
    readonly path: string | null;
    readonly dirty: boolean;
    readonly globalLibraryDirty: boolean;
    readonly readOnly: boolean;
  } | null;
  readonly scene: { readonly id: string; readonly name: string; readonly frameCount: number } | null;
  readonly frame: number | null;
  readonly element: { readonly id: string; readonly name: string; readonly type: string } | null;
  readonly mode: 'drawing' | 'staging' | null;
  readonly activeView: { readonly viewId: string; readonly panelId: string; readonly type: string } | null;
  readonly focusedPanel: string | null;
  readonly workspace: { readonly name: string; readonly modified: boolean };
  readonly startScreenVisible: boolean;
  readonly modal: { readonly kind: string; readonly title: string } | null;
  readonly fullScreen?: boolean;
}

export interface TabGroup {
  readonly tabs: readonly string[];
  readonly current: string;
  readonly size?: number;
}

export interface Geometry {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface Arrangement {
  readonly centralPanel: string;
  readonly corners: 'sides' | 'topBottom';
  readonly docks: {
    readonly left: readonly TabGroup[];
    readonly right: readonly TabGroup[];
    readonly top: readonly TabGroup[];
    readonly bottom: readonly TabGroup[];
  };
  readonly floating: readonly {
    readonly panelId: string;
    readonly visible: boolean;
    readonly geometry: Geometry;
  }[];
  readonly toolPalettes: readonly {
    readonly id: string;
    readonly visible: boolean;
    readonly floating: boolean;
    readonly position?: { readonly x: number; readonly y: number };
  }[];
  readonly mainToolbar: { readonly visible: boolean; readonly items: readonly string[] };
}

export interface Layout extends Arrangement {
  readonly fullScreen: Arrangement;
}

export interface WorkspaceEntry {
  readonly name: string;
  readonly builtIn: boolean;
  readonly modified: boolean;
  readonly position: number;
  readonly current: boolean;
}

export interface PanelEntry {
  readonly panelId: string;
  readonly title: string;
  readonly visible: boolean;
  readonly floating: boolean;
  readonly area?: string;
  readonly tabGroup?: number;
  readonly current: boolean;
  readonly geometry?: Geometry;
  readonly focused: boolean;
}

export interface PrefEntry {
  readonly key: string;
  readonly type: 'bool' | 'int' | 'number' | 'string' | 'enum' | 'colour' | 'path' | 'object';
  readonly default: unknown;
  readonly value: unknown;
  readonly min?: number;
  readonly max?: number;
  readonly values?: readonly string[];
  readonly applies: 'live' | 'nextLaunch' | 'restart';
  readonly description: string;
}

export interface Binding {
  readonly commandId: string;
  readonly params?: Record<string, unknown>;
  readonly keys: string;
  readonly context: 'global' | 'drawing' | 'staging';
  readonly isDefault: boolean;
}

export interface HistoryEntry {
  readonly index: number;
  readonly label: string;
  readonly source: 'user' | 'agent';
  readonly agentName?: string;
  readonly time: string;
}

export interface History {
  readonly entries: readonly HistoryEntry[];
  readonly position: number;
  readonly cleanPosition: number | null;
}

export interface StatusBarTexts {
  readonly visible: boolean;
  readonly description: string;
  readonly context: string;
  readonly pointer: string;
  readonly playback: string;
  readonly agentIndicator: string;
}

export interface Theme {
  readonly themeId: string;
  readonly name: string;
  readonly builtIn: boolean;
  readonly dark: boolean;
  readonly colours?: Record<string, string>;
}

export interface Tip {
  readonly messageId: string;
  readonly description: string;
  readonly suppressed: boolean;
}

export interface RecentEntry {
  readonly path: string;
  readonly name: string;
  readonly lastOpened: string;
  readonly exists: boolean;
}

export interface RecoveryEntry {
  readonly recoveryId: string;
  readonly projectName: string;
  readonly originalPath?: string | null;
  readonly savedAt: string;
}

export interface Preset {
  readonly id: string;
  readonly name: string;
  readonly kind: 'mostRecent' | 'default' | 'bundled';
  readonly frameRate: number;
  readonly width: number;
  readonly height: number;
}

export interface AgentSessionInfo {
  readonly sessionId: string;
  readonly client: string;
  readonly policy: string;
  readonly scopes: readonly string[];
  readonly connectedAt: string;
  readonly lastActivity?: string | null;
}

export interface AgentServerStatus {
  readonly enabled: boolean;
  readonly endpoint?: string | null;
  readonly sessions: readonly AgentSessionInfo[];
}
