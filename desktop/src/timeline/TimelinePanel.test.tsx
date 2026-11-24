// The Timeline panel follows an agent's change of each preference it shows (ruling E-6; 01 §7.3): the background
// (`interface.background.timeline`), the ruler's labels (`interface.timeMeasurement`) and the rows' marker colours
// (`interface.elementColours.*`), each after a `prefs.changed` event alone (no click in the window). The fourth,
// `timing.timeline.singleCellSelection`, is the kernel's to apply in the gesture (crates/executor/tests/
// c51b_single_cell_follows.rs).
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, ok, type ScriptedHost } from '../test-support/scriptedHost';
import { cssColour } from './model';

afterEach(cleanup);

const PROJECT = {
  project: {
    id: 'pr_1',
    name: 'Walk',
    path: '/p/Walk.animpkg',
    dirty: false,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

const BOB = {
  elementId: 'el_bob',
  name: 'Bob',
  type: 'drawing',
  depth: 0,
  parentId: null,
  collapsed: false,
  visibleInScene: true,
  hiddenByParent: false,
  locked: false,
  bar: null,
  keyframes: [],
  childKeyframeFrames: [],
};

function feet(f: number): string {
  return `${Math.floor(f / 16)}+${String(f % 16).padStart(2, '0')}`;
}

async function timeline(
  prefs: Record<string, string>,
  rowsNow: () => unknown[] = () => [BOB],
): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok({ ...PROJECT }),
    'timeline.rows.list': () =>
      ok({
        sceneId: 'sc_1',
        sceneLength: 24,
        currentFrame: 1,
        playbackRange: { mode: 'auto', start: 1, end: 24 },
        rows: rowsNow(),
      }),
    'timeline.view.set': () => ok({ zoomLevel: 6, firstFrame: 1, lastFrame: 40 }),
    'cell.selection.get': () =>
      ok({ elementIds: [], from: 0, to: 0, rowsSelected: 0, currentElementId: 'el_bob', currentFrame: 1 }),
    'prefs.get': (p) => {
      const keys = Array.isArray(p['keys']) ? (p['keys'] as string[]) : Object.keys(prefs);
      return ok({ values: Object.fromEntries(keys.map((k) => [k, prefs[k] ?? null])) });
    },
    'timeline.playbackRange.get': () => ok({ mode: 'auto', start: 1, end: 24 }),
    'xsheet.exposures.get': () => ok({ columns: [] }),
    'element.get': () =>
      ok({
        effectiveMarkerColour: prefs['interface.elementColours.drawing'],
        backgroundColour: null,
        note: '',
      }),
    'timeline.frameLabel.get': (p) => {
      const frames = Array.isArray(p['frames']) ? (p['frames'] as number[]) : [];
      const byFeet = prefs['interface.timeMeasurement'] === 'feetAndFrames';
      return ok({ labels: frames.map((f) => (byFeet ? feet(f) : String(f))) });
    },
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App floatPanel="timeline" />
    </HostProvider>,
  );
  await waitFor(() => {
    expect(document.querySelector('[data-tt-id="timeline.row.el_bob"]')).not.toBeNull();
  });
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};

/** The ruler's first label as the window shows it (its `data-tt-labels`). */
function firstLabel(): unknown {
  const labels = JSON.parse(el('timeline.ruler').dataset['ttLabels'] ?? '[]') as unknown;
  return Array.isArray(labels) ? (labels[0] as unknown) : undefined;
}

function agentChanges(h: ScriptedHost, prefs: Record<string, string>, key: string, value: string): void {
  prefs[key] = value;
  h.event('prefs.changed', { keys: [key] });
}

