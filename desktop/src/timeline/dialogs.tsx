// The Timeline's dialogs (reviews/M5-design-timeline-view.md section 7), on the shared modal manager (D-44, D-49):
// each collects the value its menu item needs and answers the command reference it completes; Cancel answers nothing
// and nothing is sent. Every choice of every dialog is a D-44 row.
import { useState } from 'react';

import { Dialog } from '../shell/dialogs';
import { ADD_TYPES } from './menus';
import { wholeIn, type CompletedRef } from './dialogOpen';

/** One whole-number field (D-61): OK is disabled while the value is outside the range. */
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
  const [text, setText] = useState(String(props.initial));
  const n = wholeIn(text, props.min, props.max);
  return (
    <Dialog
      id={props.id}
      title={props.title}
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        props.onDone(n);
      }}
      submitLabel="OK"
      submitDisabled={n === null}
      command={props.command}
    >
      <label>
        {props.label}{' '}
        <input
          autoFocus
          inputMode="numeric"
          value={text}
          data-tt-role="field"
          data-tt-id={`${props.id}.value`}
          onChange={(e) => {
            setText(e.target.value);
          }}
        />
      </label>
      {n === null && (
        <p className="field-problem" role="alert">
          {`Type a whole number from ${props.min} to ${props.max}.`}
        </p>
      )}
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
  const [count, setCount] = useState('1');
  const n = wholeIn(count, 1, 100);
  return (
    <Dialog
      id="timelineNewElements"
      title="New Elements"
      onCancel={() => {
        props.onDone(null);
      }}
      onSubmit={() => {
        if (n === null) return;
        const params: Record<string, unknown> = { type, count: n };
        if (name.trim() !== '') params['name'] = name.trim();
        if (props.above) params['above'] = props.above;
        props.onDone({ command: 'element.create', params });
      }}
      submitLabel="Create"
      submitDisabled={n === null}
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
        Number{' '}
        <input
          inputMode="numeric"
          value={count}
          data-tt-role="field"
          data-tt-id="timelineNewElements.count"
          onChange={(e) => {
            setCount(e.target.value);
          }}
        />
      </label>
      {n === null && (
        <p className="field-problem" role="alert">
          Type a number of elements from 1 to 100.
        </p>
      )}
    </Dialog>
  );
}
