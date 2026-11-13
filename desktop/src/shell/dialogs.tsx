// Dialog host (M4 C4.1): dialogs collect the parameters of a command and run it once on OK (01 §0, 11 §2). Each open
// dialog is a modal state the executor knows about (user priority, ADR-0021), and is part of the UI tree.
import { useCallback, useMemo, useRef, useState, type ReactNode } from 'react';

import { DialogsContext, type Close, type DialogsApi } from './dialogsApi';
import { useMessages } from './messagesApi';

interface Entry {
  readonly key: number;
  readonly title: string;
  readonly node: ReactNode;
}

export function DialogHost({ children }: { children: ReactNode }) {
  const messages = useMessages();
  const [stack, setStack] = useState<readonly Entry[]>([]);
  const next = useRef(1);

  const open = useCallback(
    <T,>(title: string, render: (close: Close<T>) => ReactNode): Promise<T | null> =>
      new Promise<T | null>((resolve) => {
        const key = next.current++;
        const leave = messages.enterModal('dialog', title);
        const close: Close<T> = (result) => {
          leave();
          setStack((s) => s.filter((e) => e.key !== key));
          resolve(result);
        };
        setStack((s) => [...s, { key, title, node: render(close) }]);
      }),
    [messages],
  );

  const api = useMemo<DialogsApi>(() => ({ open }), [open]);
  return (
    <DialogsContext.Provider value={api}>
      {children}
      {stack.map((e) => (
        <div key={e.key} className="modal-backdrop">
          {e.node}
        </div>
      ))}
    </DialogsContext.Provider>
  );
}

/** Standard dialog chrome: a titled panel, Escape cancels, Enter submits when valid. */
export function Dialog({
  id,
  title,
  children,
  onCancel,
  onSubmit,
  submitLabel,
  submitDisabled,
  wide,
}: {
  id: string;
  title: string;
  children: ReactNode;
  onCancel: () => void;
  onSubmit?: () => void;
  submitLabel?: string;
  submitDisabled?: boolean;
  wide?: boolean;
}) {
  return (
    <div
      className={wide ? 'dialog dialog-wide' : 'dialog'}
      role="dialog"
      aria-modal="true"
      aria-label={title}
      data-tt-role="dialog"
      data-tt-id={id}
      data-tt-label={title}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onCancel();
        }
      }}
    >
      <h2>{title}</h2>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (onSubmit && !submitDisabled) onSubmit();
        }}
      >
        {children}
        <div className="buttons">
          <button type="button" onClick={onCancel} data-tt-role="button" data-tt-id={`${id}.cancel`}>
            Cancel
          </button>
          {onSubmit && (
            <button
              type="submit"
              className="button-default"
              disabled={submitDisabled}
              data-tt-role="button"
              data-tt-id={`${id}.submit`}
            >
              {submitLabel ?? 'OK'}
            </button>
          )}
        </div>
      </form>
    </div>
  );
}

/** A one-field name prompt (New Workspace, Rename Workspace). */
export function NamePrompt({
  id,
  title,
  label,
  initial,
  submitLabel,
  validate,
  close,
}: {
  id: string;
  title: string;
  label: string;
  initial: string;
  submitLabel: string;
  validate: (name: string) => string | null;
  close: Close<string>;
}) {
  const [name, setName] = useState(initial);
  const problem = validate(name);
  return (
    <Dialog
      id={id}
      title={title}
      onCancel={() => {
        close(null);
      }}
      onSubmit={() => {
        close(name.trim());
      }}
      submitLabel={submitLabel}
      submitDisabled={problem !== null}
    >
      <label>
        {label}{' '}
        <input
          autoFocus
          value={name}
          data-tt-role="field"
          data-tt-id={`${id}.name`}
          onChange={(e) => {
            setName(e.target.value);
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
