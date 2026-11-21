// The Properties panel's Tool options page (02 §7.7, §8.8-§8.11; M4 C4.4a, reviews/M4-design-panels.md §4): each
// tool's page sends its command with the declared reference; the toggles send the negation of what they show, Draw on
// Top Layer is disabled while Draw Behind is on; the page follows the active tool and an agent's change (events alone).
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, ok, type ScriptedHost } from '../test-support/scriptedHost';

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

const OPTIONS: Record<string, Record<string, unknown>> = {
  select: { rectRule: 'inside' },
  pencil: { thicknessSource: 'penMax', thickness: null },
  polyline: { snap: true, snapRadius: 8, thicknessSource: 'penMax', thickness: null },
};

interface State {
  tool: string;
  settings: Record<string, unknown>;
  gap: string;
}

async function page(state: State): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok({ ...PROJECT }),
    'tool.active.get': () => ok({ toolId: state.tool }),
    'tool.options.get': (p) => ok({ toolId: p['toolId'], options: OPTIONS[String(p['toolId'])] ?? {} }),
    'tool.settings.get': () => ok(state.settings),
    'colour.gapClosing.get': () => ok({ mode: state.gap }),
    'tool.pen.list': () =>
      ok({
        pens: [
          { penId: 'pen_1', name: 'Pen 1', type: 'brush' },
          { penId: 'pen_2', name: 'Pen 2', type: 'brush' },
          { penId: 'pen_3', name: 'Eraser 1', type: 'eraser' },
        ],
        currentBrushPenId: 'pen_2',
        currentEraserPenId: 'pen_3',
      }),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App floatPanel="properties" />
    </HostProvider>,
  );
  await screen.findByText('Draw on Top Layer');
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const last = (h: ScriptedHost, command: string) =>
  h
    .actions()
    .filter((a) => a.command === command)
    .at(-1)?.params;
const settings = { placeBehind: false, drawTopLayer: false, snapToContour: false };

describe('the Tool options page (02 §7.7)', () => {
  it.each([
    ['select', 'toolOptions.rectRule', 'intersect', 'tool.select.options.set', { rectRule: 'intersect' }],
    ['brush', 'toolOptions.pen', 'pen_1', 'tool.brush.options.set', { penId: 'pen_1' }],
    ['eraser', 'toolOptions.pen', 'pen_3', 'tool.eraser.options.set', { penId: 'pen_3' }],
    [
      'pencil',
      'toolOptions.thicknessSource',
      'fixed',
      'tool.pencil.options.set',
      { thicknessSource: 'fixed', thickness: 1 },
    ],
    ['paint', 'toolOptions.gapClosing', 'small', 'colour.gapClosing.set', { mode: 'small' }],
  ])('%s: the list sends the chosen option declared reference', async (tool, id, value, command, params) => {
    const h = await page({ tool, settings, gap: 'off' });
    await waitFor(() => {
      expect(document.querySelector(`[data-tt-id="${id}"]`)).not.toBeNull();
    });
    const option = el(`${id}.${value}`);
    expect(JSON.parse(option.getAttribute('data-tt-params') ?? '{}')).toEqual(params);
    act(() => {
      fireEvent.change(el(id), { target: { value } });
    });
    await waitFor(() => {
      expect(last(h, command)).toEqual(params);
    });
  });

  it('a fixed thickness and the polyline snap radius commit on Enter; snapping toggles', async () => {
    const h = await page({ tool: 'polyline', settings, gap: 'off' });
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="toolOptions.snapRadius"]')).not.toBeNull();
    });
    act(() => {
      fireEvent.change(el('toolOptions.snapRadius'), { target: { value: '12' } });
      fireEvent.keyDown(el('toolOptions.snapRadius'), { key: 'Enter' });
    });
    await waitFor(() => {
      expect(last(h, 'tool.polyline.options.set')).toEqual({ snapRadius: 12 });
    });
    act(() => {
      fireEvent.click(el('toolOptions.snap'));
    });
    await waitFor(() => {
      expect(last(h, 'tool.polyline.options.set')).toEqual({ snap: false });
    });
  });

  it('each toggle sends the negation of its state; Draw on Top Layer is disabled while Draw Behind is on', async () => {
    const h = await page({ tool: 'brush', settings, gap: 'off' });
    for (const [id, command] of [
      ['toolOptions.drawTopLayer', 'tool.drawTopLayer.set'],
      ['toolOptions.placeBehind', 'tool.placeBehind.set'],
      ['toolOptions.snapToContour', 'tool.snapToContour.set'],
    ] as const) {
      act(() => {
        fireEvent.click(el(id));
      });
      await waitFor(() => {
        expect(last(h, command)).toEqual({ enabled: true });
      });
    }
    cleanup();
    await page({ tool: 'brush', settings: { ...settings, placeBehind: true }, gap: 'off' });
    await waitFor(() => {
      expect(el('toolOptions.drawTopLayer')).toHaveProperty('disabled', true);
    });
  });

  it("follows the active tool and an agent's change from the events alone", async () => {
    const state: State = { tool: 'paint', settings, gap: 'off' };
    const h = await page(state);
    await waitFor(() => {
      expect((el('toolOptions.gapClosing') as HTMLSelectElement).value).toBe('off');
    });
    state.gap = 'large';
    act(() => {
      h.event('prefs.changed', { keys: ['paint.gapClosing'], source: 'agent' });
    });
    await waitFor(() => {
      expect((el('toolOptions.gapClosing') as HTMLSelectElement).value).toBe('large');
    });
    state.tool = 'select';
    act(() => {
      h.event('tool.active.changed', { toolId: 'select' });
    });
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="toolOptions.rectRule"]')).not.toBeNull();
    });
    expect(document.querySelector('[data-tt-id="toolOptions.gapClosing"]')).toBeNull();
  });

  it('keeps the Contour Editor line controls, disabled with the reason, when no single line is selected', async () => {
    // Finding C44B-R1-01 (disabled rather than hidden, the design note for panels §1).
    await page({ tool: 'contourEditor', settings, gap: 'off' });
    await waitFor(() => {
      expect(el('toolOptions.selectedLine.thickness')).toHaveProperty('disabled', true);
    });
    expect(el('toolOptions.selectedLine.invisible')).toHaveProperty('disabled', true);
    expect(el('toolOptions.selectedLine.thickness').getAttribute('data-tt-disabled-reason')).toMatch(/Select one/);
    expect(el('toolOptions.selectedLine.reason').textContent).toMatch(/Select one pencil line/);
  });
});
