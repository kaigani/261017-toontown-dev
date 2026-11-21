// Ruling D-66 item 2 (C44A-R2-01; 01 §8.4: "platform text-editing shortcuts act on the focused text field"): with the
// focus in a text-entry element (the numeric field, a text input, a text area, an inline rename, anything editable),
// the platform's text-editing keys belong to the field and never reach the keymap: select all, cut, copy, paste, undo
// and redo of the field's own text, word and line navigation, Home, End, Delete, Backspace, and plain character keys,
// those bound to tools included. Each such key is left to the browser (its default not prevented) and no command is
// sent; a chord that is not text editing (Save) still reaches the keymap outside a dialog. The decision is made in one
// place, `fieldOwnsKey` in the key dispatcher (`windowHooks.ts`).
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';
import TEXT_FIELD_KEYS from '../keys/text-field-keys.json';
import { fieldOwnsKey, textFieldOwnsSequence } from './windowHooks';

afterEach(() => {
  cleanup();
  document.querySelectorAll('[data-test-field]').forEach((e) => {
    e.remove();
  });
});

/** A key as the browser reports it: `key`, `code` and the modifiers. */
interface Press {
  readonly name: string;
  readonly seq: string;
  readonly key: string;
  readonly code: string;
  readonly ctrlKey?: boolean;
  readonly shiftKey?: boolean;
  readonly altKey?: boolean;
}

/** The text-editing keys of Windows and Linux, each with the command the keymap binds it to in this test. */
const EDITING: readonly (Press & { command: string })[] = [
  { name: 'select all', seq: 'Primary+A', key: 'a', code: 'KeyA', ctrlKey: true, command: 'edit.selectAll' },
  { name: 'cut', seq: 'Primary+X', key: 'x', code: 'KeyX', ctrlKey: true, command: 'edit.cut' },
  { name: 'copy', seq: 'Primary+C', key: 'c', code: 'KeyC', ctrlKey: true, command: 'edit.copy' },
  { name: 'paste', seq: 'Primary+V', key: 'v', code: 'KeyV', ctrlKey: true, command: 'edit.paste' },
  { name: 'undo', seq: 'Primary+Z', key: 'z', code: 'KeyZ', ctrlKey: true, command: 'edit.undo' },
  {
    name: 'redo',
    seq: 'Primary+Shift+Z',
    key: 'Z',
    code: 'KeyZ',
    ctrlKey: true,
    shiftKey: true,
    command: 'edit.redo',
  },
  { name: 'redo (Y)', seq: 'Primary+Y', key: 'y', code: 'KeyY', ctrlKey: true, command: 'edit.redo' },
  {
    name: 'word left',
    seq: 'Primary+Left',
    key: 'ArrowLeft',
    code: 'ArrowLeft',
    ctrlKey: true,
    command: 'playback.previousKey',
  },
  {
    name: 'select word right',
    seq: 'Primary+Shift+Right',
    key: 'ArrowRight',
    code: 'ArrowRight',
    ctrlKey: true,
    shiftKey: true,
    command: 'playback.nextKey',
  },
  {
    name: 'text start',
    seq: 'Primary+Home',
    key: 'Home',
    code: 'Home',
    ctrlKey: true,
    command: 'playback.first',
  },
  {
    name: 'delete word',
    seq: 'Primary+Backspace',
    key: 'Backspace',
    code: 'Backspace',
    ctrlKey: true,
    command: 'edit.deleteBack',
  },
  { name: 'Home', seq: 'Home', key: 'Home', code: 'Home', command: 'playback.start' },
  { name: 'End', seq: 'End', key: 'End', code: 'End', command: 'playback.end' },
  {
    name: 'select to line end',
    seq: 'Shift+End',
    key: 'End',
    code: 'End',
    shiftKey: true,
    command: 'project.save',
  },
  { name: 'Delete', seq: 'Delete', key: 'Delete', code: 'Delete', command: 'edit.delete' },
  { name: 'Backspace', seq: 'Backspace', key: 'Backspace', code: 'Backspace', command: 'edit.delete' },
  { name: 'a tool letter', seq: 'B', key: 'b', code: 'KeyB', command: 'tool.active.set' },
  {
    name: 'a capital tool letter',
    seq: 'Shift+B',
    key: 'B',
    code: 'KeyB',
    shiftKey: true,
    command: 'tool.active.set',
  },
  { name: 'a digit', seq: '1', key: '1', code: 'Digit1', command: 'view.zoom.set' },
  { name: 'Space', seq: 'Space', key: ' ', code: 'Space', command: 'playback.toggle' },
];

