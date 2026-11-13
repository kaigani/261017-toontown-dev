// The panel catalogue (01 §5.2) with the checkpoint that brings each panel's content. Titles and owners mirror the
// kernel (`crates/executor/src/ui.rs` PANELS, `help::panel_owner`); the running-application test compares this list
// with `window.panel.list`.

export interface PanelInfo {
  readonly id: string;
  readonly title: string;
  /** The checkpoint that brings the content (null: implemented). */
  readonly owner: string | null;
  readonly view?: boolean;
}

export const PANEL_CATALOGUE: readonly PanelInfo[] = [
  {
    id: 'cameraView',
    title: 'Camera view',
    owner: 'M4 C4.2 (canvas) and M4 C4.3 (Camera view tools)',
    view: true,
  },
  { id: 'drawingView', title: 'Drawing view', owner: 'M4 C4.2', view: true },
  { id: 'topView', title: 'Top view', owner: 'M6', view: true },
  { id: 'sideView', title: 'Side view', owner: 'M6', view: true },
  { id: 'timeline', title: 'Timeline', owner: 'M5 C5.1' },
  { id: 'xsheet', title: 'Exposure sheet', owner: 'M5 C5.2' },
  { id: 'functionEditor', title: 'Function editor', owner: 'M6' },
  { id: 'properties', title: 'Properties', owner: 'M4 C4.2' },
  { id: 'colour', title: 'Colour', owner: 'M7 C7.1' },
  { id: 'pen', title: 'Pen', owner: 'M4 C4.2' },
  { id: 'text', title: 'Text', owner: 'M4 C4.2' },
  { id: 'cells', title: 'Cells', owner: 'M5 C5.2' },
  { id: 'library', title: 'Library', owner: 'M9' },
  { id: 'storyline', title: 'Storyline', owner: 'M9' },
  { id: 'sceneManager', title: 'Scene manager', owner: 'M9' },
  { id: 'effectPreview', title: 'Effect preview', owner: 'M7' },
  { id: 'motionRecording', title: 'Motion recording', owner: 'M6' },
  { id: 'lipSync', title: 'Lip-sync', owner: 'M8' },
  { id: 'history', title: 'History', owner: null },
  { id: 'soundEditor', title: 'Sound element editor', owner: 'M8' },
  { id: 'capture', title: 'Capture window', owner: 'M10 C10.3' },
];

export const TOOL_PALETTES: readonly {
  readonly id: string;
  readonly title: string;
  readonly owner: string;
}[] = [
  { id: 'drawingTools', title: 'Drawing tools', owner: 'M4 C4.2' },
  { id: 'animationTools', title: 'Animation tools', owner: 'M6 C6.1' },
  { id: 'boneTools', title: 'Bone tools', owner: 'M6 C6.3' },
  { id: 'effectTools', title: 'Effect tools', owner: 'M7' },
  { id: 'soundTools', title: 'Sound tools', owner: 'M8' },
];

export function panelInfo(id: string): PanelInfo | undefined {
  return PANEL_CATALOGUE.find((p) => p.id === id);
}
