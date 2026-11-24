// The Timeline's dialogs (reviews/M5-design-timeline-view.md section 7), on the shared modal manager (D-44, D-49):
// each collects the value its menu item needs and answers the command reference it completes; Cancel answers nothing
// and nothing is sent. Every choice of every dialog is a D-44 row.
import { useEffect, useRef, useState } from 'react';

import { Dialog } from '../shell/dialogs';
import { NumericField } from '../shell/NumericField';
import { ADD_TYPES } from './menus';
import type { CompletedRef } from './dialogOpen';
import type { TransferOptions } from './model';

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

/** One text field (multi-line for a note). */
export function TextDialog(props: {
  id: string;
  title: string;
  label: string;
  initial: string;
  command: string;
  multiline?: boolean;
  required?: boolean;
  validate?: (t: string) => string | null;
  onDone: (t: string | null) => void;
}) {
  const [text, setText] = useState(props.initial);
  const value = props.multiline ? text : text.trim();
  const problem =
    props.required && value === '' ? 'This cannot be empty.' : (props.validate?.(value) ?? null);
  const field = {
    autoFocus: true,
    value: text,
    'data-tt-role': 'field',
    'data-tt-id': `${props.id}.value`,
  };
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
      submitDisabled={problem !== null}
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
            }}
          />
        ) : (
          <input
            {...field}
            onChange={(e) => {
              setText(e.target.value);
            }}
          />
        )}
      </label>
      {problem && (
        <p className="field-problem" role="alert">
          {problem}
        </p>
      )}
    </Dialog>
  );
}

/** Batch creation (04 §3.2): type, name and count. */
export function NewElementsDialog(props: {
  above?: string | undefined;
  onDone: (ref: CompletedRef | null) => void;
}) {
  const [type, setType] = useState('drawing');
  const [name, setName] = useState('');
  const [count, setCount] = useState(1);
  return (
    <Dialog
      id="timelineNewElements"
      title="New Elements"
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        const params: Record<string, unknown> = { type, count };
        if (name.trim() !== '') params['name'] = name.trim();
        if (props.above) params['above'] = props.above;
        props.onDone({ command: 'element.create', params });
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
          value={name}
          placeholder="(the type's name)"
          data-tt-role="field"
          data-tt-id="timelineNewElements.name"
          onChange={(e) => {
            setName(e.target.value);
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
    </Dialog>
  );
}

const HEX_COLOUR = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/;

/** Element Colours (04 §9.6): the marker and the background colour; an empty field leaves that colour as it is. */
export function ColoursDialog(props: { onDone: (params: Record<string, unknown> | null) => void }) {
  const [marker, setMarker] = useState('');
  const [background, setBackground] = useState('');
  const bad = (t: string) => t.trim() !== '' && !HEX_COLOUR.test(t.trim());
  const problem =
    bad(marker) || bad(background) ? 'Type a colour as #RRGGBB or #RRGGBBAA, or leave it empty.' : null;
  return (
    <Dialog
      id="timelineColours"
      title="Element Colours"
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        const params: Record<string, unknown> = {};
        if (marker.trim() !== '') params['marker'] = marker.trim();
        if (background.trim() !== '') params['background'] = background.trim();
        props.onDone(params);
      }}
      submitLabel="OK"
      submitDisabled={problem !== null || (marker.trim() === '' && background.trim() === '')}
      command="element.colour.set"
    >
      <label>
        Marker colour{' '}
        <input
          autoFocus
          value={marker}
          placeholder="#RRGGBB"
          data-tt-role="field"
          data-tt-id="timelineColours.marker"
          onChange={(e) => {
            setMarker(e.target.value);
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
          }}
        />
      </label>
      {problem && (
        <p className="field-problem" role="alert">
          {problem}
        </p>
      )}
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
