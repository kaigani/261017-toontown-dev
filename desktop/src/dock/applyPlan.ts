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
  for (const id of plan.active) api.getPanel(id)?.api.setActive();
}