describe('the Timeline follows an agent’s change of its preferences (E-6)', () => {
  it('the background', async () => {
    const prefs = { 'interface.background.timeline': '#F0F0F0FF' };
    const h = await timeline(prefs);
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttBackground']).toBe(cssColour('#F0F0F0FF'));
    });
    agentChanges(h, prefs, 'interface.background.timeline', '#202020FF');
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttBackground']).toBe(cssColour('#202020FF'));
    });
  });

  it('the ruler’s labels: frames, then feet and frames', async () => {
    const prefs = { 'interface.timeMeasurement': 'frames' };
    const h = await timeline(prefs);
    await waitFor(() => {
      expect(firstLabel()).toBe('1');
    });
    agentChanges(h, prefs, 'interface.timeMeasurement', 'feetAndFrames');
    await waitFor(() => {
      expect(firstLabel()).toBe('0+01');
    });
  });

  it('a row’s marker colour from the type’s default colour', async () => {
    const prefs = { 'interface.elementColours.drawing': '#8FD98FFF' };
    const h = await timeline(prefs);
    await waitFor(() => {
      // The style as the page normalises it (an opaque colour reads as rgb()).
      expect(el('timeline.row.el_bob').style.borderLeft).toContain('rgb(143, 217, 143)');
    });
    agentChanges(h, prefs, 'interface.elementColours.drawing', '#FF0000FF');
    await waitFor(() => {
      expect(el('timeline.row.el_bob').style.borderLeft).toContain('rgb(255, 0, 0)');
    });
  });
});

// Ruling E-40, the window's side: an agent's change of the effect-parameter setting (`timeline.effectParameterRows.set`)
// raises `shell.timelineView`, and the Timeline reads its rows again: the hidden parameter row leaves the layer list.
describe('the Timeline follows an agent’s change of the effect-parameter setting (E-40)', () => {
  it('a hidden parameter row leaves the layer list', async () => {
    const clip = {
      ...BOB,
      elementId: 'el_clip',
      name: 'Clip',
      type: 'clipping',
      bar: { start: 1, end: 24, loops: 1 },
    };
    const mask = {
      ...BOB,
      elementId: 'el_mask',
      name: 'Clip-mask',
      type: 'effectParameter',
      depth: 1,
      parentId: 'el_clip',
    };
    let shown = true;
    const h = await timeline({}, () => (shown ? [clip, mask, BOB] : [clip, BOB]));
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="timeline.row.el_mask"]')).not.toBeNull();
    });
    shown = false;
    h.event('shell.timelineView', {
      sceneId: 'sc_1',
      view: { zoomLevel: 6, firstFrame: 1, lastFrame: 40 },
      effectParameterRowsHidden: true,
      effectParameterRows: {},
    });
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="timeline.row.el_mask"]')).toBeNull();
    });
    expect(document.querySelector('[data-tt-id="timeline.row.el_clip"]')).not.toBeNull();
  });
});

// Plan §6.5: the arrow keys in the Timeline move the current cell, a frame left or right, a shown row up or down.
describe('the arrow keys in the Timeline', () => {
  it('move the current cell; a slider keeps its own arrows', async () => {
    const second = { ...BOB, elementId: 'el_two', name: 'Two' };
    const h = await timeline({}, () => [BOB, second]);
    const sent = () => h.actions().filter((a) => a.command === 'xsheet.currentCell.set');
    const tracks = el('timeline.tracks');
    act(() => {
      tracks.focus();
      fireEvent.keyDown(tracks, { key: 'ArrowRight' });
    });
    await waitFor(() => {
      expect(sent().at(-1)?.params).toEqual({ elementId: 'el_bob', frame: 2 });
    });
    act(() => {
      fireEvent.keyDown(tracks, { key: 'ArrowDown' });
    });
    await waitFor(() => {
      expect(sent().at(-1)?.params).toEqual({ elementId: 'el_two', frame: 1 });
    });
    const before = sent().length;
    act(() => {
      fireEvent.keyDown(el('timeline.zoom'), { key: 'ArrowRight' });
    });
    expect(sent().length).toBe(before);
  });
});

// A frame change (ten a second during playback) moves the playhead only: re-reading the rows and the visible rows'
// fields for each one saturated the window during playback on a slow machine (run 45304471459).
describe('a current-frame change', () => {
  it('moves the playhead without reading the rows again', async () => {
    const h = await timeline({});
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('1');
    });
    const reads = () =>
      h.sent.filter((x) => x.command === 'timeline.rows.list' || x.command === 'element.get').length;
    const before = reads();
    act(() => {
      h.event('timeline.currentFrame.changed', { sceneId: 'sc_1', frame: 9, source: 'playback' });
    });
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('9');
    });
    expect(reads()).toBe(before);
    // The app-state notice of the frame alone, and a selection notice: no rows read.
    act(() => {
      // The kernel's notice carries the whole state (the store takes it as the app state).
      h.event('app.state.changed', { changed: ['frame'], state: { ...PROJECT, frame: 12 } });
      h.event('cell.selection.changed', {});
    });
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('12');
    });
    await new Promise((r) => setTimeout(r, 300));
    expect(reads()).toBe(before);
  });
});

