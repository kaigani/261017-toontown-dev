// The Timeline's dialogs (reviews/M5-design-timeline-view.md section 7), on the shared modal manager (D-44, D-49):
// each collects the value its menu item needs and answers the command reference it completes; Cancel answers nothing
// and nothing is sent. Every choice of every dialog is a D-44 row.
import { useEffect, useRef, useState } from 'react';

import { Dialog } from '../shell/dialogs';
import { NumericField } from '../shell/NumericField';
import { ADD_TYPES } from './menus';
import type { CompletedRef } from './dialogOpen';
import type { TransferOptions } from './model';

/** What a dialog that sends a typed value does at OK (ruling E-53): the kernel decides. `send` answers null when the
 *  value was taken (the dialog closes) or the refusal, which the dialog shows under its fields while it stays open with
 *  what was typed, the focus back in `focus`; a second OK while one is out is ignored. Typing clears the refusal. */
function useTypedSubmit(focus: { current: HTMLElement | null }) {
  const [refusal, setRefusal] = useState<string | null>(null);
  const sending = useRef(false);
  const submit = (send: () => Promise<string | null>, done: () => void) => {
    if (sending.current) return;
    sending.current = true;
    void send().then((refused) => {
      sending.current = false;
      if (refused === null) {
        done();
        return;
      }
      setRefusal(refused);
      focus.current?.focus();
    });
  };
  return {
    refusal,
    submit,
    clear: () => {
      setRefusal(null);
    },
  };
}

/** The refusal of a typed value, under the dialog's fields (ruling E-53; the status line shows it too). */
function Refusal(props: { id: string; text: string | null }) {
  return props.text === null ? null : (
    <p className="field-problem" role="alert" data-tt-role="text" data-tt-id={`${props.id}.refusal`}>
      {props.text}
    </p>
  );
}

/** One whole-number field: the window's one numeric field (ruling D-61 item 3; C51B-R1S-06), with its commit rule. An
 *  empty, non-numeric or out-of-range entry is never a value: the field returns to the value it holds and says why, and
 *  OK sends the value held. Enter on an unchanged value is OK (01 §2). */
export function NumberDialog(props: {
  id: string;
  title: string;
  label: string;
  initial: number;
  min: number;
  max: number;
  command: string;
  onDone: (n: number | null) => void;
}) {
  const [value, setValue] = useState(props.initial);
  const input = useRef<HTMLInputElement | null>(null);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  return (
    <Dialog
      id={props.id}
      title={props.title}
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        props.onDone(value);
      }}
      submitLabel="OK"
      command={props.command}
    >
      <label>
        {props.label}{' '}
        <NumericField
          id={`${props.id}.value`}
          label={props.label}
          role="field"
          guiOnly="typing"
          value={value}
          range={{ min: props.min, max: props.max, integer: true }}
          inputRef={input}
          onSubmit={() => {
            props.onDone(value);
          }}
          onCommit={(v) => {
            setValue(v);
          }}
        />
      </label>
    </Dialog>
  );
}

/** One text field (multi-line for a note). OK sends the text as typed through `submit` (ruling E-53: the kernel decides,
 *  an empty name included): taken, the dialog closes with it; refused, it stays open with the text and says why. */
export function TextDialog(props: {
  id: string;
  title: string;
  label: string;
  initial: string;
  command: string;
  multiline?: boolean;
  submit: (t: string) => Promise<string | null>;
  onDone: (t: string | null) => void;
}) {
  const [text, setText] = useState(props.initial);
  const focus = useRef<HTMLElement | null>(null);
  const typed = useTypedSubmit(focus);
  const field = {
    autoFocus: true,
    value: text,
    'data-tt-role': 'field',
    'data-tt-id': `${props.id}.value`,
    ref: (el: HTMLElement | null) => {
      focus.current = el;
    },
  };
  return (
    <Dialog
      id={props.id}
      title={props.title}
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        typed.submit(
          () => props.submit(text),
          () => {
            props.onDone(text);
          },
        );
      }}
      submitLabel="OK"
      command={props.command}
    >
      <label>
        {props.label}{' '}
        {props.multiline ? (
          <textarea
            {...field}
            rows={4}
            onChange={(e) => {
              setText(e.target.value);
              typed.clear();
            }}
          />
        ) : (
          <input
            {...field}
            onChange={(e) => {
              setText(e.target.value);
              typed.clear();
            }}
          />
        )}
      </label>
      <Refusal id={props.id} text={typed.refusal} />
    </Dialog>
  );
}

