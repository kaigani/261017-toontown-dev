// The Pen panel (02 §6.1 lines 354-384, §6.2 lines 386-398; M4 C4.4a, reviews/M4-design-panels.md §3): the pens of
// the active tool's type (Brush or Eraser), the current one pressed; New, Delete and in-place rename; the current pen's
// settings (tip shape, minimum and maximum size, smoothness, tip angle), each committed on release or on Enter with one
// `tool.pen.set` ("Set Pen Style"); a preview stroke drawn from the pen's values; and, behind a disclosure, symmetry and
// Draw Behind. Every control runs its command through the one entry point and shows the kernel's state, re-read on the
// kernel's events (an agent's change shows too); the push rule of minimum and maximum is the kernel's, never the panel's.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import type { CommandRef } from '../shell/availability';
import { optionRef, runChosen } from '../shell/listControl';
import { enabledIn, useAvailability } from '../shell/useAvailability';
import { useShell } from '../state/context';

type Json = Record<string, unknown>;
const obj = (v: unknown): Json => (typeof v === 'object' && v !== null ? (v as Json) : {});

export interface Pen {
  penId: string;
  name: string;
  type: 'brush' | 'eraser';
  tipShape: string;
  minSize: number;
  maxSize: number;
  smoothness: number;
  tipAngle: number;
}

const TIP_SHAPES = [
  { value: 'round', label: 'Round' },
  { value: 'oval', label: 'Oval' },
  { value: 'square', label: 'Square' },
  { value: 'flat', label: 'Flat' },
];

/** The pen settings with a slider: key, label, range and step (02 §6.1). */
const SETTINGS = [
  { key: 'minSize', label: 'Minimum size', min: 0, max: 100, step: 0.5 },
  { key: 'maxSize', label: 'Maximum size', min: 0, max: 100, step: 0.5 },
  { key: 'smoothness', label: 'Smoothness', min: 0, max: 10, step: 1 },
  { key: 'tipAngle', label: 'Tip angle', min: 0, max: 360, step: 1 },
] as const;

function pensOf(v: unknown): Pen[] {
  const list = obj(v)['pens'];
  return Array.isArray(list) ? (list.filter((p) => typeof obj(p)['penId'] === 'string') as Pen[]) : [];
}

/** The family the panel lists: the Eraser's pens while the Eraser is active, else the Brush's (02 §6.1). */
function familyOf(toolId: unknown): 'brush' | 'eraser' {
  return toolId === 'eraser' ? 'eraser' : 'brush';
}