const SAVE: Press & { command: string } = {
  name: 'save',
  seq: 'Primary+S',
  key: 's',
  code: 'KeyS',
  ctrlKey: true,
  command: 'project.save',
};

const OPEN = {
  ...NO_PROJECT_STATE,
  project: {
    id: 'pr_1',
    name: 'Walk',
    path: '/p/Walk.animpkg',
    dirty: true,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

/** D-69 item 2: function keys reach the keymap from a field (F5 bound here to a command); Shift+F10 stays the field's. */
const F5: Press & { command: string } = {
  name: 'F5',
  seq: 'F5',
  key: 'F5',
  code: 'F5',
  command: 'project.save',
};
const SHIFT_F5: Press & { command: string } = {
  name: 'Shift+F5',
  seq: 'Shift+F5',
  key: 'F5',
  code: 'F5',
  shiftKey: true,
  command: 'project.save',
};
/** Bound so that a dispatch would show; with the alternate key held it is what a dead key's physical key spells. */
const ALT_E: Press & { command: string } = {
  name: 'Alt+E',
  seq: 'Alt+E',
  key: 'e',
  code: 'KeyE',
  altKey: true,
  command: 'project.save',
};

async function app(): Promise<ScriptedHost> {
  const all = [...EDITING, SAVE, F5, SHIFT_F5, ALT_E];
  const h = createScriptedHost({
    'app.state.get': () => ok(OPEN),
    'keymap.list': () =>
      ok(all.map((k) => ({ commandId: k.command, keys: k.seq, context: 'global', isDefault: true }))),
  });
  for (const k of all)
    h.keys[k.seq] = {
      outcome: 'command',
      keys: k.seq,
      commandId: k.command,
      params: {},
      enabled: true,
      dialog: false,
    };
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  await waitFor(() => {
    expect(h.sent.some((s) => s.command === 'keymap.list')).toBe(true);
  });
  return h;
}

/** Presses `k` in `el`: whether the browser keeps its default (true: the field's text editing acts). */
function press(el: Element, k: Press): boolean {
  return fireEvent.keyDown(el, {
    key: k.key,
    code: k.code,
    ctrlKey: k.ctrlKey ?? false,
    shiftKey: k.shiftKey ?? false,
    altKey: k.altKey ?? false,
  });
}

async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 20));
  });
}

/** A text-entry element: one whose keys the field owns (a list box is not one). */
const textEntry = (e: HTMLElement): boolean =>
  !(e instanceof HTMLSelectElement) &&
  fieldOwnsKey(e, 'Primary+A', { key: 'a', altKey: false, ctrlKey: true, metaKey: false }, false);

/** Every text-entry kind the window has, as a test element: the numeric field is a text input with a decimal input
 *  mode, the pen rename field a text input, plus a search input, a text area and an editable element. */
function testFields(): HTMLElement[] {
  const make = (tag: string, attrs: Record<string, string>) => {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
    e.setAttribute('data-test-field', '');
    document.body.appendChild(e);
    return e;
  };
  return [
    make('input', { type: 'text', inputmode: 'decimal', 'aria-label': 'numeric' }),
    make('input', { type: 'text', 'aria-label': 'rename' }),
    make('input', { type: 'search', 'aria-label': 'search' }),
    make('input', { 'aria-label': 'no type' }),
    make('textarea', { 'aria-label': 'text area' }),
    make('div', { contenteditable: 'true', 'aria-label': 'editable' }),
  ];
}

