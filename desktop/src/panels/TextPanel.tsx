// The Text panel (02 §7.6.1 lines 683-699). Two roles (design note 5 §6):
// - no text being edited: the defaults for new text (M4 C4.4a, reviews/M4-design-panels.md §5), `tool.text.options.set`
//   (font, size, bold, italic, baseline, kerning, auto kern, alignment, margins, indent, line spacing, removing the
//   outline);
// - the Text tool's session open (M4 C4.4b): `draw.textFormat.set` over the highlight (`start`, `length`), or at the
//   caret with nothing highlighted, where character fields apply to the next typed characters (SI-195). The values are
//   the kernel's over the range, a field that differs being "Mixed"; they arrive with the host-only `shell.textEdit`
//   notice. The project format keeps no outline colour per character, so this role has no outline control.
// The font is typed until a font list exists (SI-169). Every control sends one command and shows the kernel's state.
import { useCallback, useEffect, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import { optionRef, runChosen } from '../shell/listControl';
import { declaredRange } from '../shell/numeric';
import { NumericField } from '../shell/NumericField';
import { useShell } from '../state/context';

type Json = Record<string, unknown>;
const obj = (v: unknown): Json => (typeof v === 'object' && v !== null ? (v as Json) : {});

const NUMBERS = [
  { key: 'size', label: 'Size', min: 1, max: 512, step: 2 },
  { key: 'kerning', label: 'Kerning', min: -60, max: 60, step: 1 },
  { key: 'leftMargin', label: 'Left margin', min: 0, max: 1000, step: 1 },
  { key: 'rightMargin', label: 'Right margin', min: 0, max: 1000, step: 1 },
  { key: 'indent', label: 'Indent', min: -99, max: 1000, step: 1 },
  { key: 'lineSpacing', label: 'Line spacing', min: -99, max: 1000, step: 1 },
] as const;

const FLAGS = [
  { key: 'bold', label: 'Bold' },
  { key: 'italic', label: 'Italic' },
  { key: 'autoKern', label: 'Auto kern' },
] as const;

const CHOICES = [
  {
    key: 'baseline',
    label: 'Baseline',
    values: [
      { value: 'normal', label: 'Normal' },
      { value: 'subscript', label: 'Subscript' },
      { value: 'superscript', label: 'Superscript' },
    ],
  },
  {
    key: 'align',
    label: 'Alignment',
    values: [
      { value: 'left', label: 'Left' },
      { value: 'centre', label: 'Centre' },
      { value: 'right', label: 'Right' },
      { value: 'justify', label: 'Justify' },
    ],
  },
] as const;

/** The session the Text tool has open (`shell.textEdit`), or null. */
interface Editing {
  drawingId: string;
  textId: string;
  anchor: number;
  caret: number;
  format: Json;
}

function editingOf(v: unknown): Editing | null {
  const o = obj(v);
  if (typeof o['textId'] !== 'string' || typeof o['drawingId'] !== 'string') return null;
  return {
    drawingId: o['drawingId'],
    textId: o['textId'],
    anchor: typeof o['anchor'] === 'number' ? o['anchor'] : 0,
    caret: typeof o['caret'] === 'number' ? o['caret'] : 0,
    format: obj(o['format']),
  };
}

export function TextPanel() {
  const { engine } = useShell();
  const actions = useActions();
  const [options, setOptions] = useState<Json | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const refresh = useCallback(() => {
    void engine
      .run('tool.options.get', { toolId: 'text' })
      .done.then((env) => {
        setOptions(env.status === 'ok' ? obj(obj(env.result)['options']) : null);
      })
      .catch(() => undefined);
  }, [engine]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    // A notice heard since mounting is newer than the scene answer read below.
    let heard = false;
    void engine
      .onEvent((e) => {
        const toolId = obj(e.payload)['toolId'];
        // The Text tool's session (design note 5 §5): host-only, never sent to an agent.
        if (e.name === 'shell.textEdit') {
          heard = true;
          setEditing(editingOf(e.payload));
        }
        if (e.name === 'project.closed') {
          heard = true;
          setEditing(null);
        }
        if (
          (e.name === 'tool.options.changed' && (toolId === 'text' || toolId === null)) ||
          e.name === 'project.opened' ||
          e.name === 'project.closed'
        )
          refresh();
      })
      .then((u) => {
        if (stopped) u();
        else off = u;
        // Ruling D-85 item 2 (finding C44B-R2-02): a panel opened while a session is open reads the session when it
        // mounts (the scene answer's `textEdit`, which every view's answer carries), then follows the notices.
        return engine.canvasScene('drawingView').then((scene) => {
          if (stopped || heard) return;
          const t = obj(obj(scene)['textEdit']);
          if (typeof t['textId'] !== 'string') return;
          setEditing(
            editingOf({
              drawingId: t['drawingId'],
              textId: t['textId'],
              anchor: t['anchor'],
              caret: t['caretIndex'],
              format: t['format'],
            }),
          );
        });
      })
      .catch(() => undefined);
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, refresh]);
  // The role shown: the session's formatting, else the defaults.
  const start = editing ? Math.min(editing.anchor, editing.caret) : 0;
  const length = editing ? Math.abs(editing.caret - editing.anchor) : 0;
  const command = editing ? 'draw.textFormat.set' : 'tool.text.options.set';
  const base: Json = editing ? { drawingId: editing.drawingId, textId: editing.textId, start, length } : {};
  const values: Json = editing ? editing.format : (options ?? {});
  const mixed = (k: string) => editing !== null && values[k] === null;
  const send = (params: Json) => {
    void actions.run(command, { ...base, ...params });
  };
  if (!options)
    return (
      <div className="text-panel" data-tt-role="panel" data-tt-id="text" data-tt-label="Text">
        <p className="empty">No project is open.</p>
      </div>
    );
  const font = typeof values['font'] === 'string' ? values['font'] : '';
  const outline = typeof options['outlineColourId'] === 'string' ? options['outlineColourId'] : null;
  return (
    <div className="text-panel" data-tt-role="panel" data-tt-id="text" data-tt-label="Text">
      <p className="hint" data-tt-role="text" data-tt-id="text.role">
        {editing === null ? 'Defaults for new text' : length > 0 ? 'Selected text' : 'Next typed text'}
      </p>
      <TextValue
        id="text.font"
        label="Font"
        value={font}
        command={command}
        placeholder={mixed('font') ? 'Mixed' : undefined}
        onCommit={(v) => {
          const name = v.trim();
          if (name === font) return;
          // A session's font is a name (02 §7.6.1); the defaults' empty name is the platform default.
          if (editing && name !== '') send({ font: name });
          else if (!editing) send({ font: name === '' ? null : name });
        }}
        params={{ ...base, font: font === '' ? null : font }}
      />
      {/* D-57: system fonts are allowed; only a bundled font lays out the same on Windows and macOS, and none is
          bundled yet, so every font, the platform default included, is marked. */}
      <p className="field-hint" data-tt-role="text" data-tt-id="text.fontNotBundled">
        {font === '' ? 'The platform default font' : `"${font}"`} is not bundled: text lays out with this
        computer's font files and may differ on another system.
      </p>
      {NUMBERS.map((n) => {
        const value = typeof values[n.key] === 'number' ? (values[n.key] as number) : 0;
        return (
          <label key={n.key}>
            {n.label}
            {/* The window's one numeric field (D-61), with the command's declared range. */}
            <NumericField
              id={`text.${n.key}`}
              label={n.label}
              value={value}
              mixed={mixed(n.key)}
              range={declaredRange(command, n.key)}
              step={n.step}
              command={command}
              params={{ ...base, [n.key]: value }}
              onCommit={(v) =>
                actions.run(command, { ...base, [n.key]: n.key === 'kerning' ? Math.round(v) : v }, [
                  'arg.invalid',
                ])
              }
            />
          </label>
        );
      })}
      {FLAGS.map((f) => {
        const on = values[f.key] === true;
        const isMixed = mixed(f.key);
        return (
          <label key={f.key}>
            <input
              type="checkbox"
              checked={on}
              ref={(el) => {
                if (el) el.indeterminate = isMixed;
              }}
              aria-checked={isMixed ? 'mixed' : on}
              data-tt-role="checkbox"
              data-tt-id={`text.${f.key}`}
              data-tt-command={command}
              data-tt-params={JSON.stringify({ ...base, [f.key]: isMixed ? true : !on })}
              onChange={() => {
                send({ [f.key]: isMixed ? true : !on });
              }}
            />
            {f.label}
          </label>
        );
      })}
      {CHOICES.map((c) => (
        <label key={c.key}>
          {c.label}
          <select
            value={
              mixed(c.key)
                ? ''
                : typeof values[c.key] === 'string'
                  ? (values[c.key] as string)
                  : c.values[0].value
            }
            data-tt-role="list"
            data-tt-id={`text.${c.key}`}
            data-tt-command={command}
            onChange={runChosen(actions)}
          >
            {/* A mixed choice has no option chosen (design note 5 §6). */}
            {mixed(c.key) && (
              <option value="" disabled hidden>
                Mixed
              </option>
            )}
            {c.values.map((v) => (
              <option
                key={v.value}
                value={v.value}
                {...optionRef(`text.${c.key}`, v.value, v.label, command, {
                  ...base,
                  [c.key]: v.value,
                })}
              >
                {v.label}
              </option>
            ))}
          </select>
        </label>
      ))}
      {editing === null && (
        <button
          type="button"
          disabled={outline === null}
          title={outline === null ? 'New text has no outline' : 'Remove the outline of new text'}
          data-tt-role="button"
          data-tt-id="text.removeOutline"
          data-tt-command="tool.text.options.set"
          data-tt-params={JSON.stringify({ outlineColourId: null })}
          onClick={() => {
            send({ outlineColourId: null });
          }}
        >
          Remove outline
        </button>
      )}
    </div>
  );
}

function TextValue(props: {
  id: string;
  label: string;
  value: string;
  params: Json;
  command: string;
  placeholder?: string | undefined;
  onCommit: (v: string) => void;
}) {
  const [typed, setTyped] = useState<{ from: string; text: string } | null>(null);
  const draft = typed?.from === props.value ? typed.text : props.value;
  return (
    <label>
      {props.label}
      <input
        value={draft}
        placeholder={props.placeholder}
        data-tt-role="textField"
        data-tt-id={props.id}
        data-tt-command={props.command}
        data-tt-params={JSON.stringify(props.params)}
        onChange={(e) => {
          setTyped({ from: props.value, text: e.target.value });
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') props.onCommit(draft);
          if (e.key === 'Escape') setTyped(null);
        }}
        onBlur={() => {
          props.onCommit(draft);
        }}
      />
    </label>
  );
}
