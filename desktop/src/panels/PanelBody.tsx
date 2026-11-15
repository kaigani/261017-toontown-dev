// Panel contents (01 §5.2). C4.1 implements the History panel and the view windows' chrome; every other panel is an
// honest placeholder that names the checkpoint bringing its content, where a user would look for it.
import { useEffect, useRef, useState } from 'react';

import { CanvasView } from '../canvas/CanvasView';
import { ViewChrome } from './ViewChrome';
import { useActions } from '../shell/actionsApi';
import { useShell, useSlice } from '../state/context';
import type { History } from '../state/types';
import { panelInfo } from './catalogue';

export function PanelBody({ panelId }: { panelId: string }) {
  const info = panelInfo(panelId);
  if (panelId === 'history') return <HistoryPanel />;
  if (info?.view) return <ViewPanel panelId={panelId} />;
  return (
    <div
      className="panel-placeholder"
      data-tt-role="panel"
      data-tt-id={panelId}
      data-tt-label={info?.title ?? panelId}
    >
      <h2>{info?.title ?? panelId}</h2>
      {panelId === 'properties' && <p className="no-properties">No properties</p>}
      <p data-tt-role="text" data-tt-id={`${panelId}.owner`}>
        {info?.owner ? `This panel's content arrives with ${info.owner}.` : ''}
      </p>
    </div>
  );
}

/** The History panel (01 §10): steps in order, the current position, the saved position, agent attribution; a click
 *  undoes or redoes to just after that step in one combined jump (`edit.history.move`). */
export function HistoryPanel() {
  const actions = useActions();
  const history = useSlice('history') as History | null | undefined;
  const entries = history?.entries ?? [];
  return (
    <div className="history-panel" data-tt-role="panel" data-tt-id="history" data-tt-label="History">
      {!history && <p className="empty">No project is open.</p>}
      {history && (
        <ol>
          <li>
            <button
              type="button"
              className={history.position === 0 ? 'history-entry current' : 'history-entry'}
              data-tt-role="button"
              data-tt-id="history.0"
              data-tt-command="edit.history.move"
              data-tt-params={JSON.stringify({ position: 0 })}
              aria-current={history.position === 0 ? 'step' : undefined}
              onClick={() => {
                void actions.run('edit.history.move', { position: 0 });
              }}
            >
              Initial state{history.cleanPosition === 0 ? ' (saved)' : ''}
            </button>
          </li>
          {entries.map((e) => {
            const position = e.index + 1;
            const after = position > history.position;
            return (
              <li key={e.index}>
                <button
                  type="button"
                  className={`history-entry${position === history.position ? ' current' : ''}${after ? ' dimmed' : ''}${e.source === 'agent' ? ' agent' : ''}`}
                  aria-current={position === history.position ? 'step' : undefined}
                  data-tt-role="button"
                  data-tt-id={`history.${String(position)}`}
                  data-tt-command="edit.history.move"
                  data-tt-params={JSON.stringify({ position })}
                  onClick={() => {
                    void actions.run('edit.history.move', { position });
                  }}
                >
                  <span className="label">{e.label}</span>
                  <span className="source">
                    {e.source === 'agent' ? `agent: ${e.agentName ?? 'agent'}` : 'user'}
                  </span>
                  <time>{e.time}</time>
                  {history.cleanPosition === position && <span className="saved"> (saved)</span>}
                </button>
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
}

const VIEW_TYPES = [
  { type: 'drawing', label: 'Drawing' },
  { type: 'camera', label: 'Camera' },
  { type: 'side', label: 'Side' },
  { type: 'top', label: 'Top' },
] as const;

/** A view window's chrome (01 §5.3): view-type controls and navigation; the canvas arrives with M4 C4.2. The canvas
 *  size is reported to the executor (`view.list` sizes). */
export function ViewPanel({ panelId }: { panelId: string }) {
  const { engine } = useShell();
  const actions = useActions();
  const canvas = useRef<HTMLDivElement | null>(null);
  const info = panelInfo(panelId);
  // The Drawing and Camera view windows show the canvas, which reports its own size (M4 C4.2b, C4.3b; camera-view note
  // §6: it follows the scene answer's view type); the Top and Side windows keep the placeholder (M6 C6.1).
  const drawing = panelId === 'drawingView' || panelId === 'cameraView';
  // The window's current type (`view.list`), shown on the type buttons (ruling D-30, group 1).
  const [viewType, setViewType] = useState<string | null>(null);
  useEffect(() => {
    let stopped = false;
    let off: (() => void) | null = null;
    const read = () => {
      void engine
        .run('view.list', {})
        .done.then((env) => {
          const views =
            (env.result as { views?: { viewId?: string; type?: string }[] } | undefined)?.views ?? [];
          const mine = views.find((v) => v.viewId === panelId);
          if (!stopped && mine?.type) setViewType(mine.type);
        })
        .catch(() => undefined);
    };
    read();
    void engine
      .onEvent((e) => {
        if (e.name === 'shell.view' || e.name === 'project.opened') read();
      })
      .then((u) => {
        if (stopped) u();
        else off = u;
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, panelId]);
  useEffect(() => {
    const el = canvas.current;
    if (drawing || !el || typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (r && r.width >= 1 && r.height >= 1) {
        void engine
          .report({ viewSizes: { [panelId]: { width: r.width, height: r.height } } })
          .catch(() => undefined);
      }
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [engine, panelId, drawing]);
  return (
    <div
      className="view-panel"
      data-tt-role="panel"
      data-tt-id={panelId}
      data-tt-label={info?.title ?? panelId}
    >
      <div className="view-toolbar" role="toolbar" data-tt-role="toolbar" data-tt-id={`${panelId}.toolbar`}>
        {VIEW_TYPES.map((v) => (
          <button
            key={v.type}
            type="button"
            aria-pressed={viewType === v.type}
            title={`Show the ${v.label} view in this window (view.type.set)`}
            data-tt-role="button"
            data-tt-id={`${panelId}.type.${v.type}`}
            data-tt-command="view.type.set"
            data-tt-params={JSON.stringify({ viewId: panelId, type: v.type })}
            onClick={() => {
              void actions.run('view.type.set', { viewId: panelId, type: v.type });
            }}
          >
            {v.label}
          </button>
        ))}
        <span className="spacer" />
        <button
          type="button"
          data-tt-role="button"
          data-tt-id={`${panelId}.focus`}
          data-tt-command="window.panel.set"
          onClick={() => {
            void actions.run('window.panel.set', { panelId, focus: true });
          }}
        >
          Focus
        </button>
      </div>
      {drawing && <ViewChrome viewId={panelId} />}
      {drawing ? (
        <CanvasView engine={engine} viewId={panelId} />
      ) : (
        <div ref={canvas} className="view-canvas" data-tt-role="canvas" data-tt-id={`${panelId}.canvas`}>
          <p>{info?.owner ? `The interactive canvas arrives with ${info.owner}.` : ''}</p>
        </div>
      )}
    </div>
  );
}