/** Batch creation (04 §3.2): type, name and count. An empty name field is the type's default name; any name typed is
 *  sent as typed (ruling E-53: the kernel refuses an empty, invalid, duplicate or too long one, and the dialog stays open
 *  with it). */
export function NewElementsDialog(props: {
  above?: string | undefined;
  submit: (ref: CompletedRef) => Promise<string | null>;
  onDone: (ref: CompletedRef | null) => void;
}) {
  const [type, setType] = useState('drawing');
  const [name, setName] = useState('');
  const [count, setCount] = useState(1);
  const focus = useRef<HTMLElement | null>(null);
  const typed = useTypedSubmit(focus);
  return (
    <Dialog
      id="timelineNewElements"
      title="New Elements"
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        const params: Record<string, unknown> = { type, count };
        if (name !== '') params['name'] = name;
        if (props.above) params['above'] = props.above;
        const ref = { command: 'element.create', params };
        typed.submit(
          () => props.submit(ref),
          () => {
            props.onDone(ref);
          },
        );
      }}
      submitLabel="Create"
      command="element.create"
    >
      <label>
        Type{' '}
        <select
          value={type}
          data-tt-role="field"
          data-tt-id="timelineNewElements.type"
          onChange={(e) => {
            setType(e.target.value);
          }}
        >
          {ADD_TYPES.map((t) => (
            <option key={t.type} value={t.type}>
              {t.label}
            </option>
          ))}
        </select>
      </label>
      <label>
        Name{' '}
        <input
          ref={(el) => {
            focus.current = el;
          }}
          value={name}
          placeholder="(the type's name)"
          data-tt-role="field"
          data-tt-id="timelineNewElements.name"
          onChange={(e) => {
            setName(e.target.value);
            typed.clear();
          }}
        />
      </label>
      <label>
        Number {/* The batch count: the one numeric field (C51B-R1S-06), 1 to 100. */}
        <NumericField
          id="timelineNewElements.count"
          label="Number"
          role="field"
          guiOnly="typing"
          value={count}
          range={{ min: 1, max: 100, integer: true }}
          onCommit={(v) => {
            setCount(v);
          }}
        />
      </label>
      <Refusal id="timelineNewElements" text={typed.refusal} />
    </Dialog>
  );
}

/** Element Colours (04 §9.6): the marker and the background colour; an empty field leaves that colour as it is. A
 *  colour typed is sent as typed, spaces included (rulings E-53, E-62: the kernel refuses one it cannot read, and the
 *  dialog stays open). */
export function ColoursDialog(props: {
  /** The element's own colours (empty: the default), the values the dialog starts from (C51B-R1S-05). */
  marker?: string;
  background?: string;
  submit: (params: Record<string, unknown>) => Promise<string | null>;
  onDone: (params: Record<string, unknown> | null) => void;
}) {
  const [marker, setMarker] = useState(props.marker ?? '');
  const [background, setBackground] = useState(props.background ?? '');
  const focus = useRef<HTMLElement | null>(null);
  const typed = useTypedSubmit(focus);
  return (
    <Dialog
      id="timelineColours"
      title="Element Colours"
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        const params: Record<string, unknown> = {};
        // Only a truly empty field leaves that colour as it is; anything typed, spaces included, goes to the kernel
        // (rulings E-53, E-62: C51B-R3S-03).
        if (marker !== '') params['marker'] = marker;
        if (background !== '') params['background'] = background;
        typed.submit(
          () => props.submit(params),
          () => {
            props.onDone(params);
          },
        );
      }}
      submitLabel="OK"
      submitDisabled={marker === '' && background === ''}
      command="element.colour.set"
    >
      <label>
        Marker colour{' '}
        <input
          autoFocus
          ref={(el) => {
            focus.current = el;
          }}
          value={marker}
          placeholder="#RRGGBB"
          data-tt-role="field"
          data-tt-id="timelineColours.marker"
          onChange={(e) => {
            setMarker(e.target.value);
            typed.clear();
          }}
        />
      </label>
      <label>
        Background colour{' '}
        <input
          value={background}
          placeholder="#RRGGBB"
          data-tt-role="field"
          data-tt-id="timelineColours.background"
          onChange={(e) => {
            setBackground(e.target.value);
            typed.clear();
          }}
        />
      </label>
      <Refusal id="timelineColours" text={typed.refusal} />
    </Dialog>
  );
}