describe('fieldOwnsKey', () => {
  const input = () => {
    const e = document.createElement('input');
    e.type = 'text';
    return e;
  };
  const ev = (
    key: string,
    mods: { altKey?: boolean; ctrlKey?: boolean; metaKey?: boolean; altGraph?: boolean } = {},
  ) => ({
    key,
    altKey: mods.altKey ?? false,
    ctrlKey: mods.ctrlKey ?? false,
    metaKey: mods.metaKey ?? false,
    getModifierState: (k: string) => k === 'AltGraph' && mods.altGraph === true,
  });

  it('gives a text field every text-editing key on Windows and Linux, and keeps Save for the keymap', () => {
    for (const k of EDITING)
      expect([k.seq, fieldOwnsKey(input(), k.seq, ev(k.key), false)]).toEqual([k.seq, true]);
    expect(fieldOwnsKey(input(), 'Primary+S', ev('s'), false)).toBe(false);
    expect(fieldOwnsKey(input(), 'Primary+Shift+S', ev('S'), false)).toBe(false);
    expect(fieldOwnsKey(input(), 'Primary+O', ev('o'), false)).toBe(false);
  });

  it('gives a text field the macOS text-editing keys', () => {
    for (const seq of [
      'Primary+A',
      'Primary+X',
      'Primary+C',
      'Primary+V',
      'Primary+Z',
      'Primary+Shift+Z',
      'Primary+Left',
      'Primary+Shift+Right',
      'Alt+Left',
      'Alt+Shift+Right',
      'Alt+Backspace',
      'Ctrl+A',
      'Ctrl+E',
      'Ctrl+K',
    ])
      expect([seq, fieldOwnsKey(input(), seq, ev('x'), true)]).toEqual([seq, true]);
    expect(fieldOwnsKey(input(), 'Primary+S', ev('s'), true)).toBe(false);
  });

  it('gives a text field a character typed with AltGr, or with Option on macOS', () => {
    expect(
      fieldOwnsKey(input(), 'Primary+Alt+A', ev('ą', { ctrlKey: true, altKey: true, altGraph: true }), false),
    ).toBe(true);
    expect(fieldOwnsKey(input(), 'Alt+E', ev('´', { altKey: true }), true)).toBe(true);
    // Alt with a letter on Windows is not a typed character: the menu's access keys and the keymap keep it.
    expect(fieldOwnsKey(input(), 'Alt+F', ev('f', { altKey: true }), false)).toBe(false);
  });

  it('owns nothing outside a text field, and a list box only its typing-ahead', () => {
    const button = document.createElement('button');
    const check = document.createElement('input');
    check.type = 'checkbox';
    for (const el of [button, check, document.body]) {
      expect(fieldOwnsKey(el, 'Primary+A', ev('a'), false)).toBe(false);
      expect(fieldOwnsKey(el, 'B', ev('b'), false)).toBe(false);
    }
    const select = document.createElement('select');
    expect(fieldOwnsKey(select, 'B', ev('b'), false)).toBe(true);
    expect(fieldOwnsKey(select, 'Primary+A', ev('a'), false)).toBe(false);
  });
});

