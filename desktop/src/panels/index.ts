import type { IDockviewPanelProps } from 'dockview-react';
import type { FunctionComponent } from 'react';

import type { PanelId } from '../layout/defaultLayout';
import { DrawingPanel, PropertiesPanel, TimelinePanel } from './PanelViews';

export const panelComponents: Record<PanelId, FunctionComponent<IDockviewPanelProps>> = {
  drawingView: DrawingPanel,
  timeline: TimelinePanel,
  properties: PropertiesPanel,
};
