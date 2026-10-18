import type { DockviewApi } from 'dockview-react';

/** Panels of the M0 shell. Ids are the stable panel ids of spec 01 §5.2. */
export const PANELS = {
  drawingView: { title: 'Drawing' },
  timeline: { title: 'Timeline' },
  properties: { title: 'Properties' },
} as const;

export type PanelId = keyof typeof PANELS;

export const PANEL_IDS = Object.keys(PANELS) as PanelId[];

/**
 * Applies the default arrangement: Drawing as the central view, Properties docked right at full height ("side
 * panels extended", 01 §5.5 Default workspace), Timeline docked below the central view.
 */
export function applyDefaultLayout(api: DockviewApi): void {
  api.clear();
  api.addPanel({ id: 'drawingView', component: 'drawingView', title: PANELS.drawingView.title });
  api.addPanel({
    id: 'properties',
    component: 'properties',
    title: PANELS.properties.title,
    position: { direction: 'right' },
    initialWidth: 300,
  });
  api.addPanel({
    id: 'timeline',
    component: 'timeline',
    title: PANELS.timeline.title,
    position: { referencePanel: 'drawingView', direction: 'below' },
    initialHeight: 220,
  });
}
