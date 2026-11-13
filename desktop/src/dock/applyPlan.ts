// Applies a build plan (layoutPlan.ts) to a dockview instance.
import type { DockviewApi } from 'dockview-react';

import { panelInfo } from '../panels/catalogue';
import type { Arrangement } from '../state/types';
import { planLayout } from './layoutPlan';

function title(id: string): string {
  return panelInfo(id)?.title ?? id;
}

/** Applies a build plan to a dockview instance. */
export function applyPlan(api: DockviewApi, a: Arrangement): void {
  api.clear();
  const plan = planLayout(a);
  for (const s of plan.steps) {
    if (api.getPanel(s.panelId)) continue;
    api.addPanel({
      id: s.panelId,
      component: 'panel',
      title: title(s.panelId),
      params: { panelId: s.panelId },
      ...(s.direction
        ? {
            position: s.reference
              ? { referencePanel: s.reference, direction: s.direction }
              : {
                  direction:
                    s.direction === 'above' ? 'above' : s.direction === 'below' ? 'below' : s.direction,
                },
          }
        : {}),
      ...(s.initialWidth !== undefined ? { initialWidth: s.initialWidth } : {}),
      ...(s.initialHeight !== undefined ? { initialHeight: s.initialHeight } : {}),
    });
  }
  // Floating panels (01 §5.4) float over the window at their kernel geometry. Separate OS windows are listed in the
  // C4.1 handoff with their owner.
  for (const f of a.floating) {
    if (!f.visible || api.getPanel(f.panelId)) continue;
    api.addPanel({
      id: f.panelId,
      component: 'panel',
      title: title(f.panelId),
      params: { panelId: f.panelId },
      floating: {
        position: { left: f.geometry.x, top: f.geometry.y },
        width: f.geometry.width,
        height: f.geometry.height,
      },
    });
  }
  for (const id of plan.active) api.getPanel(id)?.api.setActive();
}
