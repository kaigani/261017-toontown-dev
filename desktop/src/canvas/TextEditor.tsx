// The Text tool's editor (M4 C4.4b; design note 5 §4; SI-193): a hidden text area at the caret, focused while the
// session is open in this view. It holds no text: each input goes to the kernel's input path (`text_input`) and its
// value is emptied, so the browser never keeps a second copy. A composition (an input method, a dead key) is shown by
// the platform in the area, made visible at the caret at the run's size; nothing is sent until it ends, then once. The
// session's text id, anchor and caret are attributes, so `inspect_ui` shows the session.
import { useEffect, useRef, useState } from 'react';

import { copyText } from '../shell/clipboard';
import { beforeInput, editingKey, sliceCodePoints, type EditorInput } from './text';

export interface EditorSession {
  drawingId: string;
  textId: string;
  anchor: number;
  caret: number;
  /** View pixels: the caret's top-left and the run's size. */
  x: number;
  y: number;
  size: number;
}

export function TextEditor({
  session,
  focused,
  mac,
  send,
  readText,
}: {
  session: EditorSession | null;
  /** The session is open in this view (its editor takes the focus here only). */
  focused: boolean;
  mac: boolean;
  /** One input into the kernel's input path. */
  send: (input: EditorInput) => void;
  /** The session object's text (for Copy and Cut, sliced by code points). */
  readText: () => Promise<string>;
}) {
  const area = useRef<HTMLTextAreaElement | null>(null);
  const [composing, setComposing] = useState(false);
  const sendRef = useRef(send);
  const sessionRef = useRef(session);
  const readRef = useRef(readText);
  useEffect(() => {
    sendRef.current = send;
    sessionRef.current = session;
    readRef.current = readText;
  });
  const open = session !== null;
  useEffect(() => {
    if (open && focused) area.current?.focus({ preventScroll: true });
  }, [open, focused, session?.textId]);
  useEffect(() => {
    const el = area.current;
    if (!el) return undefined;
    const clear = () => {
      el.value = '';
    };
    const highlight = async (): Promise<string> => {
      const s = sessionRef.current;
      if (!s || s.anchor === s.caret) return '';
      const t = await readRef.current();
      return sliceCodePoints(t, Math.min(s.anchor, s.caret), Math.max(s.anchor, s.caret));
    };
    const onBeforeInput = (e: InputEvent) => {
      if (e.isComposing) return;
      const i = beforeInput(e.inputType, e.data);
      if (i) {
        e.preventDefault();
        sendRef.current(i);
      }
      clear();
    };
    const onCompositionStart = () => {
      setComposing(true);
    };
    const onCompositionEnd = (e: CompositionEvent) => {
      setComposing(false);
      if (e.data) sendRef.current({ kind: 'text', text: e.data });
      clear();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.isComposing || e.key === 'Process' || e.key === 'Dead') return;
      const k = editingKey(e, mac);
      if (k === null) return;
      if (k === 'paste') {
        // The browser's paste follows (its `paste` event sends it); the keymap's Paste does not act on the drawing.
        e.stopPropagation();
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      if (k === 'copy') {
        void highlight().then((t) => (t ? copyText(t) : undefined));
        return;
      }
      if (k === 'cut') {
        void highlight().then(async (t) => {
          if (!t) return;
          await copyText(t);
          sendRef.current({ kind: 'cut' });
        });
        return;
      }
      sendRef.current({ kind: 'key', keys: k });
    };
    const onPaste = (e: ClipboardEvent) => {
      e.preventDefault();
      const t = e.clipboardData?.getData('text/plain') ?? '';
      if (t) sendRef.current({ kind: 'paste', text: t });
      clear();
    };
    const onCopy = (e: ClipboardEvent) => {
      e.preventDefault();
      void highlight().then((t) => (t ? copyText(t) : undefined));
    };
    el.addEventListener('beforeinput', onBeforeInput);
    el.addEventListener('compositionstart', onCompositionStart);
    el.addEventListener('compositionend', onCompositionEnd);
    el.addEventListener('keydown', onKeyDown);
    el.addEventListener('paste', onPaste);
    el.addEventListener('copy', onCopy);
    return () => {
      el.removeEventListener('beforeinput', onBeforeInput);
      el.removeEventListener('compositionstart', onCompositionStart);
      el.removeEventListener('compositionend', onCompositionEnd);
      el.removeEventListener('keydown', onKeyDown);
      el.removeEventListener('paste', onPaste);
      el.removeEventListener('copy', onCopy);
    };
  }, [mac]);
  if (!session) return null;
  return (
    <textarea
      ref={area}
      className={composing ? 'text-editor composing' : 'text-editor'}
      data-tt-role="textEditor"
      data-tt-id="canvas.textEditor"
      data-tt-label="Text editor"
      data-tt-text-id={`${session.drawingId}/${session.textId}`}
      data-tt-anchor={session.anchor}
      data-tt-caret={session.caret}
      data-tt-command="draw.text.set"
      aria-label="Text editor"
      autoComplete="off"
      autoCorrect="off"
      spellCheck={false}
      rows={1}
      style={{
        position: 'absolute',
        left: session.x,
        top: session.y,
        height: session.size,
        fontSize: session.size,
        lineHeight: 1,
        width: composing ? 'auto' : 1,
        minWidth: composing ? 40 : 1,
        opacity: composing ? 1 : 0,
        resize: 'none',
        overflow: 'hidden',
        border: 'none',
        padding: 0,
        margin: 0,
        background: composing ? 'var(--panel-bg, #fff)' : 'transparent',
        font: composing ? `${String(session.size)}px system-ui, sans-serif` : undefined,
        pointerEvents: 'none',
        zIndex: 2,
      }}
    />
  );
}
