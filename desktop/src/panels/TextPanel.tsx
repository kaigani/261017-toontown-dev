// The Text panel as the defaults for new text (02 §7.6.1 lines 683-699; M4 C4.4a, reviews/M4-design-panels.md §5):
// with no text being edited it edits `tool.text.options.set` (font, size, bold, italic, baseline, kerning, auto kern,
// alignment, margins, indent, line spacing, removing the outline). C4.4b's Text tool gives the panel its second role
// (formatting highlighted text). The font is typed until a font list exists (SI-169); an outline colour is chosen with
// the palette (M7), so the panel offers only to remove one. Every control sends one command and shows the kernel's
// state, re-read on `tool.options.changed` for the Text tool and the project events.
import { useCallback, useEffect, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import { optionRef, runChosen } from '../shell/listControl';
import { declaredRange, NumericField } from '../shell/NumericField';
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

export function TextPanel() {
  const { engine } = useShell();
  const actions = useActions();
  const [options, setOptions] = useState<Json | null>(null);
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
    void engine
      .onEvent((e) => {
        const toolId = obj(e.payload)['toolId'];
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
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, refresh]);
  const send = (params: Json) => {
    void actions.run('tool.text.options.set', params);
  };
  if (!options)
    return (
      <div className="text-panel" data-tt-role="panel" data-tt-id="text" data-tt-label="Text">
        <p className="empty">No project is open.</p>
      </div>
    );
  const font = typeof options['font'] === 'string' ? options['font'] : '';
  const outline = typeof options['outlineColourId'] === 'string' ? options['outlineColourId'] : null;
  return (
    <div className="text-panel" data-tt-role="panel" data-tt-id="text" data-tt-label="Text">
      <p className="hint">Defaults for new text</p>
      <TextValue
        id="text.font"
        label="Font"
        value={font}
        onCommit={(v) => {
          const name = v.trim();
          if (name !== font) send({ font: name === '' ? null : name });
        }}
        params={{ font: font === '' ? null : font }}
      />
      {/* D-57: system fonts are allowed; only a bundled font lays out the same on Windows and macOS, and none is
          bundled yet, so every font, the platform default included, is marked. */}
      <p className="field-hint" data-tt-role="text" data-tt-id="text.fontNotBundled">
        {font === '' ? 'The platform default font' : `"${font}"`} is not bundled: text lays out with this
        computer's font files and may differ on another system.
      </p>
      {NUMBERS.map((n) => {
        const value = typeof options[n.key] === 'number' ? (options[n.key] as number) : 0;
        return (
          <label key={n.key}>
            {n.label}
            {/* The window's one numeric field (D-61), with the command's declared range. */}
            <NumericField
              id={`text.${n.key}`}
              label={n.label}
              value={value}
              range={declaredRange('tool.text.options.set', n.key)}
              step={n.step}
              command="tool.text.options.set"
              params={{ [n.key]: value }}
              onCommit={(v) =>
                actions.run(
                  'tool.text.options.set',
                  { [n.key]: n.key === 'kerning' ? Math.round(v) : v },
                  ['arg.invalid'],
                )
              }
            />
          </label>
        );
      })}
      {FLAGS.map((f) => {
        const on = options[f.key] === true;
        return (
          <label key={f.key}>
            <input
              type="checkbox"
              checked={on}
              data-tt-role="checkbox"
              data-tt-id={`text.${f.key}`}
              data-tt-command="tool.text.options.set"
              data-tt-params={JSON.stringify({ [f.key]: !on })}
              onChange={() => {
                send({ [f.key]: !on });
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
            value={typeof options[c.key] === 'string' ? (options[c.key] as string) : c.values[0].value}
            data-tt-role="list"
            data-tt-id={`text.${c.key}`}
            data-tt-command="tool.text.options.set"
            onChange={runChosen(actions)}
          >
            {c.values.map((v) => (
              <option
                key={v.value}
                value={v.value}
                {...optionRef(`text.${c.key}`, v.value, v.label, 'tool.text.options.set', {
                  [c.key]: v.value,
                })}
              >
                {v.label}
              </option>
            ))}
          </select>
        </label>
      ))}
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
    </div>
  );
}

function TextValue(props: {
  id: string;
  label: string;
  value: string;
  params: Json;
  onCommit: (v: string) => void;
}) {
  const [typed, setTyped] = useState<{ from: string; text: string } | null>(null);
  const draft = typed?.from === props.value ? typed.text : props.value;
  return (
    <label>
      {props.label}
      <input
        value={draft}
        data-tt-role="textField"
        data-tt-id={props.id}
        data-tt-command="tool.text.options.set"
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