// Plan §6.2's window budgets on the plan's scene (1,003 rows): the engine runs one command at a time, so a read the
// draw does not need delays the one it does. A click's app-state notice (the current element, the selection) reads the
// selection alone; a view notice that moves the view alone sets it; one rows read is in flight at a time.
describe('the Timeline reads only what a notice changes', () => {
  const settle = () => new Promise((r) => setTimeout(r, 300));
  const count = (h: ScriptedHost, command: string) => h.sent.filter((x) => x.command === command).length;

  it('a click’s app-state notice reads the selection, not the rows', async () => {
    const h = await timeline({});
    await settle();
    const rows = count(h, 'timeline.rows.list');
    const selection = count(h, 'cell.selection.get');
    act(() => {
      h.event('app.state.changed', {
        changed: ['element', 'selection', 'focusedPanel'],
        state: { ...PROJECT },
      });
    });
    await waitFor(() => {
      expect(count(h, 'cell.selection.get')).toBeGreaterThan(selection);
    });
    await settle();
    expect(count(h, 'timeline.rows.list')).toBe(rows);
    // A field the rows depend on (the scene) reads everything again.
    act(() => {
      h.event('app.state.changed', { changed: ['scene'], state: { ...PROJECT } });
    });
    await waitFor(() => {
      expect(count(h, 'timeline.rows.list')).toBeGreaterThan(rows);
    });
  });

  it('a view notice that moves the view alone reads the rows for the new frames only', async () => {
    const h = await timeline({});
    const notice = (firstFrame: number, effectParameterRowsHidden = false) => ({
      sceneId: 'sc_1',
      view: { zoomLevel: 6, firstFrame, lastFrame: firstFrame + 39 },
      revealElementId: null,
      revealCount: 0,
      soundWaveforms: true,
      effectParameterRowsHidden,
      effectParameterRows: {},
    });
    act(() => {
      h.event('shell.timelineView', notice(1));
    });
    await settle();
    const selection = count(h, 'cell.selection.get');
    const prefs = count(h, 'prefs.get');
    act(() => {
      h.event('shell.timelineView', notice(10));
    });
    await waitFor(() => {
      const last = h.sent.filter((x) => x.command === 'timeline.rows.list').at(-1);
      expect(last?.params['from']).toBe(10);
    });
    await settle();
    expect(count(h, 'cell.selection.get')).toBe(selection);
    expect(count(h, 'prefs.get')).toBe(prefs);
    // A change of a display toggle in the notice reads everything again (E-40).
    act(() => {
      h.event('shell.timelineView', notice(10, true));
    });
    await waitFor(() => {
      expect(count(h, 'prefs.get')).toBeGreaterThan(prefs);
    });
  });

  it('keeps one rows read in flight and sends the newest waiting request when it returns', async () => {
    const h = await timeline({});
    await settle();
    const original = h.handlers['timeline.rows.list'];
    if (!original) throw new Error('no rows handler');
    const answers: (() => void)[] = [];
    h.handlers['timeline.rows.list'] = (p, s) =>
      new Promise((resolve) => {
        answers.push(() => {
          resolve(original(p, s));
        });
      });
    const before = count(h, 'timeline.rows.list');
    act(() => {
      h.event('doc.changed', {});
      h.event('doc.changed', {});
      h.event('doc.changed', {});
    });
    await waitFor(() => {
      expect(count(h, 'timeline.rows.list')).toBe(before + 1);
    });
    await settle();
    expect(count(h, 'timeline.rows.list')).toBe(before + 1);
    h.handlers['timeline.rows.list'] = original;
    act(() => {
      answers.shift()?.();
    });
    await waitFor(() => {
      expect(count(h, 'timeline.rows.list')).toBe(before + 2);
    });
    await settle();
    expect(count(h, 'timeline.rows.list')).toBe(before + 2);
  });
});