describe('the key dispatcher leaves the text-editing keys to the focused field (D-66 item 2)', () => {
  it('in every text-entry kind of the main window: no command, the default kept; Save still reaches the keymap', async () => {
    const h = await app();
    for (const field of testFields()) {
      field.focus();
      for (const k of EDITING) {
        const before = h.actions().length;
        const kept = press(field, k);
        await settle();
        expect([field.getAttribute('aria-label'), k.name, kept]).toEqual([
          field.getAttribute('aria-label'),
          k.name,
          true,
        ]);
        expect([field.getAttribute('aria-label'), k.name, h.actions().slice(before)]).toEqual([
          field.getAttribute('aria-label'),
          k.name,
          [],
        ]);
      }
    }
    // A chord that is not text editing goes to the keymap from a field (Save of a dirty project).
    const [first] = testFields();
    if (!first) throw new Error('no field');
    expect(press(first, SAVE)).toBe(false);
    await waitFor(() => {
      expect(h.actions().some((a) => a.command === 'project.save')).toBe(true);
    });
  });

  it('every text-entry element the main window renders keeps them too', async () => {
    const h = await app();
    const rendered = [
      ...document.querySelectorAll<HTMLElement>('input, textarea, [contenteditable="true"]'),
    ].filter((e) => textEntry(e));
    for (const field of rendered) {
      for (const k of EDITING) {
        const before = h.actions().length;
        expect([field.dataset['ttId'] ?? field.outerHTML.slice(0, 60), k.name, press(field, k)]).toEqual([
          field.dataset['ttId'] ?? field.outerHTML.slice(0, 60),
          k.name,
          true,
        ]);
        await settle();
        expect(h.actions().slice(before)).toEqual([]);
      }
    }
  });

  it('in a Preferences numeric field (a modal dialog): select all and the rest stay the field’s', async () => {
    const h = await app();
    act(() => {
      h.event('shell.preferences', { category: 'files' });
    });
    const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    // Its numeric fields (`files.backups.count`) and any other text entry it shows.
    const fields = await waitFor(() => {
      const all = [...dialog.querySelectorAll<HTMLElement>('input, textarea')].filter((e) => textEntry(e));
      if (all.length === 0) throw new Error('no text field in Preferences › Files');
      return all;
    });
    for (const field of fields) {
      field.focus();
      for (const k of EDITING) {
        const before = h.actions().length;
        // Before D-66 the modal's guard prevented Primary+A, so the field's text was never selected.
        expect([k.name, press(field, k)]).toEqual([k.name, true]);
        await settle();
        expect(h.actions().slice(before)).toEqual([]);
      }
    }
  });
});

