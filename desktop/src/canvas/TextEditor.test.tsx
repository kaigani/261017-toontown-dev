// The Text tool's editor (M4 C4.4b; design note 5 §4, §8): the input-type mapping, one send per composition, the value
// emptied after each input, Primary+A/X/C/V taken by the editor and Primary+Z left to the keymap, and the listeners
// attached when the session appears (the editor mounts before any session).
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TextEditor, type EditorSession } from './TextEditor';
import { beforeInput, editingKey } from './text';

afterEach(cleanup);

const SESSION: EditorSession = {
  drawingId: 'dr_1',
  textId: 'nd_1',
  anchor: 0,
  caret: 0,
  x: 10,
  y: 20,
  size: 36,
};

function mount() {
  const sent: unknown[] = [];
  const send = vi.fn((input: unknown) => sent.push(input));
  const props = { focused: true, mac: false, send, readText: () => Promise.resolve('Hello') };
  const view = render(<TextEditor session={null} {...props} />);
  // The session appears after the editor mounted (a Text tool click).
  view.rerender(<TextEditor session={SESSION} {...props} />);
  const area = document.querySelector<HTMLTextAreaElement>('[data-tt-id="canvas.textEditor"]');
  if (!area) throw new Error('no editor');
  return { area, sent, view, props };
}

function input(area: HTMLTextAreaElement, inputType: string, data: string | null, isComposing = false) {
  const e = new InputEvent('beforeinput', { inputType, data, isComposing, cancelable: true, bubbles: true });
  act(() => {
    area.dispatchEvent(e);
  });
  return e;
}

describe('TextEditor', () => {
  it('sends typed text once the session appears, and empties its value', () => {
    const { area, sent } = mount();
    expect(document.activeElement).toBe(area);
    area.value = 'x';
    const e = input(area, 'insertText', 'H');
    expect(sent).toEqual([{ kind: 'text', text: 'H' }]);
    expect(e.defaultPrevented).toBe(true);
    expect(area.value).toBe('');
    expect(area.getAttribute('data-tt-text-id')).toBe('dr_1/nd_1');
  });

  it('maps line breaks and deletions to the editing keys', () => {
    const { area, sent } = mount();
    input(area, 'insertLineBreak', null);
    input(area, 'insertParagraph', null);
    input(area, 'deleteContentBackward', null);
    input(area, 'deleteContentForward', null);
    expect(sent).toEqual([
      { kind: 'key', keys: 'Enter' },
      { kind: 'key', keys: 'Enter' },
      { kind: 'key', keys: 'Backspace' },
      { kind: 'key', keys: 'Delete' },
    ]);
  });

  it('sends a composition once, at its end, and nothing while composing', () => {
    const { area, sent } = mount();
    act(() => {
      area.dispatchEvent(new CompositionEvent('compositionstart', { data: '' }));
    });
    expect(area.className).toContain('composing');
    input(area, 'insertCompositionText', 'に', true);
    input(area, 'insertCompositionText', 'にほ', true);
    expect(sent).toEqual([]);
    act(() => {
      area.dispatchEvent(new CompositionEvent('compositionend', { data: '日本' }));
    });
    expect(sent).toEqual([{ kind: 'text', text: '日本' }]);
    expect(area.className).not.toContain('composing');
  });

  it('takes the editing keys and Primary+A, X, C; leaves Primary+V to the paste and Primary+Z to the keymap', () => {
    const { area, sent } = mount();
    const key = (init: KeyboardEventInit) => {
      const e = new KeyboardEvent('keydown', { ...init, cancelable: true, bubbles: true });
      act(() => {
        area.dispatchEvent(e);
      });
      return e;
    };
    expect(key({ key: 'ArrowLeft', shiftKey: true }).defaultPrevented).toBe(true);
    expect(key({ key: 'a', ctrlKey: true }).defaultPrevented).toBe(true);
    expect(key({ key: 'c', ctrlKey: true }).defaultPrevented).toBe(true);
    expect(key({ key: 'v', ctrlKey: true }).defaultPrevented).toBe(false);
    expect(key({ key: 'z', ctrlKey: true }).defaultPrevented).toBe(false);
    expect(sent).toEqual([
      { kind: 'key', keys: 'Shift+Left' },
      { kind: 'key', keys: 'Primary+A' },
    ]);
  });

  it('pastes the clipboard text as one input', () => {
    const { area, sent } = mount();
    const e = new Event('paste', { cancelable: true, bubbles: true }) as ClipboardEvent;
    Object.defineProperty(e, 'clipboardData', { value: { getData: () => 'pasted' } });
    act(() => {
      area.dispatchEvent(e);
    });
    expect(sent).toEqual([{ kind: 'paste', text: 'pasted' }]);
  });
});

describe('the editor mapping (text.ts)', () => {
  it('maps keys by platform', () => {
    expect(
      editingKey({ key: 'a', metaKey: true, ctrlKey: false, shiftKey: false, altKey: false }, true),
    ).toBe('Primary+A');
    expect(
      editingKey({ key: 'a', metaKey: false, ctrlKey: true, shiftKey: false, altKey: false }, true),
    ).toBeNull();
    expect(
      editingKey({ key: 'Tab', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }, false),
    ).toBe('Tab');
    expect(
      editingKey({ key: 'x', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }, false),
    ).toBeNull();
    expect(beforeInput('insertFromPaste', 'x')).toBeNull();
  });
});
