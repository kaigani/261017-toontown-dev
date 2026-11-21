// The Text tool in the window (M4 C4.4b; design note 5 §4, §5; SI-192, SI-193, SI-194): what the canvas draws for the
// edit session and the cursor it shows, from the scene answer's `textEdit` and `textBoxes` (drawing units) through the
// view's drawing-to-view mapping. The kernel is the only layout: nothing here measures a glyph. The editor's input
// mapping (`editorInput`) turns the hidden text area's events into the kernel's input path (`text_input`).

import { apply, ViewTris, type Affine } from './geometry';
import type { Rgba } from './format';
import type { SceneTextBox, SceneTextEdit } from './scene';
import { SELECTION_ACCENT } from './selection';

type Pt = [number, number];

/** Hit radii in view pixels: the width handles (SI-131) and the frame band (SI-194); the kernel's `text_edit.rs`. */
export const HANDLE_PX = 6;
export const FRAME_PX = 4;
const HANDLE_SIZE = 8;
const WHITE: Rgba = [255, 255, 255, 255];
const HIGHLIGHT: Rgba = [32, 128, 255, 80];

const map = (d2v: Affine, p: readonly [number, number]): Pt => apply(d2v, p[0], p[1]);

/** The session's overlay (design note 5 §4 "Drawing"): the highlight, the steady caret, the edited box's frame and its
 *  two width handles. */
export function textOverlay(t: SceneTextEdit | null, d2v: Affine): ViewTris | null {
  if (!t) return null;
  const out = new ViewTris();
  for (const q of t.highlight) {
    const v = q.map((p) => map(d2v, p)) as [Pt, Pt, Pt, Pt];
    out.tri(v[0][0], v[0][1], v[1][0], v[1][1], v[2][0], v[2][1], HIGHLIGHT);
    out.tri(v[0][0], v[0][1], v[2][0], v[2][1], v[3][0], v[3][1], HIGHLIGHT);
  }
  out.polyline(
    t.quad.map((p) => map(d2v, p)),
    1,
    SELECTION_ACCENT,
    true,
  );
  const [a, b] = t.caret.map((p) => map(d2v, p)) as [Pt, Pt];
  out.line(a[0], a[1], b[0], b[1], 1.5, [0, 0, 0, 255]);
  for (const h of t.handles) {
    const [x, y] = map(d2v, h);
    out.square(x, y, HANDLE_SIZE, WHITE, SELECTION_ACCENT);
  }
  return out;
}

function distToSegment(p: Pt, a: Pt, b: Pt): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const l2 = dx * dx + dy * dy;
  const t = l2 > 0 ? Math.min(1, Math.max(0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
  return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
}

function insideQuad(p: Pt, q: readonly Pt[]): boolean {
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = q[i] ?? [0, 0];
    const b = q[(i + 1) % 4] ?? [0, 0];
    const c = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    if (c === 0) continue;
    if (sign === 0) sign = Math.sign(c);
    else if (Math.sign(c) !== sign) return false;
  }
  return true;
}

/** The cursor over view point `p` (design note 5 §4), in the kernel's zone order: a width handle of the edited box
 *  resizes, a box's frame moves, inside a box the I-beam, else the default. */
export function textCursor(
  t: SceneTextEdit | null,
  boxes: SceneTextBox[] | null,
  d2v: Affine,
  p: Pt,
): string {
  if (t) {
    for (const h of t.handles) {
      const [x, y] = map(d2v, h);
      if (Math.hypot(x - p[0], y - p[1]) <= HANDLE_PX) return 'ew-resize';
    }
  }
  const all = [...(boxes ?? [])].reverse();
  for (const b of all) {
    const v = b.quad.map((q) => map(d2v, q));
    for (let i = 0; i < 4; i++)
      if (distToSegment(p, v[i] ?? [0, 0], v[(i + 1) % 4] ?? [0, 0]) <= FRAME_PX) return 'move';
  }
  for (const b of all)
    if (
      insideQuad(
        p,
        b.quad.map((q) => map(d2v, q)),
      )
    )
      return 'text';
  return '';
}

/** Where the editor sits (design note 5 §4): at the caret's top in view pixels, the run's size in view pixels (the
 *  pre-edit's size during a composition). */
export function editorPlace(t: SceneTextEdit, d2v: Affine): { x: number; y: number; size: number } {
  const [a, b] = t.caret.map((p) => map(d2v, p)) as [Pt, Pt];
  return { x: a[0], y: a[1], size: Math.max(8, Math.hypot(b[0] - a[0], b[1] - a[1])) };
}

