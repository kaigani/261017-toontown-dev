// The Properties panel's Tool options page (02 §7.7 lines 701-720, §8.8-§8.11 lines 786-810, §12.3; M4 C4.4a,
// reviews/M4-design-panels.md §4): the active tool's options and the three persistent toggles. Select: the rectangle
// rule. Brush and Eraser: their pen. Pencil, Rectangle, Ellipse, Line and Polyline: the line thickness (the kernel writes
// the five together); Polyline also snapping. Paint, Unpaint and Paint Unpainted: automatic gap closing (03 §8.8).
// Contour Editor (M4 C4.4b; SI-196): the line properties of the one selected pencil line or invisible stroke, its
// thickness and whether it is invisible (`draw.stroke.set`, "Edit Stroke"; 02 §8 line properties, line 1326). Every
// control runs one command and shows the kernel's state, re-read on its events; nothing is stored here (02 lines
// 1111-1113: the kernel persists the options).
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import type { CommandRef } from '../shell/availability';
import { optionRef, runChosen } from '../shell/listControl';
import { declaredRange } from '../shell/numeric';
import { NumericField } from '../shell/NumericField';
import { enabledIn, useAvailability } from '../shell/useAvailability';
import { useShell } from '../state/context';

type Json = Record<string, unknown>;
const obj = (v: unknown): Json => (typeof v === 'object' && v !== null ? (v as Json) : {});

const THICKNESS_TOOLS = ['pencil', 'rectangle', 'ellipse', 'line', 'polyline'];
const PAINT_TOOLS = ['paint', 'unpaint', 'paintUnpainted'];
const GAP_MODES = [
  { value: 'off', label: 'Off' },
  { value: 'small', label: 'Small gaps' },
  { value: 'medium', label: 'Medium gaps' },
  { value: 'large', label: 'Large gaps' },
];

interface PenRow {
  penId: string;
  name: string;
  type: string;
}