const PEG_KEYS = [
  ['motion', 'Motion'],
  ['rotation', 'Rotation'],
  ['scale', 'Scale'],
  ['pivot', 'Pivot'],
  ['velocity', 'Velocity'],
  ['skew', 'Skew'],
] as const;

/** The transfer-options dialog shared by Paste Special, Cut Special and drag and drop (04 §11.14's two groups): the
 *  stored set is read when it opens (`load`), OK answers the set chosen. */
export function TransferOptionsDialog(props: {
  title: string;
  load: () => Promise<TransferOptions | null>;
  onDone: (options: TransferOptions | null) => void;
}) {
  const [o, setO] = useState<TransferOptions | null>(null);
  // Read once, when the dialog opens: `load` is a new function at every render of the caller.
  const load = useRef(props.load);
  useEffect(() => {
    let live = true;
    void load.current().then((v) => {
      if (live) setO(v);
    });
    return () => {
      live = false;
    };
  }, []);
  const check = (id: string, label: string, value: boolean, set: (v: boolean) => void) => (
    <label key={id}>
      <input
        type="checkbox"
        checked={value}
        data-tt-role="checkbox"
        data-tt-id={`timelineTransfer.${id}`}
        data-tt-gui-only="typing"
        onChange={(e) => {
          set(e.target.checked);
        }}
      />{' '}
      {label}
    </label>
  );
  return (
    <Dialog
      id="timelineTransfer"
      title={props.title}
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        props.onDone(o);
      }}
      submitLabel="OK"
      submitDisabled={o === null}
      command="timeline.transferOptions.set"
    >
      {o === null ? (
        <p data-tt-role="text" data-tt-id="timelineTransfer.loading">
          Reading the options...
        </p>
      ) : (
        <>
          <fieldset>
            <legend>Drawings</legend>
            {check('overwriteDrawings', 'Overwrite existing drawings', o.overwriteDrawings, (v) => {
              setO({ ...o, overwriteDrawings: v });
            })}
            {check('updateColours', 'Update colours from the source palette', o.updateColours, (v) => {
              setO({ ...o, updateColours: v });
            })}
            {check('exposure', 'Create and delete exposure', o.exposure, (v) => {
              setO({ ...o, exposure: v });
            })}
            {check('autoExtend', 'Extend the exposure automatically', o.autoExtend, (v) => {
              setO({ ...o, autoExtend: v });
            })}
          </fieldset>
          <fieldset>
            <legend>Keyframes</legend>
            {check('keyframeSubstitution', 'Substitute keyframes', o.keyframeSubstitution, (v) => {
              setO({ ...o, keyframeSubstitution: v });
            })}
            {check('boundaryKeys', 'Keyframes at the boundaries', o.boundaryKeys, (v) => {
              setO({ ...o, boundaryKeys: v });
            })}
            {PEG_KEYS.map(([k, label]) =>
              check(`pegKeys.${k}`, `${label} keys`, o.pegKeys[k], (v) => {
                setO({ ...o, pegKeys: { ...o.pegKeys, [k]: v } });
              }),
            )}
          </fieldset>
        </>
      )}
    </Dialog>
  );
}