// C51B-R1S-02: frame notices and rows answers in one order. A rows answer whose read was sent before a frame notice
// arrives late: it must not move the playhead back to its older frame.
describe('a rows answer older than a frame notice', () => {
  it('keeps the newer frame', async () => {
    const h = await timeline({});
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('1');
    });
    await new Promise((r) => setTimeout(r, 300));
    const original = h.handlers['timeline.rows.list'];
    if (!original) throw new Error('no rows handler');
    const answers: (() => void)[] = [];
    h.handlers['timeline.rows.list'] = (p, s) =>
      new Promise((resolve) => {
        answers.push(() => {
          resolve(original(p, s));
        });
      });
    const before = h.sent.filter((x) => x.command === 'timeline.rows.list').length;
    act(() => {
      h.event('doc.changed', {});
    });
    await waitFor(() => {
      expect(h.sent.filter((x) => x.command === 'timeline.rows.list').length).toBe(before + 1);
    });
    // The frame moves while the read is out (its answer still says frame 1).
    act(() => {
      h.event('timeline.currentFrame.changed', { sceneId: 'sc_1', frame: 20, source: 'playback' });
    });
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('20');
    });
    h.handlers['timeline.rows.list'] = original;
    act(() => {
      answers.shift()?.();
    });
    await new Promise((r) => setTimeout(r, 300));
    expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('20');
  });
});

// C51B-R1S-01: Show Timeline Selection scrolls the revealed row into the visible window, also when it is repeated for
// the same row.
describe('Show Timeline Selection', () => {
  it('scrolls the list to the revealed row, each time', async () => {
    // Bob first (the harness waits for his row), then 59 more rows.
    const many = [
      BOB,
      ...Array.from({ length: 59 }, (_, n) => ({
        ...BOB,
        elementId: `el_r${String(n + 1)}`,
        name: `R${String(n + 1)}`,
      })),
    ];
    const h = await timeline({}, () => many);
    const list = el('timeline.layers');
    const notice = (count: number) => ({
      sceneId: 'sc_1',
      view: { zoomLevel: 6, firstFrame: 1, lastFrame: 40 },
      revealElementId: count > 0 ? 'el_r50' : null,
      revealCount: count,
      soundWaveforms: true,
      effectParameterRowsHidden: false,
      effectParameterRows: {},
    });
    act(() => {
      h.event('shell.timelineView', notice(0));
    });
    await new Promise((r) => setTimeout(r, 200));
    expect(list.scrollTop).toBe(0);
    act(() => {
      h.event('shell.timelineView', notice(1));
    });
    await waitFor(() => {
      expect(list.scrollTop).toBeGreaterThan(0);
    });
    // The person scrolls back up; the same row revealed again scrolls again.
    act(() => {
      list.scrollTop = 0;
      fireEvent.scroll(list);
    });
    act(() => {
      h.event('shell.timelineView', notice(2));
    });
    await waitFor(() => {
      expect(list.scrollTop).toBeGreaterThan(0);
    });
  });
});

