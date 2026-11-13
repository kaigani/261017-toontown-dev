// The dock area (01 §5.4; M4 C4.1): renders the kernel's arrangement with dockview and turns every docking gesture
// into a command. Drops are vetoed and replaced by `window.panel.move`; a tab's close button is `window.panel.set
// {visible: false}`; a double-click toggles floating; the tab bar's context menu is the panel checklist; a finished
// splitter drag stores the area sizes with `workspace.layout.set`. The grid is rebuilt from the kernel's answer.
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
import type { Arrangement, Layout, PanelEntry } from '../state/types';
import { applyPlan } from './applyPlan';
import { dropToMove, fingerprint } from './layoutPlan';

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
  return (
    <div
      className="dock-tab"
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

/** Whether the applied theme is dark (theme.ts sets `data-theme` on the document element). */
function useDarkTheme(): boolean {
  const read = () => document.documentElement.getAttribute('data-theme') !== 'light';
  const [dark, setDark] = useState(read);
  useEffect(() => {
    const observer = new MutationObserver(() => {
      setDark(document.documentElement.getAttribute('data-theme') !== 'light');
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    return () => {
      observer.disconnect();
    };
  }, []);
  return dark;
}

export function DockArea({ layout, fullScreen }: { layout: Layout | undefined; fullScreen: boolean }) {
  const actions = useActions();
  const dark = useDarkTheme();
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
    <div className="dock-host" data-tt-role="dock" data-tt-id="dock">
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