// D-69 item 2 (C44A-R3S-01): on macOS Option+E arrives as `key: "Dead"`, `code: "KeyE"`, `altKey: true`; the sequence
// falls back to the physical key ("Alt+E"), which the accelerator guard cancelled, so accents could not be typed.
describe('dead keys, compositions and function keys in a text field (D-69 item 2)', () => {
  const input = () => {
    const e = document.createElement('input');
    e.type = 'text';
    return e;
  };
  it("the rule: a dead key and any key of a composition are the field's; function keys are not, but Shift+F10", () => {
    const base = { altKey: false, ctrlKey: false, metaKey: false };
    expect(fieldOwnsKey(input(), 'Alt+E', { ...base, key: 'Dead', altKey: true }, true)).toBe(true);
    expect(
      fieldOwnsKey(input(), 'Primary+A', { ...base, key: 'a', ctrlKey: true, isComposing: true }, false),
    ).toBe(true);
    expect(fieldOwnsKey(input(), 'A', { ...base, key: 'Process', keyCode: 229 }, false)).toBe(true);
    expect(fieldOwnsKey(input(), 'F5', { ...base, key: 'F5' }, false)).toBe(false);
    expect(fieldOwnsKey(input(), 'Shift+F5', { ...base, key: 'F5' }, false)).toBe(false);
    expect(fieldOwnsKey(input(), 'Shift+F10', { ...base, key: 'F10' }, false)).toBe(true);
    // Outside a text field a dead key is nobody's in particular.
    expect(
      fieldOwnsKey(document.createElement('button'), 'Alt+E', { ...base, key: 'Dead', altKey: true }, true),
    ).toBe(false);
  });

  it('the macOS dead key, exactly as it arrives, types in the field: not cancelled, nothing sent', async () => {
    const h = await app();
    const [field] = testFields();
    if (!field) throw new Error('no field');
    field.focus();
    const before = h.actions().length;
    const kept = fireEvent.keyDown(field, { key: 'Dead', code: 'KeyE', altKey: true });
    await settle();
    expect(kept).toBe(true);
    expect(h.actions().slice(before)).toEqual([]);
    // The same physical key without a dead key is the bound chord, which the dispatcher sends.
    expect(fireEvent.keyDown(field, { key: 'e', code: 'KeyE', altKey: true })).toBe(false);
    await waitFor(() => {
      expect(h.actions().some((a) => a.command === 'project.save')).toBe(true);
    });
  });

  it("a composition sequence is the field's from its start to its end", async () => {
    const h = await app();
    const [field] = testFields();
    if (!field) throw new Error('no field');
    field.focus();
    const before = h.actions().length;
    const steps = [
      { key: 'Process', code: 'KeyN', keyCode: 229 },
      { key: 'Process', code: 'KeyI', keyCode: 229, isComposing: true },
      { key: 'e', code: 'KeyE', altKey: true, isComposing: true },
      { key: 'Enter', code: 'Enter', isComposing: true },
    ];
    for (const step of steps) {
      expect([step.key, fireEvent.keyDown(field, step)]).toEqual([step.key, true]);
    }
    await settle();
    expect(h.actions().slice(before)).toEqual([]);
  });

  it("function keys reach the keymap from a field; Shift+F10 stays the field's", async () => {
    const h = await app();
    const [field] = testFields();
    if (!field) throw new Error('no field');
    field.focus();
    expect(press(field, F5)).toBe(false);
    await waitFor(() => {
      expect(h.actions().filter((a) => a.command === 'project.save')).toHaveLength(1);
    });
    expect(press(field, SHIFT_F5)).toBe(false);
    await waitFor(() => {
      expect(h.actions().filter((a) => a.command === 'project.save')).toHaveLength(2);
    });
    const before = h.actions().length;
    expect(
      press(field, { name: 'Shift+F10', seq: 'Shift+F10', key: 'F10', code: 'F10', shiftKey: true }),
    ).toBe(true);
    await settle();
    expect(h.actions().slice(before)).toEqual([]);
  });
});

// D-73 item 2: one rule, read by the page and by the kernel (`keymap.rs` `text_field_owns` checks the same corpus).
describe('the shared text-field key rule (D-73 item 2)', () => {
  it('answers every corpus case as the kernel does', () => {
    for (const c of TEXT_FIELD_KEYS.corpus)
      expect([c.seq, c.mac, textFieldOwnsSequence(c.seq, c.mac)]).toEqual([c.seq, c.mac, c.owned]);
  });

  // SI-193 (finding C44B-R1S-04): the canvas's text editor gives undo and redo to the document; the kernel checks the
  // same cases (`keymap.rs` `the_canvas_text_kind_gives_undo_and_redo_to_the_document`).
  it('answers the canvas text kind as the kernel does, and an ordinary field keeps its undo', () => {
    for (const c of TEXT_FIELD_KEYS.canvasTextCorpus) {
      const owned = textFieldOwnsSequence(c.seq, c.mac, 'canvasText');
      expect([c.seq, c.mac, owned]).toEqual([c.seq, c.mac, c.owned]);
    }
    expect(textFieldOwnsSequence('Primary+Z', false)).toBe(true);
    const editor = document.createElement('textarea');
    editor.dataset['ttFieldKind'] = 'canvasText';
    const plain = document.createElement('textarea');
    const z = { key: 'z', altKey: false, ctrlKey: true, metaKey: false };
    expect(fieldOwnsKey(editor, 'Primary+Z', z, false)).toBe(false);
    expect(fieldOwnsKey(plain, 'Primary+Z', z, false)).toBe(true);
  });
});