// C51B-R1S-05 (04 lines 405-408): a dialog that edits an existing value opens with it. With a note, the row context's
// item and the dialog say Update Element Note, the editor shows the note, and OK without typing keeps it.
describe('the dialogs that edit an existing value', () => {
  it('prefill the element note and call it Update', async () => {
    const h = await timeline({});
    h.handlers['element.get'] = () =>
      ok({
        effectiveMarkerColour: '#FF8800FF',
        markerColour: '#112233FF',
        backgroundColour: null,
        note: 'Keep this',
      });
    act(() => {
      h.event('element.state.changed', {});
    });
    await waitFor(() => {
      expect(h.sent.filter((x) => x.command === 'element.get').length).toBeGreaterThan(1);
    });
    await new Promise((r) => setTimeout(r, 200));
    act(() => {
      fireEvent.contextMenu(el('timeline.row.el_bob'), { clientX: 30, clientY: 30 });
    });
    const item = await waitFor(() => el('timeline.rowMenu.timeline.row.note'));
    expect(item.textContent).toContain('Update Element Note');
    act(() => {
      item.click();
    });
    const field = await waitFor(() => el('timelineNote.value'));
    expect((field as HTMLTextAreaElement).value).toBe('Keep this');
    expect(document.body.textContent).toContain('Update Element Note');
    act(() => {
      el('timelineNote.submit').click();
    });
    await waitFor(() => {
      expect(h.actions().find((a) => a.command === 'element.note.set')?.params).toEqual({
        elementId: 'el_bob',
        text: 'Keep this',
      });
    });
  });

  // C51B-R1S-06: the frame dialogs use the one numeric field: an entry that is not a number restores the value held.
  it('Set Current Frame starts at the current frame and keeps its value on a bad entry', async () => {
    const h = await timeline({});
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('1');
    });
    act(() => {
      h.event('timeline.currentFrame.changed', { sceneId: 'sc_1', frame: 5, source: 'user' });
    });
    await waitFor(() => {
      expect(el('timeline.tracks').dataset['ttCurrentFrame']).toBe('5');
    });
    act(() => {
      fireEvent.contextMenu(el('timeline.ruler'), { clientX: 100, clientY: 5 });
    });
    const item = await waitFor(() => el('timeline.rulerMenu.timeline.ruler.currentFrame'));
    act(() => {
      item.click();
    });
    const field = (await waitFor(() => el('timelineCurrentFrame.value'))) as HTMLInputElement;
    expect(field.value).toBe('5');
    act(() => {
      fireEvent.change(field, { target: { value: 'abc' } });
      fireEvent.blur(field);
    });
    await waitFor(() => {
      expect(field.value).toBe('5');
    });
    expect(field.dataset['ttProblem']).toBeTruthy();
    act(() => {
      fireEvent.change(field, { target: { value: '7' } });
      fireEvent.blur(field);
    });
    act(() => {
      el('timelineCurrentFrame.submit').click();
    });
    await waitFor(() => {
      expect(h.actions().find((a) => a.command === 'timeline.currentFrame.set')?.params).toEqual({
        frame: 7,
      });
    });
  });
});

// C5.1b round 1 observation: the fonts-ready notice is an availability event. A control a `fontsLoading` precheck
// disabled while the font list was read is asked again when the list is read.
describe('the fonts-ready notice', () => {
  it('reads the availability again', async () => {
    const h = await timeline({});
    let fontsRead = false;
    const before = h.available;
    h.available = (ref) =>
      ref.commandId === 'element.solo.set' && !fontsRead
        ? { enabled: false, reason: 'the system font list is still being read' }
        : before(ref);
    act(() => {
      h.event('shell.fontsLoading', { waiting: true });
    });
    await waitFor(() => {
      expect((el('timeline.solo') as HTMLButtonElement).disabled).toBe(true);
    });
    fontsRead = true;
    act(() => {
      h.event('shell.fontsLoading', { waiting: false });
    });
    await waitFor(() => {
      expect((el('timeline.solo') as HTMLButtonElement).disabled).toBe(false);
    });
  });
});

// Ruling E-50 item 1 (04 §4.1): rename is one thing in the Timeline. A double click on a row (which the list receives:
// it captures the pointer at each press) and the row context's Rename open the same editor, in the row, with the name
// selected; Enter sends element.rename, Escape sends nothing.
describe('rename in place', () => {
  it('opens from a double click on the list at the row', async () => {
    const h = await timeline({});
    act(() => {
      fireEvent.doubleClick(el('timeline.layers'), { clientX: 60, clientY: 5 });
    });
    const field = (await waitFor(() => el('timeline.row.el_bob.name'))) as HTMLInputElement;
    await waitFor(() => {
      expect(document.activeElement).toBe(field);
    });
    expect(field.value).toBe('Bob');
    expect([field.selectionStart, field.selectionEnd]).toEqual([0, 3]);
    act(() => {
      fireEvent.change(field, { target: { value: 'Robert' } });
      fireEvent.keyDown(field, { key: 'Enter' });
    });
    await waitFor(() => {
      expect(h.actions().find((a) => a.command === 'element.rename')?.params).toEqual({
        elementId: 'el_bob',
        name: 'Robert',
      });
    });
  });

  it('opens from the row context as the same editor, not a dialog', async () => {
    const h = await timeline({});
    act(() => {
      fireEvent.contextMenu(el('timeline.row.el_bob'), { clientX: 30, clientY: 30 });
    });
    const item = await waitFor(() => el('timeline.rowMenu.timeline.row.rename'));
    act(() => {
      item.click();
    });
    const field = (await waitFor(() => el('timeline.row.el_bob.name'))) as HTMLInputElement;
    await waitFor(() => {
      expect(document.activeElement).toBe(field);
    });
    expect(field.value).toBe('Bob');
    expect(document.querySelector('[data-tt-id="timelineRename"]')).toBeNull();
    act(() => {
      fireEvent.keyDown(field, { key: 'Escape' });
    });
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="timeline.row.el_bob.name"]')).toBeNull();
    });
    expect(h.actions().some((a) => a.command === 'element.rename')).toBe(false);
  });
});