export function ToolOptionsPage() {
  const { engine } = useShell();
  const actions = useActions();
  const [tool, setTool] = useState<string | null>(null);
  const [options, setOptions] = useState<Json>({});
  const [settings, setSettings] = useState<Json>({});
  const [gap, setGap] = useState<string>('off');
  const [pens, setPens] = useState<{ list: PenRow[]; brush: string | null; eraser: string | null }>({
    list: [],
    brush: null,
    eraser: null,
  });
  const refresh = useCallback(() => {
    void engine
      .run('tool.active.get', {})
      .done.then((env) => {
        const t = env.status === 'ok' ? obj(env.result)['toolId'] : null;
        const id = typeof t === 'string' ? t : null;
        setTool(id);
        if (!id) return;
        void engine
          .run('tool.options.get', { toolId: id })
          .done.then((o) => {
            setOptions(o.status === 'ok' ? obj(obj(o.result)['options']) : {});
          })
          .catch(() => undefined);
      })
      .catch(() => undefined);
    void engine
      .run('tool.settings.get', {})
      .done.then((env) => {
        if (env.status === 'ok') setSettings(obj(env.result));
      })
      .catch(() => undefined);
    void engine
      .run('colour.gapClosing.get', {})
      .done.then((env) => {
        const m = env.status === 'ok' ? obj(env.result)['mode'] : null;
        if (typeof m === 'string') setGap(m);
      })
      .catch(() => undefined);
    void engine
      .run('tool.pen.list', {})
      .done.then((env) => {
        const r = obj(env.result);
        const list = Array.isArray(r['pens']) ? (r['pens'] as PenRow[]) : [];
        setPens({
          list: env.status === 'ok' ? list : [],
          brush: typeof r['currentBrushPenId'] === 'string' ? r['currentBrushPenId'] : null,
          eraser: typeof r['currentEraserPenId'] === 'string' ? r['currentEraserPenId'] : null,
        });
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
          e.name === 'tool.active.changed' ||
          e.name === 'tool.options.changed' ||
          e.name === 'tool.pen.changed' ||
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

  const placeBehind = settings['placeBehind'] === true;
  const toggles = useMemo(
    () => [
      {
        id: 'drawTopLayer',
        label: 'Draw on Top Layer',
        ref: { commandId: 'tool.drawTopLayer.set', params: { enabled: settings['drawTopLayer'] !== true } },
        on: settings['drawTopLayer'] === true,
      },
      {
        id: 'placeBehind',
        label: 'Draw Behind',
        ref: { commandId: 'tool.placeBehind.set', params: { enabled: !placeBehind } },
        on: placeBehind,
      },
      {
        id: 'snapToContour',
        label: 'Snap to Contour',
        ref: { commandId: 'tool.snapToContour.set', params: { enabled: settings['snapToContour'] !== true } },
        on: settings['snapToContour'] === true,
      },
    ],
    [settings, placeBehind],
  );
  const refs: CommandRef[] = useMemo(() => toggles.map((t) => t.ref), [toggles]);
  const availability = useAvailability(refs);

  return (
    <div className="tool-options" data-tt-role="group" data-tt-id="toolOptions" data-tt-label="Tool options">
      {tool === null && <p className="empty">No project is open.</p>}
      {tool === 'select' && (
        <ChoiceList
          id="toolOptions.rectRule"
          label="Rectangle selects"
          value={options['rectRule'] === 'intersect' ? 'intersect' : 'inside'}
          choices={[
            { value: 'inside', label: 'Objects inside' },
            { value: 'intersect', label: 'Objects it touches' },
          ]}
          command="tool.select.options.set"
          params={(v) => ({ rectRule: v })}
          onChange={runChosen(actions)}
        />
      )}
      {(tool === 'brush' || tool === 'eraser') && (
        <ChoiceList
          id="toolOptions.pen"
          label="Pen"
          value={(tool === 'brush' ? pens.brush : pens.eraser) ?? ''}
          choices={pens.list.filter((p) => p.type === tool).map((p) => ({ value: p.penId, label: p.name }))}
          command={tool === 'brush' ? 'tool.brush.options.set' : 'tool.eraser.options.set'}
          params={(v) => ({ penId: v })}
          onChange={runChosen(actions)}
        />
      )}
      {tool !== null && THICKNESS_TOOLS.includes(tool) && <Thickness options={options} />}
      {tool === 'polyline' && <PolylineSnap options={options} />}
      {tool === 'contourEditor' && <SelectedLine />}
      {tool !== null && PAINT_TOOLS.includes(tool) && (
        <ChoiceList
          id="toolOptions.gapClosing"
          label="Automatic gap closing"
          value={gap}
          choices={GAP_MODES}
          command="colour.gapClosing.set"
          params={(v) => ({ mode: v })}
          onChange={runChosen(actions)}
        />
      )}
      {tool !== null && (
        <div className="tool-toggles" role="group" aria-label="Drawing settings">
          {toggles.map((t) => {
            // 02 line 810: Draw Behind wins when both are on, so Draw on Top Layer is off while Draw Behind is.
            const blocked = t.id === 'drawTopLayer' && placeBehind;
            return (
              <label
                key={t.id}
                title={blocked ? 'Draw Behind is on: it takes precedence (02 §8.10)' : t.label}
              >
                <input
                  type="checkbox"
                  checked={t.on}
                  disabled={blocked || !enabledIn(availability(t.ref))}
                  data-tt-role="checkbox"
                  data-tt-id={`toolOptions.${t.id}`}
                  data-tt-command={t.ref.commandId}
                  data-tt-params={JSON.stringify(t.ref.params)}
                  onChange={() => {
                    // The shown state follows the kernel's `tool.options.changed`, not this click.
                    void actions.run(t.ref.commandId, t.ref.params);
                  }}
                />
                {t.label}
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** A list whose options each declare the reference choosing them sends (C43B-R2-03). */
function ChoiceList(props: {
  id: string;
  label: string;
  value: string;
  choices: { value: string; label: string }[];
  command: string;
  params: (value: string) => Json;
  onChange: ReturnType<typeof runChosen>;
}) {
  return (
    <label className="tool-option">
      {props.label}
      <select
        value={props.value}
        data-tt-role="list"
        data-tt-id={props.id}
        data-tt-command={props.command}
        onChange={props.onChange}
      >
        {props.choices.map((c) => (
          <option
            key={c.value}
            value={c.value}
            {...optionRef(props.id, c.value, c.label, props.command, props.params(c.value))}
          >
            {c.label}
          </option>
        ))}
      </select>
    </label>
  );
}

/** Line thickness (02 §7.7): from the pen's maximum size, or a fixed value 0-100 du committed on Enter or blur. */
function Thickness({ options }: { options: Json }) {
  const actions = useActions();
  const source = options['thicknessSource'] === 'fixed' ? 'fixed' : 'penMax';
  const value = typeof options['thickness'] === 'number' ? options['thickness'] : 1;
  return (
    <div className="tool-option">
      <ChoiceList
        id="toolOptions.thicknessSource"
        label="Line thickness"
        value={source}
        choices={[
          { value: 'penMax', label: "The pen's maximum size" },
          { value: 'fixed', label: 'Fixed' },
        ]}
        command="tool.pencil.options.set"
        params={(v) =>
          v === 'fixed' ? { thicknessSource: 'fixed', thickness: value } : { thicknessSource: 'penMax' }
        }
        onChange={runChosen(actions)}
      />
      <NumericField
        id="toolOptions.thickness"
        label="Thickness"
        value={value}
        range={declaredRange('tool.pencil.options.set', 'thickness')}
        step={0.5}
        disabled={source !== 'fixed'}
        command="tool.pencil.options.set"
        params={{ thicknessSource: 'fixed', thickness: value }}
        onCommit={(v) =>
          actions.run('tool.pencil.options.set', { thicknessSource: 'fixed', thickness: v }, ['arg.invalid'])
        }
      />
    </div>
  );
}

/** Polyline snapping (02 §7.7): on or off, and the radius 1-50 view pixels committed on Enter or blur. */
function PolylineSnap({ options }: { options: Json }) {
  const actions = useActions();
  const snap = options['snap'] !== false;
  const radius = typeof options['snapRadius'] === 'number' ? options['snapRadius'] : 8;
  return (
    <div className="tool-option">
      <label>
        <input
          type="checkbox"
          checked={snap}
          data-tt-role="checkbox"
          data-tt-id="toolOptions.snap"
          data-tt-command="tool.polyline.options.set"
          data-tt-params={JSON.stringify({ snap: !snap })}
          onChange={() => {
            void actions.run('tool.polyline.options.set', { snap: !snap });
          }}
        />
        Snap to points
      </label>
      <NumericField
        id="toolOptions.snapRadius"
        label="Snap radius"
        value={radius}
        range={declaredRange('tool.polyline.options.set', 'snapRadius')}
        command="tool.polyline.options.set"
        params={{ snapRadius: radius }}
        onCommit={(v) => actions.run('tool.polyline.options.set', { snapRadius: v }, ['arg.invalid'])}
      />
    </div>
  );
}

/** The line properties of the one selected pencil line or invisible stroke (SI-196): its thickness and its visibility,
 *  each one `draw.stroke.set` ("Edit Stroke"), re-read on the drawing's changes and the selection's. */
function SelectedLine() {
  const { engine } = useShell();
  const actions = useActions();
  const [line, setLine] = useState<{ strokeId: string; kind: string; thickness: number } | null>(null);
  const refresh = useCallback(() => {
    void (async () => {
      const sel = await engine.run('draw.selection.get', {}).done;
      const r = obj(sel.result);
      const ids = Array.isArray(r['selectedIds']) ? (r['selectedIds'] as unknown[]) : [];
      if (sel.status !== 'ok' || ids.length !== 1 || typeof ids[0] !== 'string') {
        setLine(null);
        return;
      }
      const id = ids[0];
      // The current drawing (the selection's): its id qualifies the stroke the command names.
      const d = await engine.run('draw.drawing.get', { flatten: true }).done;
      const drawingId = obj(d.result)['drawingId'];
      if (typeof drawingId !== 'string') {
        setLine(null);
        return;
      }
      const nodes = Array.isArray(obj(d.result)['nodes']) ? (obj(d.result)['nodes'] as unknown[]) : [];
      const n = nodes.map(obj).find((x) => x['id'] === id);
      const kind = typeof n?.['kind'] === 'string' ? n['kind'] : '';
      if (!n || (kind !== 'pencil' && kind !== 'invisible')) {
        setLine(null);
        return;
      }
      const t = obj(n['lineStyle'])['thickness'];
      setLine({ strokeId: `${drawingId}/${id}`, kind, thickness: typeof t === 'number' ? t : 0 });
    })().catch(() => {
      setLine(null);
    });
  }, [engine]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    void engine
      .onEvent((e) => {
        if (
          e.name === 'draw.selection.changed' ||
          e.name === 'draw.drawing.changed' ||
          e.name === 'app.context.changed' ||
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
  if (!line)
    return (
      <p className="hint" data-tt-role="text" data-tt-id="toolOptions.selectedLine">
        Select one pencil line or invisible stroke to edit its line.
      </p>
    );
  const invisible = line.kind === 'invisible';
  return (
    <div role="group" aria-label="Selected line" data-tt-role="group" data-tt-id="toolOptions.selectedLine">
      <label>
        Line thickness
        <NumericField
          id="toolOptions.selectedLine.thickness"
          label="Line thickness"
          value={line.thickness}
          range={declaredRange('draw.stroke.set', 'thickness')}
          step={0.5}
          disabled={invisible}
          command="draw.stroke.set"
          params={{ strokeId: line.strokeId, thickness: line.thickness }}
          onCommit={(v) =>
            actions.run('draw.stroke.set', { strokeId: line.strokeId, thickness: v }, ['arg.invalid'])
          }
        />
      </label>
      <label>
        <input
          type="checkbox"
          checked={invisible}
          data-tt-role="checkbox"
          data-tt-id="toolOptions.selectedLine.invisible"
          data-tt-command="draw.stroke.set"
          data-tt-params={JSON.stringify({
            strokeId: line.strokeId,
            kind: invisible ? 'pencil' : 'invisible',
          })}
          onChange={() => {
            void actions.run('draw.stroke.set', {
              strokeId: line.strokeId,
              kind: invisible ? 'pencil' : 'invisible',
            });
          }}
        />
        Invisible
      </label>
    </div>
  );
}
