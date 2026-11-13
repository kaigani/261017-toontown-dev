// The dock area (01 §5.4; M4 C4.1): renders the kernel's arrangement with dockview and turns every docking gesture
// into a command. Drops are vetoed and replaced by `window.panel.move`; a tab's close button is `window.panel.set
// {visible: false}`; a double-click toggles floating; the tab bar's context menu is the panel checklist. A splitter
// drag ends in the same-place `window.panel.move` (a panel tabbed with itself) that records the area's size in every
// area (SI-118, interim ruling O-3a). The grid is rebuilt from the kernel's answer.
import {
  DockviewReact,
  themeDark,
  themeLight,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewPanelHeaderProps,
  type IDockviewPanelProps,
} from 'dockview-react';
import { useCallback, useEffect, useRef, useState, type FunctionComponent } from 'react';

import { PANEL_CATALOGUE, panelInfo } from '../panels/catalogue';
import { PanelBody } from '../panels/PanelBody';
import { useActions } from '../shell/actionsApi';
import { useSlice } from '../state/context';
import { useAppliedTheme } from '../theme/theme';
import type { Arrangement, Layout, PanelEntry } from '../state/types';
import { applyPlan } from './applyPlan';
import { dropToMove, fingerprint, samePlaceResize, type Area } from './layoutPlan';

function title(id: string): string {
  return panelInfo(id)?.title ?? id;
}

function PanelComponent(props: IDockviewPanelProps<{ panelId: string }>) {
  return <PanelBody panelId={props.params.panelId} />;
}