// Ruling E-50 item 3: while the engine is not responding, a context menu opened in the Timeline shows each item disabled
// with that reason (a click sends nothing), and the items come back when the engine answers again.
describe('a context menu while the engine is not responding', () => {
  it('shows every item disabled with the reason', async () => {
    const h = await timeline({});
    act(() => {
      h.emit('engine-status', { notResponding: true, generation: 0 });
    });
    // A floating Timeline has no banner of its own; its menus follow the watchdog all the same.
    await new Promise((r) => setTimeout(r, 100));
    act(() => {
      fireEvent.contextMenu(el('timeline.row.el_bob'), { clientX: 30, clientY: 30 });
    });
    const item = await waitFor(() => el('timeline.rowMenu.timeline.row.lock'));
    await waitFor(() => {
      expect(item.getAttribute('aria-disabled')).toBe('true');
    });
    expect(item.textContent).toContain('the engine is not responding');
    // A later owner's item gives the engine's reason too while the engine is stopped (E-52's observation).
    const owned = el('timeline.rowMenu.timeline.row.addDefaultKey');
    expect(owned.textContent).toContain('the engine is not responding');
    expect(owned.textContent).not.toContain('arrives with');
    const sent = h.actions().length;
    act(() => {
      item.click();
    });
    expect(h.actions().length).toBe(sent);
    act(() => {
      h.emit('engine-status', { notResponding: false, generation: 1 });
    });
    await waitFor(() => {
      expect(el('timeline.rowMenu.timeline.row.lock').getAttribute('aria-disabled')).toBe('false');
    });
  });
});

// Plan §6.2's playback budget (C5.1b round 1): an app-state change of the frame alone (each playback frame) re-reads the
// status bar only; the panels, the layout and the workspaces do not depend on it. Any other field reads them again.
describe('the store at a frame change', () => {
  it('reads the status bar only', async () => {
    const h = await timeline({});
    const count = (c: string) => h.sent.filter((x) => x.command === c).length;
    // The start-up reads have completed: every slice read has been sent and no read has been sent for three polls in
    // a row (a wait for a state, D-45; C51B-R2S-03: a fixed 300 ms took the baseline before them on a slow runner).
    let last = -1;
    let quiet = 0;
    await waitFor(
      () => {
        for (const c of [
          'window.panel.list',
          'workspace.get',
          'workspace.list',
          'app.state.get',
          'ui.statusBar.get',
        ])
          expect(count(c)).toBeGreaterThan(0);
        const n = h.sent.length;
        quiet = n === last ? quiet + 1 : 0;
        last = n;
        expect(quiet).toBeGreaterThanOrEqual(3);
      },
      { interval: 50, timeout: 5000 },
    );
    const before = [
      'window.panel.list',
      'workspace.get',
      'workspace.list',
      'app.state.get',
      'ui.statusBar.get',
    ].map(count);
    act(() => {
      h.event('app.state.changed', { changed: ['frame'], state: { ...PROJECT, frame: 9 } });
    });
    // The notice's refresh has run: the store sends a refresh's reads together, so once the status bar's read has gone
    // out, any other slice of the same refresh has too (no fixed wait before judging).
    await waitFor(() => {
      expect(count('ui.statusBar.get')).toBeGreaterThan(before[4] ?? 0);
    });
    expect(['window.panel.list', 'workspace.get', 'workspace.list', 'app.state.get'].map(count)).toEqual(
      before.slice(0, 4),
    );
    act(() => {
      h.event('app.state.changed', { changed: ['element'], state: { ...PROJECT, frame: 9 } });
    });
    await waitFor(() => {
      expect(count('window.panel.list')).toBeGreaterThan(before[0] ?? 0);
    });
  });
});