/** One input of the kernel's input path (`text_input`). */
export type EditorInput =
  | { kind: 'text'; text: string }
  | { kind: 'key'; keys: string }
  | { kind: 'paste'; text: string }
  | { kind: 'cut' };

const KEYS: Record<string, string> = {
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  Home: 'Home',
  End: 'End',
};

/** The editing key of a keydown (design note 5 §4): the arrows, Home and End (Shift extends), Escape, Tab, Primary+A;
 *  the words `copy`, `cut` and `paste` for Primary+C, X and V; null for every other key (typed characters arrive as `beforeinput`,
 *  Primary+Z and the other shortcuts reach the keymap). */
export function editingKey(
  e: Pick<KeyboardEvent, 'key' | 'shiftKey' | 'ctrlKey' | 'metaKey' | 'altKey'>,
  mac: boolean,
): string | null {
  const primary = mac ? e.metaKey : e.ctrlKey;
  const arrow = e.key === 'ArrowLeft' || e.key === 'ArrowRight';
  const side = e.key === 'ArrowLeft' ? 'Left' : 'Right';
  const shift = e.shiftKey ? 'Shift+' : '';
  if (primary && !e.altKey) {
    const k = e.key.toLowerCase();
    if (k === 'a') return 'Primary+A';
    if (k === 'c') return 'copy';
    if (k === 'x') return 'cut';
    if (k === 'v') return 'paste';
    // Word navigation and deletion (01 §8.4; D-81 item 4b): Ctrl on Windows and Linux; on macOS Command with the
    // arrows goes to the line's start or end.
    if (arrow) return mac ? `${shift}${e.key === 'ArrowLeft' ? 'Home' : 'End'}` : `Primary+${shift}${side}`;
    if (!mac && (e.key === 'Backspace' || e.key === 'Delete')) return `Primary+${e.key}`;
    return null;
  }
  // macOS: Option with the arrows, Backspace and Delete works by words.
  if (mac && e.altKey && !e.ctrlKey && !e.metaKey) {
    if (arrow) return `Alt+${shift}${side}`;
    if (e.key === 'Backspace' || e.key === 'Delete') return `Alt+${e.key}`;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return null;
  const nav = KEYS[e.key];
  if (nav) return e.shiftKey ? `Shift+${nav}` : nav;
  if (e.key === 'Escape') return 'Escape';
  if (e.key === 'Tab') return 'Tab';
  return null;
}

/** The input of a `beforeinput` (design note 5 §4), or null for the types the editor leaves (a composition's own
 *  updates arrive once at `compositionend`; a paste at `paste`). */
export function beforeInput(inputType: string, data: string | null): EditorInput | null {
  switch (inputType) {
    case 'insertText':
    case 'insertReplacementText':
      return data ? { kind: 'text', text: data } : null;
    case 'insertLineBreak':
    case 'insertParagraph':
      return { kind: 'key', keys: 'Enter' };
    case 'deleteContentBackward':
      return { kind: 'key', keys: 'Backspace' };
    case 'deleteContentForward':
      return { kind: 'key', keys: 'Delete' };
    default:
      return null;
  }
}

/** Joins UTF-16 halves split across inputs (D-81 item 4a): a character outside the basic plane can arrive as a high
 *  surrogate then a low one in two events. `held` is the high surrogate kept from the previous input; returns the text
 *  to send (whole characters only) and the high surrogate to keep. A half is never sent: a held high surrogate not
 *  followed by its low one, and a low one without its high one, are dropped (neither is a character). */
export function joinSurrogates(held: string, data: string): { send: string; held: string } {
  let t = held + data;
  let keep = '';
  const last = t.charCodeAt(t.length - 1);
  if (t.length > 0 && last >= 0xd800 && last <= 0xdbff) {
    keep = t.slice(-1);
    t = t.slice(0, -1);
  }
  // Any remaining unpaired half (a lone low surrogate, or a high one followed by something else) is not a character.
  let send = '';
  for (let i = 0; i < t.length; i++) {
    const c = t.charCodeAt(i);
    const high = c >= 0xd800 && c <= 0xdbff;
    const low = c >= 0xdc00 && c <= 0xdfff;
    const next = t.charCodeAt(i + 1);
    if (high && next >= 0xdc00 && next <= 0xdfff) {
      send += t.slice(i, i + 2);
      i++;
    } else if (!high && !low) {
      send += t.charAt(i);
    }
  }
  return { send, held: keep };
}

/** The highlighted characters, sliced by code points (the kernel's positions). */
export function sliceCodePoints(text: string, start: number, end: number): string {
  return Array.from(text).slice(start, end).join('');
}
