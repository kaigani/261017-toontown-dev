// The Properties panel's Drawing page, the drawing inspector (02 §9 lines 858-872; M4 C4.4a, reviews/M4-design-panels.md
// §6). Its subject until M5 is the current element's drawing at the current frame (SI-168): the element and exposure
// (read only), the drawing's name (`draw.drawing.rename` on Enter or blur), Show preview (`draw.inspectorPreview.set`, a
// preference), the thumbnail (`draw.drawing.render`) and the Browse slider over the element's drawings, which shows
// another drawing's thumbnail without changing anything (`element.drawing.list` is its read).
import { useCallback, useEffect, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import { enabledIn, useAvailability } from '../shell/useAvailability';
import { useShell, useSlice } from '../state/context';
import type { AppState } from '../state/types';

type Json = Record<string, unknown>;
const obj = (v: unknown): Json => (typeof v === 'object' && v !== null ? (v as Json) : {});

interface Drawing {
  id: string;
  name: string;
  exposures: { from: number; to: number }[];
}

const PREVIEW_PREF = 'drawing.inspectorPreview';

export function DrawingInspectorPage() {
  const { engine } = useShell();
  const actions = useActions();
  const app = useSlice('appState') as AppState | null | undefined;
  const element = app?.element ?? null;
  const frame = app?.frame ?? null;
  const [drawings, setDrawings] = useState<Drawing[]>([]);
  const [preview, setPreview] = useState(true);
  const [browse, setBrowse] = useState<number | null>(null);
  const [thumb, setThumb] = useState<string | null>(null);
  const current =
    drawings.find((d) => frame !== null && d.exposures.some((w) => w.from <= frame && frame <= w.to)) ?? null;
  const refresh = useCallback(() => {
    void engine
      .run('prefs.get', { keys: [PREVIEW_PREF] })
      .done.then((env) => {
        const v = obj(obj(env.result)['values'])[PREVIEW_PREF];
        setPreview(v !== false);
      })
      .catch(() => undefined);
    // Without an element the page shows nothing of a drawing (it returns early), so nothing is read.
    if (!element) return;
    void engine
      .run('element.drawing.list', { elementId: element.id })
      .done.then((env) => {
        const list = obj(env.result)['drawings'];
        setDrawings(env.status === 'ok' && Array.isArray(list) ? (list as Drawing[]) : []);
      })
      .catch(() => undefined);
  }, [engine, element]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    void engine
      .onEvent((e) => {
        if (
          e.name === 'draw.drawing.changed' ||
          e.name === 'doc.changed' ||
          e.name === 'doc.undone' ||
          e.name === 'doc.redone' ||
          e.name === 'prefs.changed' ||
          e.name === 'project.opened' ||
          e.name === 'project.closed'
        )
          refresh();
      })
      .then((u) => {
        if (stopped) u();
        else off = u;
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, refresh]);
  // The thumbnail: the current drawing, or the one the Browse slider points at.
  const shown = browse !== null ? (drawings[browse] ?? null) : current;
  useEffect(() => {
    if (!preview || !shown) return;
    let live = true;
    void engine
      .run('draw.drawing.render', {
        drawingId: shown.id,
        width: 160,
        region: 'bounds',
        background: '#FFFFFFFF',
      })
      .done.then((env) => {
        const png = obj(env.result)['png'];
        if (live) setThumb(env.status === 'ok' && typeof png === 'string' ? png : null);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [engine, preview, shown]);
  const previewRef = { commandId: 'draw.inspectorPreview.set', params: { visible: !preview } };
  const availability = useAvailability([previewRef]);

  if (!element || !current)
    return (
      <div className="drawing-inspector" data-tt-role="group" data-tt-id="inspector" data-tt-label="Drawing">
        <p className="empty">
          {element ? 'No drawing is exposed at this frame.' : 'No drawing element is current.'}
        </p>
      </div>
    );
  const exposure = current.exposures.map((w) =>
    w.from === w.to ? String(w.from) : `${String(w.from)}-${String(w.to)}`,
  );
  return (
    <div className="drawing-inspector" data-tt-role="group" data-tt-id="inspector" data-tt-label="Drawing">
      <p data-tt-role="text" data-tt-id="inspector.element">
        {element.name} · frames {exposure.join(', ')}
      </p>
      <NameField key={current.id} drawing={current} />
      <label>
        <input
          type="checkbox"
          checked={preview}
          disabled={!enabledIn(availability(previewRef))}
          data-tt-role="checkbox"
          data-tt-id="inspector.preview"
          data-tt-command="draw.inspectorPreview.set"
          data-tt-params={JSON.stringify(previewRef.params)}
          onChange={() => {
            void actions.run('draw.inspectorPreview.set', previewRef.params);
          }}
        />
        Show preview
      </label>
      {preview && thumb && (
        <img
          className="inspector-thumbnail"
          alt={`Preview of ${shown?.name ?? ''}`}
          src={`data:image/png;base64,${thumb}`}
          data-tt-role="image"
          data-tt-id="inspector.thumbnail"
        />
      )}
      {drawings.length > 1 && (
        <label>
          Browse
          <input
            type="range"
            min={0}
            max={drawings.length - 1}
            step={1}
            value={browse ?? Math.max(0, drawings.indexOf(current))}
            aria-label="Browse the element's drawings"
            data-tt-role="slider"
            data-tt-id="inspector.browse"
            data-tt-gui-only="navigate"
            onChange={(e) => {
              setBrowse(Number(e.target.value));
            }}
          />
          <span data-tt-role="text" data-tt-id="inspector.browsing">
            {shown?.name ?? ''}
          </span>
        </label>
      )}
    </div>
  );
}

/** The drawing's name, committed with `draw.drawing.rename` on Enter or blur ("Rename Drawing"); a refusal shows under
 *  the field; Escape keeps the name. */
function NameField({ drawing }: { drawing: Drawing }) {
  const actions = useActions();
  // The draft belongs to the name it was typed from: a rename from elsewhere shows at once.
  const [typed, setTyped] = useState<{ from: string; text: string } | null>(null);
  const draft = typed?.from === drawing.name ? typed.text : drawing.name;
  const setDraft = (text: string) => {
    setTyped({ from: drawing.name, text });
  };
  const [problem, setProblem] = useState<string | null>(null);
  const commit = () => {
    const name = draft.trim();
    if (name === drawing.name || name === '') {
      setTyped(null);
      return;
    }
    void actions
      .run('draw.drawing.rename', { drawingId: drawing.id, name }, [
        'drawing.nameInvalid',
        'drawing.nameExists',
      ])
      .then((env) => {
        setProblem(env.status === 'error' ? (env.error?.message ?? 'refused') : null);
      });
  };
  return (
    <label>
      Name
      <input
        value={draft}
        data-tt-role="textField"
        data-tt-id="inspector.name"
        data-tt-command="draw.drawing.rename"
        data-tt-params={JSON.stringify({ drawingId: drawing.id, name: draft.trim() })}
        onChange={(e) => {
          setDraft(e.target.value);
          setProblem(null);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit();
          if (e.key === 'Escape') {
            setTyped(null);
            setProblem(null);
          }
        }}
        onBlur={commit}
      />
      {problem && (
        <span className="field-problem" role="alert" data-tt-role="text" data-tt-id="inspector.nameProblem">
          {problem}
        </span>
      )}
    </label>
  );
}