function Tab(props: IDockviewPanelHeaderProps<{ panelId: string }>) {
  const actions = useActions();
  const id = props.params.panelId;
  const panels = (useSlice('panels') as readonly PanelEntry[] | undefined) ?? [];
  const floating = panels.find((p) => p.panelId === id)?.floating === true;
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  // The shown tab of its group is reported as selected (the UI tree's `checked`; O-5).
  const [shown, setShown] = useState(props.api.isVisible);
  useEffect(() => {
    const d = props.api.onDidVisibilityChange((e) => {
      setShown(e.isVisible);
    });
    return () => {
      d.dispose();
    };
  }, [props.api]);
  return (
    <div
      className="dock-tab"
      role="tab"
      aria-selected={shown}
      data-tt-role="tab"
      data-tt-id={`tab.${id}`}
      data-tt-label={title(id)}
      onDoubleClick={() => {
        // Double-click toggles floating/docked (01 §5.4).
        void actions.run('window.panel.set', { panelId: id, floating: !floating });
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
    >
      <span>{title(id)}</span>
      <button
        type="button"
        className="dock-tab-close"
        aria-label={`Hide ${title(id)}`}
        data-tt-role="button"
        data-tt-id={`tab.${id}.hide`}
        data-tt-command="window.panel.set"
        data-tt-params={JSON.stringify({ panelId: id, visible: false })}
        onPointerDown={(e) => {
          e.stopPropagation();
        }}
        onClick={(e) => {
          e.stopPropagation();
          void actions.run('window.panel.set', { panelId: id, visible: false });
        }}
      >
        ×
      </button>
      {menu && (
        <PanelChecklist
          at={menu}
          onClose={() => {
            setMenu(null);
          }}
        />
      )}
    </div>
  );
}

/** The tab bar's context menu: every panel with a show/hide check mark (01 §5.4). */
export function PanelChecklist({ at, onClose }: { at: { x: number; y: number }; onClose: () => void }) {
  const actions = useActions();
  return (
    <div
      className="context-menu"
      role="menu"
      style={{ left: at.x, top: at.y }}
      data-tt-role="menu"
      data-tt-id="panelChecklist"
      onMouseLeave={onClose}
    >
      {PANEL_CATALOGUE.map((p) => (
        <button
          key={p.id}
          type="button"
          role="menuitemcheckbox"
          data-tt-role="menuitem"
          data-tt-id={`panelChecklist.${p.id}`}
          data-tt-command="window.panel.set"
          onClick={() => {
            onClose();
            void actions.run('window.panel.set', { panelId: p.id, visible: true });
          }}
        >
          {p.title}
        </button>
      ))}
    </div>
  );
}

const components: Record<string, FunctionComponent<IDockviewPanelProps<{ panelId: string }>>> = {
  panel: PanelComponent,
};

export function DockArea({ layout, fullScreen }: { layout: Layout | undefined; fullScreen: boolean }) {
  const actions = useActions();
  const dark = useAppliedTheme() === 'dark';
  const apiRef = useRef<DockviewApi | null>(null);
  const shown = useRef<string>('');
  const arrangement: Arrangement | undefined = layout ? (fullScreen ? layout.fullScreen : layout) : undefined;
  const arrangementRef = useRef(arrangement);
  useEffect(() => {
    arrangementRef.current = arrangement;
  }, [arrangement]);

  const render = useCallback(() => {
    const api = apiRef.current;
    const a = arrangementRef.current;
    if (!api || !a) return;
    const fp = fingerprint(a);
    if (fp === shown.current) return;
    shown.current = fp;
    applyPlan(api, a);
  }, []);

  useEffect(render, [arrangement, render]);

  // The end of a splitter drag records the side areas' new sizes with the same-place `window.panel.move` (SI-118,
  // interim ruling O-3a). Sizes are measured when the drag starts and when it ends; only a changed area is sent.
  const hostRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const AREAS: readonly Area[] = ['left', 'right', 'top', 'bottom'];
    const measure = (): Partial<Record<Area, number>> => {
      const api = apiRef.current;
      const a = arrangementRef.current;
      const out: Partial<Record<Area, number>> = {};
      if (!api || !a) return out;
      for (const area of AREAS) {
        const first = a.docks[area][0];
        const group = first ? api.getPanel(first.current)?.group : undefined;
        if (group) out[area] = area === 'left' || area === 'right' ? group.api.width : group.api.height;
      }
      return out;
    };
    let start: Partial<Record<Area, number>> | null = null;
    const down = (e: PointerEvent) => {
      start = e.target instanceof Element && e.target.closest('.dv-sash') ? measure() : null;
    };
    const up = () => {
      const a = arrangementRef.current;
      if (!start || !a) return;
      const end = measure();
      for (const area of AREAS) {
        const before = start[area];
        const after = end[area];
        if (before === undefined || after === undefined || Math.abs(after - before) < 2) continue;
        const params = samePlaceResize(a, area, after);
        if (params) void actions.run('window.panel.move', params);
      }
      start = null;
    };
    host.addEventListener('pointerdown', down, true);
    window.addEventListener('pointerup', up, true);
    return () => {
      host.removeEventListener('pointerdown', down, true);
      window.removeEventListener('pointerup', up, true);
    };
  }, [actions]);

  const onReady = useCallback(
    (e: DockviewReadyEvent) => {
      apiRef.current = e.api;
      // Docking gestures become commands: dockview never rearranges by itself (gesture equivalence, 11 §2).
      e.api.onWillDrop((drop) => {
        drop.preventDefault();
        const a = arrangementRef.current;
        const moved = drop.getData()?.panelId;
        if (!a || !moved) return;
        const target = drop.group?.activePanel?.id ?? null;
        const params = dropToMove(a, moved, target, drop.position);
        if (params) void actions.run('window.panel.move', params);
      });
      render();
    },
    [actions, render],
  );

  return (
    <div className="dock-host" ref={hostRef} data-tt-role="dock" data-tt-id="dock">
      <DockviewReact
        components={components}
        defaultTabComponent={Tab}
        onReady={onReady}
        className="tt-dockview"
        theme={dark ? themeDark : themeLight}
      />
    </div>
  );
}
