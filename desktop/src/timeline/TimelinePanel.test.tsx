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