export function PenPanel() {
  const { engine } = useShell();
  const actions = useActions();
  const [pens, setPens] = useState<Pen[]>([]);
  const [current, setCurrent] = useState<{ brush: string | null; eraser: string | null }>({
    brush: null,
    eraser: null,
  });
  const [family, setFamily] = useState<'brush' | 'eraser'>('brush');
  const [placeBehind, setPlaceBehind] = useState(false);
  const [open, setOpen] = useState(false);
  const refresh = useCallback(() => {
    void engine
      .run('tool.pen.list', {})
      .done.then((env) => {
        if (env.status !== 'ok') {
          setPens([]);
          return;
        }
        const r = obj(env.result);
        setPens(pensOf(r));
        setCurrent({
          brush: typeof r['currentBrushPenId'] === 'string' ? r['currentBrushPenId'] : null,
          eraser: typeof r['currentEraserPenId'] === 'string' ? r['currentEraserPenId'] : null,
        });
      })
      .catch(() => undefined);
    void engine
      .run('tool.active.get', {})
      .done.then((env) => {
        if (env.status === 'ok') setFamily(familyOf(obj(env.result)['toolId']));
      })
      .catch(() => undefined);
    void engine
      .run('tool.settings.get', {})
      .done.then((env) => {
        if (env.status === 'ok') setPlaceBehind(obj(env.result)['placeBehind'] === true);
      })
      .catch(() => undefined);
  }, [engine]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    void engine
      .onEvent((e) => {
        if (
          e.name === 'tool.pen.changed' ||
          e.name === 'tool.options.changed' ||
          e.name === 'tool.active.changed' ||
          e.name === 'doc.undone' ||
          e.name === 'doc.redone' ||
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

  const shown = pens.filter((p) => p.type === family);
  const currentId = current[family];
  const pen = shown.find((p) => p.penId === currentId) ?? null;
  const { createRef, deleteRef, behindRef, refs } = useMemo(() => {
    const create: CommandRef = {
      commandId: 'tool.pen.create',
      params: { type: family, ...(currentId ? { fromPenId: currentId } : {}) },
    };
    const del: CommandRef | null = currentId
      ? { commandId: 'tool.pen.delete', params: { penId: currentId } }
      : null;
    const behind: CommandRef = { commandId: 'tool.placeBehind.set', params: { enabled: !placeBehind } };
    return {
      createRef: create,
      deleteRef: del,
      behindRef: behind,
      refs: [create, behind, ...(del ? [del] : [])],
    };
  }, [family, currentId, placeBehind]);
  const availability = useAvailability(refs);

  return (
    <div className="pen-panel" data-tt-role="panel" data-tt-id="pen" data-tt-label="Pen">
      {pens.length === 0 && <p className="empty">No project is open.</p>}
      <ul className="pen-list" role="listbox" aria-label={family === 'eraser' ? 'Eraser pens' : 'Brush pens'}>
        {shown.map((p) => (
          <PenRow key={p.penId} pen={p} current={p.penId === currentId} />
        ))}
      </ul>
      <div className="buttons">
        <button
          type="button"
          disabled={!enabledIn(availability(createRef))}
          data-tt-role="button"
          data-tt-id="pen.new"
          data-tt-command="tool.pen.create"
          data-tt-params={JSON.stringify(createRef.params)}
          onClick={() => {
            void actions.run('tool.pen.create', createRef.params);
          }}
        >
          New
        </button>
        <button
          type="button"
          disabled={!deleteRef || !enabledIn(availability(deleteRef))}
          title={deleteRef ? (availability(deleteRef)?.reason ?? 'Delete Pen') : 'Delete Pen'}
          data-tt-role="button"
          data-tt-id="pen.delete"
          data-tt-command="tool.pen.delete"
          data-tt-params={JSON.stringify(deleteRef?.params ?? {})}
          onClick={() => {
            if (deleteRef) void actions.run('tool.pen.delete', deleteRef.params);
          }}
        >
          Delete
        </button>
      </div>
      {pen && <PenSettings pen={pen} />}
      {pen && <PenPreview pen={pen} />}
      <details
        className="pen-advanced"
        open={open}
        onToggle={(e) => {
          setOpen((e.target as HTMLDetailsElement).open);
        }}
      >
        <summary data-tt-role="button" data-tt-id="pen.advanced" data-tt-gui-only="disclose">
          Advanced options
        </summary>
        <label>
          <input
            type="checkbox"
            checked={placeBehind}
            disabled={!enabledIn(availability(behindRef))}
            data-tt-role="checkbox"
            data-tt-id="pen.placeBehind"
            data-tt-command="tool.placeBehind.set"
            data-tt-params={JSON.stringify(behindRef.params)}
            onChange={() => {
              // The shown state follows the kernel's `tool.options.changed`, not this click.
              void actions.run('tool.placeBehind.set', behindRef.params);
            }}
          />
          Draw Behind
        </label>
      </details>
    </div>
  );
}

/** One pen of the list: a click selects it (`tool.pen.select`, no undo step); a double click or F2 renames it in place
 *  (`tool.pen.rename` on Enter or blur; Escape keeps the name; the same name sends nothing). */
function PenRow({ pen, current }: { pen: Pen; current: boolean }) {
  const actions = useActions();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(pen.name);
  const [problem, setProblem] = useState<string | null>(null);
  const done = useRef(false);
  const commit = () => {
    if (done.current) return;
    done.current = true;
    const name = draft.trim();
    if (name === '' || name === pen.name) {
      setEditing(false);
      setProblem(null);
      return;
    }
    void actions.run('tool.pen.rename', { penId: pen.penId, name }, ['pen.nameExists']).then((env) => {
      if (env.status === 'error' && env.error?.code === 'pen.nameExists') {
        setProblem(env.error.message);
        done.current = false;
        return;
      }
      setEditing(false);
      setProblem(null);
    });
  };
  if (editing)
    return (
      <li className="pen-row editing">
        <input
          autoFocus
          value={draft}
          aria-label={`Rename ${pen.name}`}
          data-tt-role="textField"
          data-tt-id="pen.rename"
          data-tt-command="tool.pen.rename"
          data-tt-params={JSON.stringify({ penId: pen.penId, name: draft.trim() })}
          onChange={(e) => {
            setDraft(e.target.value);
            setProblem(null);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            if (e.key === 'Escape') {
              e.stopPropagation();
              done.current = true;
              setEditing(false);
              setDraft(pen.name);
              setProblem(null);
            }
          }}
          onBlur={commit}
        />
        {problem && (
          <p className="field-problem" role="alert" data-tt-role="text" data-tt-id="pen.renameProblem">
            {problem}
          </p>
        )}
      </li>
    );
  return (
    <li className={current ? 'pen-row current' : 'pen-row'}>
      <button
        type="button"
        role="option"
        aria-selected={current}
        data-tt-role="button"
        data-tt-id={`pen.row.${pen.penId}`}
        data-tt-label={pen.name}
        data-tt-command="tool.pen.select"
        data-tt-params={JSON.stringify({ penId: pen.penId })}
        onClick={() => {
          void actions.run('tool.pen.select', { penId: pen.penId });
        }}
        onDoubleClick={() => {
          done.current = false;
          setDraft(pen.name);
          setEditing(true);
        }}
        onKeyDown={(e) => {
          if (e.key === 'F2') {
            e.preventDefault();
            done.current = false;
            setDraft(pen.name);
            setEditing(true);
          }
        }}
      >
        {pen.name}
      </button>
    </li>
  );
}

/** The current pen's settings: a tip shape list, and per value a slider with a numeric field. A slider sends nothing
 *  while dragged and one `tool.pen.set` at release; a field commits on Enter or blur; the shown values are the kernel's. */
function PenSettings({ pen }: { pen: Pen }) {
  const actions = useActions();
  return (
    <div className="pen-settings" data-tt-role="group" data-tt-id="pen.settings">
      <label>
        Tip shape
        <select
          value={pen.tipShape}
          data-tt-role="list"
          data-tt-id="pen.tipShape"
          data-tt-command="tool.pen.set"
          onChange={runChosen(actions)}
        >
          {TIP_SHAPES.map((t) => (
            <option
              key={t.value}
              value={t.value}
              {...optionRef('pen.tipShape', t.value, t.label, 'tool.pen.set', {
                penId: pen.penId,
                tipShape: t.value,
              })}
            >
              {t.label}
            </option>
          ))}
        </select>
      </label>
      {SETTINGS.map((s) => (
        <PenValue key={s.key} pen={pen} setting={s} />
      ))}
    </div>
  );
}

function PenValue({ pen, setting }: { pen: Pen; setting: (typeof SETTINGS)[number] }) {
  const actions = useActions();
  const value = pen[setting.key];
  // Drafts belong to the value they were typed or slid from: a new value from the kernel shows at once.
  const [typed, setTyped] = useState<{ from: number; text: string } | null>(null);
  const [slid, setSlid] = useState<{ from: number; v: number } | null>(null);
  const draft = typed?.from === value ? typed.text : String(value);
  const sliding = slid?.from === value ? slid.v : null;
  const setDraft = (text: string) => {
    setTyped({ from: value, text });
  };
  const setSliding = (v: number) => {
    setSlid({ from: value, v });
  };
  const send = (v: number) => {
    if (!Number.isFinite(v) || v === value) return;
    void actions.run('tool.pen.set', { penId: pen.penId, [setting.key]: v });
  };
  const id = `pen.${setting.key}`;
  return (
    <div className="pen-value">
      <label htmlFor={id}>{setting.label}</label>
      <input
        id={id}
        type="range"
        min={setting.min}
        max={setting.max}
        step={setting.step}
        value={sliding ?? value}
        data-tt-role="slider"
        data-tt-id={id}
        data-tt-command="tool.pen.set"
        data-tt-params={JSON.stringify({ penId: pen.penId, [setting.key]: value })}
        onChange={(e) => {
          setSliding(Number(e.target.value));
        }}
        onPointerUp={() => {
          if (sliding !== null) send(sliding);
        }}
        onKeyUp={() => {
          if (sliding !== null) send(sliding);
        }}
      />
      <input
        type="number"
        min={setting.min}
        max={setting.max}
        step={setting.step}
        value={draft}
        aria-label={setting.label}
        data-tt-role="textField"
        data-tt-id={`${id}.field`}
        data-tt-command="tool.pen.set"
        data-tt-params={JSON.stringify({ penId: pen.penId, [setting.key]: value })}
        onChange={(e) => {
          setDraft(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') send(Number(draft));
          if (e.key === 'Escape') setDraft(String(value));
        }}
        onBlur={() => {
          send(Number(draft));
        }}
      />
    </div>
  );
}

/** A preview stroke from the pen's values (02 §13.2 lines 1182-1183: no render call): a pressure ramp from the minimum
 *  to the maximum size, with the tip's shape and angle, drawn in the panel. */
function PenPreview({ pen }: { pen: Pen }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    const g = c?.getContext('2d');
    if (!c || !g) return;
    g.clearRect(0, 0, c.width, c.height);
    g.fillStyle = 'currentColor';
    const n = 40;
    const scale = (c.height - 8) / 100;
    for (let i = 0; i <= n; i++) {
      const t = i / n;
      const x = 8 + t * (c.width - 16);
      const y = c.height / 2 + Math.sin(t * Math.PI * 2) * (c.height / 6);
      const size = Math.max(0.5, (pen.minSize + (pen.maxSize - pen.minSize) * t) * scale);
      g.save();
      g.translate(x, y);
      g.rotate((pen.tipAngle * Math.PI) / 180);
      if (pen.tipShape === 'square') g.fillRect(-size / 2, -size / 2, size, size);
      else if (pen.tipShape === 'flat') g.fillRect(-size / 2, -size / 8, size, size / 4);
      else {
        g.beginPath();
        g.ellipse(0, 0, size / 2, pen.tipShape === 'oval' ? size / 4 : size / 2, 0, 0, Math.PI * 2);
        g.fill();
      }
      g.restore();
    }
  }, [pen.minSize, pen.maxSize, pen.tipShape, pen.tipAngle]);
  return (
    <canvas
      ref={ref}
      className="pen-preview"
      width={220}
      height={48}
      role="img"
      aria-label={`Preview of ${pen.name}`}
      data-tt-role="image"
      data-tt-id="pen.preview"
    />
  );
}
