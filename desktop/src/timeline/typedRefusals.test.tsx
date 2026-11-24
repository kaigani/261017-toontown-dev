// Ruling E-53 (C51B-R2-01; 04 §3.2, §4.1): every typed value a Timeline dialog submits goes to the kernel, which
// decides. A refusal (an empty, invalid, duplicate or too long name; a colour the kernel cannot read) keeps the dialog
// open with what was typed and shows why; a value taken closes it. The status line's text for each refusal is
// `refusalLine`'s (D-87). The rename editor in the row is TimelinePanel.test.tsx's "rename in place".
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { refusalLine } from '../shell/refusals';
import { ColoursDialog, NewElementsDialog, TextDialog } from './dialogs';

afterEach(cleanup);

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};

const NAME_REFUSALS = [
  ['element.nameEmpty', 'names must not be empty'],
  ['element.nameInvalid', 'names must not contain any of \\ / : * ? < > " | .'],
  ['element.nameDuplicate', 'an element of this scene is already named `Bob`'],
  ['element.nameTooLong', 'names must not exceed 64 characters'],
] as const;

describe('the status line text of a refused typed value (D-87, E-53)', () => {
  it.each(NAME_REFUSALS)('%s is a refusal: the command label and the reason', (code, message) => {
    expect(refusalLine({ code, message }, 'element.rename')).toBe(`Rename element: ${message}`);
  });

  it('the kernel’s arg.invalid is a refusal only for a typed value', () => {
    const error = { code: 'arg.invalid', message: 'not a colour: `green-ish`' };
    expect(refusalLine(error, 'element.colour.set')).toBeNull();
    expect(refusalLine(error, 'element.colour.set', true)).toBe(
      'Set element colour: not a colour: `green-ish`',
    );
  });
});

/** Renders a dialog alone. */
function show(node: React.ReactNode) {
  return render(<>{node}</>);
}

describe('a dialog with a typed field stays open on a refusal (E-53)', () => {
  it.each(NAME_REFUSALS)(
    'Rename: %s keeps the dialog open with the text, then a taken name closes it',
    async (code) => {
      const submitted: string[] = [];
      const refusal = `Rename element: ${code}`;
      const submit = vi.fn((t: string) => {
        submitted.push(t);
        return Promise.resolve(t === 'Robert' ? null : refusal);
      });
      const onDone = vi.fn();
      show(
        <TextDialog
          id="timelineRename"
          title="Rename Element"
          label="Name"
          initial="Bob"
          command="element.rename"
          submit={submit}
          onDone={onDone}
        />,
      );
      const field = el('timelineRename.value') as HTMLInputElement;
      // The empty name is sent too: the kernel decides (no client-side rule disables OK).
      act(() => {
        fireEvent.change(field, { target: { value: '' } });
      });
      expect((el('timelineRename.submit') as HTMLButtonElement).disabled).toBe(false);
      act(() => {
        el('timelineRename.submit').click();
      });
      await waitFor(() => {
        expect(el('timelineRename.refusal').textContent).toBe(refusal);
      });
      expect(submitted).toEqual(['']);
      expect(onDone).not.toHaveBeenCalled();
      expect(field.value).toBe('');
      expect(document.activeElement).toBe(field);
      // Typing clears the refusal; a name the kernel takes closes the dialog with it.
      act(() => {
        fireEvent.change(field, { target: { value: 'Robert' } });
      });
      expect(document.querySelector('[data-tt-id="timelineRename.refusal"]')).toBeNull();
      act(() => {
        el('timelineRename.submit').click();
      });
      await waitFor(() => {
        expect(onDone).toHaveBeenCalledWith('Robert');
      });
      expect(submitted).toEqual(['', 'Robert']);
    },
  );

  it('Cancel sends nothing', () => {
    const submit = vi.fn(() => Promise.resolve(null));
    const onDone = vi.fn();
    show(
      <TextDialog
        id="timelineNote"
        title="Element Note"
        label="Note"
        initial=""
        command="element.note.set"
        multiline
        submit={submit}
        onDone={onDone}
      />,
    );
    act(() => {
      el('timelineNote.cancel').click();
    });
    expect(submit).not.toHaveBeenCalled();
    expect(onDone).toHaveBeenCalledWith(null);
  });

  it('New Elements: a refused name keeps the dialog open; an empty field is the type’s default name', async () => {
    const sent: unknown[] = [];
    const submit = vi.fn((ref: { params: Record<string, unknown> }) => {
      sent.push(ref.params);
      return Promise.resolve(ref.params['name'] === 'Bob' ? 'Create element: duplicate' : null);
    });
    const onDone = vi.fn();
    show(<NewElementsDialog submit={submit} onDone={onDone} />);
    const name = el('timelineNewElements.name') as HTMLInputElement;
    act(() => {
      fireEvent.change(name, { target: { value: 'Bob' } });
    });
    act(() => {
      el('timelineNewElements.submit').click();
    });
    await waitFor(() => {
      expect(el('timelineNewElements.refusal').textContent).toBe('Create element: duplicate');
    });
    expect(onDone).not.toHaveBeenCalled();
    expect(name.value).toBe('Bob');
    act(() => {
      fireEvent.change(name, { target: { value: '' } });
    });
    act(() => {
      el('timelineNewElements.submit').click();
    });
    await waitFor(() => {
      expect(onDone).toHaveBeenCalled();
    });
    expect(sent).toEqual([
      { type: 'drawing', count: 1, name: 'Bob' },
      { type: 'drawing', count: 1 },
    ]);
  });

  it('Element Colours: a colour typed goes to the kernel, and its refusal keeps the dialog open', async () => {
    const submit = vi.fn(() => Promise.resolve('Set element colour: not a colour: `green-ish`'));
    const onDone = vi.fn();
    show(<ColoursDialog submit={submit} onDone={onDone} />);
    act(() => {
      fireEvent.change(el('timelineColours.marker'), { target: { value: 'green-ish' } });
    });
    act(() => {
      el('timelineColours.submit').click();
    });
    await waitFor(() => {
      expect(el('timelineColours.refusal').textContent).toBe('Set element colour: not a colour: `green-ish`');
    });
    expect(submit).toHaveBeenCalledWith({ marker: 'green-ish' });
    expect(onDone).not.toHaveBeenCalled();
    expect((el('timelineColours.marker') as HTMLInputElement).value).toBe('green-ish');
  });

  it('a second OK while one submission is out sends nothing more', async () => {
    let answer: (v: string | null) => void = () => undefined;
    const submit = vi.fn(
      () =>
        new Promise<string | null>((r) => {
          answer = r;
        }),
    );
    const onDone = vi.fn();
    show(
      <TextDialog
        id="timelineRename"
        title="Rename Element"
        label="Name"
        initial="Bob"
        command="element.rename"
        submit={submit}
        onDone={onDone}
      />,
    );
    act(() => {
      el('timelineRename.submit').click();
      el('timelineRename.submit').click();
    });
    expect(submit).toHaveBeenCalledTimes(1);
    act(() => {
      answer(null);
    });
    await waitFor(() => {
      expect(onDone).toHaveBeenCalledTimes(1);
    });
  });
});
