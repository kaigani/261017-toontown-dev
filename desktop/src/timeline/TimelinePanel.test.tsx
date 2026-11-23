// The Timeline panel follows an agent's change of each preference it shows (ruling E-6; 01 §7.3): the background
// (`interface.background.timeline`), the ruler's labels (`interface.timeMeasurement`) and the rows' marker colours
// (`interface.elementColours.*`), each after a `prefs.changed` event alone (no click in the window). The fourth,
// `timing.timeline.singleCellSelection`, is the kernel's to apply in the gesture (crates/executor/tests/
// c51b_single_cell_follows.rs).
import { cleanup, render, waitFor } from '@testing-library/react';
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

async function timeline(prefs: Record<string, string>): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok({ ...PROJECT }),
    'timeline.rows.list': () =>
      ok({
        sceneId: 'sc_1',
        sceneLength: 24,
        currentFrame: 1,
        playbackRange: { mode: 'auto', start: 1, end: 24 },
        rows: [BOB],
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
