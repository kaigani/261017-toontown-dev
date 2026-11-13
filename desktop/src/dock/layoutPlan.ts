// Layout ↔ dock grid (M4 C4.1). The kernel's arrangement (01 §12.4 `Layout`) is rendered as a dock grid: the central
// view; dock areas whose tab groups stack along the area's long side; corner ownership decides which areas run the
// full window length (01 §5.4). A drop on the grid is never applied by the grid itself: it is translated to the one
// `window.panel.move` an agent would send, and the grid re-renders from the kernel's answer.
import type { Arrangement, TabGroup } from '../state/types';

export type Direction = 'left' | 'right' | 'above' | 'below' | 'within';

/** One panel to add, in order. `reference` absent: relative to the whole grid (a root edge). */
export interface AddStep {
  readonly panelId: string;
  readonly reference?: string;
  readonly direction?: Direction;
  /** Width (left/right areas) or height (top/bottom areas) of the area, on its first panel. */
  readonly initialWidth?: number;
  readonly initialHeight?: number;
}

export interface Plan {
  readonly steps: readonly AddStep[];
  /** The tab in front of each group. */
  readonly active: readonly string[];
}

type Area = 'left' | 'right' | 'top' | 'bottom';

const EDGE: Record<Area, Direction> = { left: 'left', right: 'right', top: 'above', bottom: 'below' };

function areaSteps(area: Area, groups: readonly TabGroup[], reference: string | undefined): AddStep[] {
  const steps: AddStep[] = [];
  let previous: string | undefined;
  groups.forEach((g, i) => {
    const first = g.tabs[0];
    if (first === undefined) return;
    const size = i === 0 ? g.size : undefined;
    if (i === 0) {
      steps.push({
        panelId: first,
        ...(reference === undefined ? {} : { reference }),
        direction: EDGE[area],
        ...(size !== undefined && (area === 'left' || area === 'right') ? { initialWidth: size } : {}),
        ...(size !== undefined && (area === 'top' || area === 'bottom') ? { initialHeight: size } : {}),
      });
    } else if (previous !== undefined) {
      // Left and right areas stack their groups downwards, top and bottom areas side by side (01 §12.4).
      steps.push({
        panelId: first,
        reference: previous,
        direction: area === 'left' || area === 'right' ? 'below' : 'right',
      });
    }
    for (const t of g.tabs.slice(1)) steps.push({ panelId: t, reference: first, direction: 'within' });
    previous = first;
  });
  return steps;
}

/** The build plan of an arrangement (floating panels are separate windows and are not part of the grid). */
export function planLayout(a: Arrangement): Plan {
  const steps: AddStep[] = [{ panelId: a.centralPanel }];
  const inner: Area[] = a.corners === 'sides' ? ['top', 'bottom'] : ['left', 'right'];
  const outer: Area[] = a.corners === 'sides' ? ['left', 'right'] : ['top', 'bottom'];
  // Inner areas sit next to the central view; outer areas take the whole window length (corner ownership).
  for (const area of inner) steps.push(...areaSteps(area, a.docks[area], a.centralPanel));
  for (const area of outer) steps.push(...areaSteps(area, a.docks[area], undefined));
  const active = (['left', 'right', 'top', 'bottom'] as Area[]).flatMap((area) =>
    a.docks[area].map((g) => g.current),
  );
  return { steps, active };
}

/** Where a panel is docked in an arrangement. */
export function placeOf(
  a: Arrangement,
  panelId: string,
): { area: Area | 'central'; group?: TabGroup } | null {
  if (a.centralPanel === panelId) return { area: 'central' };
  for (const area of ['left', 'right', 'top', 'bottom'] as Area[]) {
    const group = a.docks[area].find((g) => g.tabs.includes(panelId));
    if (group) return { area, group };
  }
  return null;
}

export type DropPosition = 'top' | 'bottom' | 'left' | 'right' | 'center';

/**
 * The `window.panel.move` parameters for a drop of `panelId` at `position` on the group whose front tab is `target`
 * (or on the grid's edge when `target` is null). Null: nothing to do (dropped onto itself).
 */
export function dropToMove(
  a: Arrangement,
  panelId: string,
  target: string | null,
  position: DropPosition,
): Record<string, unknown> | null {
  const edgeArea: Record<Exclude<DropPosition, 'center'>, Area> = {
    left: 'left',
    right: 'right',
    top: 'top',
    bottom: 'bottom',
  };
  if (target === null) {
    if (position === 'center') return { panelId, area: 'central' };
    return { panelId, area: edgeArea[position], position: 'after' };
  }
  if (target === panelId && position === 'center') return null;
  const place = placeOf(a, target);
  if (!place) return null;
  if (place.area === 'central') {
    // Onto the central view: its middle replaces the central view, its edges dock into that side's area.
    if (position === 'center') return { panelId, area: 'central' };
    return { panelId, area: edgeArea[position], position: 'after' };
  }
  const relative: Record<DropPosition, string> = {
    center: 'tab',
    left: 'before',
    right: 'after',
    top: 'above',
    bottom: 'below',
  };
  return { panelId, area: place.area, relativeTo: target, position: relative[position] };
}

/** A stable fingerprint of what the grid shows, to skip rebuilding when the kernel's layout did not change. */
export function fingerprint(a: Arrangement): string {
  return JSON.stringify([a.centralPanel, a.corners, a.docks, a.floating]);
}
