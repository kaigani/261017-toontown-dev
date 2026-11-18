// The Pen panel (02 §6.1, §6.2; M4 C4.4a, reviews/M4-design-panels.md §3): every control sends its declared reference
// through the one entry point; a slider sends nothing until release; rename commits on Enter, shows `pen.nameExists`
// under the field and sends nothing on Escape; Delete is disabled with the kernel's reason; the panel shows a change an
// agent makes (a kernel event alone) and the Eraser's pens while the Eraser is active.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, fail, ok, type ScriptedHost } from '../test-support/scriptedHost';

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

function pens(names: [string, string] = ['Pen 1', 'Pen 2'], current = 'pen_2') {
  return {
    pens: [
      {
        penId: 'pen_1',
        name: names[0],
        type: 'brush',
        tipShape: 'round',
        minSize: 1,
        maxSize: 4,
        smoothness: 3,
        tipAngle: 0,
      },
      {
        penId: 'pen_2',
        name: names[1],
        type: 'brush',
        tipShape: 'flat',
        minSize: 2,
        maxSize: 8,
        smoothness: 5,
        tipAngle: 30,
      },
      {
        penId: 'pen_3',
        name: 'Eraser 1',
        type: 'eraser',
        tipShape: 'round',
        minSize: 4,
        maxSize: 12,
        smoothness: 0,
        tipAngle: 0,
      },
    ],
    currentBrushPenId: current,
    currentEraserPenId: 'pen_3',
  };
}

async function panel(extra: Record<string, () => unknown> = {}): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok({ ...PROJECT }),
    'tool.pen.list': () => ok(pens()),
    'tool.active.get': () => ok({ toolId: 'brush' }),
    'tool.settings.get': () => ok({ placeBehind: false, symmetry: { mode: 'none' } }),
    ...extra,
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App floatPanel="pen" />
    </HostProvider>,
  );
  await screen.findByText('Pen 2');
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};

const sent = (h: ScriptedHost, command: string) => h.actions().filter((a) => a.command === command);

describe('the Pen panel (02 §6.1)', () => {
  it('lists the brush pens with the current one selected, and each control sends its declared reference', async () => {
    const h = await panel();
    expect(el('pen.row.pen_2').getAttribute('aria-selected')).toBe('true');
    expect(document.querySelector('[data-tt-id="pen.row.pen_3"]')).toBeNull();
    const clicks: [string, string][] = [
      ['pen.row.pen_1', 'tool.pen.select'],
      ['pen.new', 'tool.pen.create'],
      ['pen.delete', 'tool.pen.delete'],
      ['pen.placeBehind', 'tool.placeBehind.set'],
    ];
    for (const [id, command] of clicks) {
      const target = el(id);
      const declared = JSON.parse(target.getAttribute('data-tt-params') ?? '{}') as unknown;
      act(() => {
        fireEvent.click(target);
      });
      await waitFor(() => {
        expect(sent(h, command).at(-1)?.params).toEqual(declared);
      });
    }
    expect(sent(h, 'tool.pen.create').at(-1)?.params).toEqual({ type: 'brush', fromPenId: 'pen_2' });
    expect(sent(h, 'tool.pen.delete').at(-1)?.params).toEqual({ penId: 'pen_2' });
    expect(sent(h, 'tool.placeBehind.set').at(-1)?.params).toEqual({ enabled: true });
    // The tip shape list sends the chosen option's declared reference.
    act(() => {
      fireEvent.change(el('pen.tipShape'), { target: { value: 'square' } });
    });
    await waitFor(() => {
      expect(sent(h, 'tool.pen.set').at(-1)?.params).toEqual({ penId: 'pen_2', tipShape: 'square' });
    });
  });

  it('a slider sends nothing while dragged and one command at release; a field commits on Enter', async () => {
    const h = await panel();
    const slider = el('pen.maxSize');
    act(() => {
      fireEvent.change(slider, { target: { value: '20' } });
      fireEvent.change(slider, { target: { value: '30' } });
    });
    expect(sent(h, 'tool.pen.set')).toEqual([]);
    act(() => {
      fireEvent.pointerUp(slider);
    });
    await waitFor(() => {
      expect(sent(h, 'tool.pen.set').map((a) => a.params)).toEqual([{ penId: 'pen_2', maxSize: 30 }]);
    });
    const field = el('pen.smoothness.field');
    act(() => {
      fireEvent.change(field, { target: { value: '7' } });
      fireEvent.keyDown(field, { key: 'Enter' });
    });
    await waitFor(() => {
      expect(sent(h, 'tool.pen.set').at(-1)?.params).toEqual({ penId: 'pen_2', smoothness: 7 });
    });
  });

  it('rename commits on Enter, shows a clash under the field, and Escape sends nothing', async () => {
    const h = await panel({ 'tool.pen.rename': () => fail('pen.nameExists', 'a pen named Pen 1 exists') });
    act(() => {
      fireEvent.doubleClick(el('pen.row.pen_2'));
    });
    const field = await screen.findByRole('textbox', { name: 'Rename Pen 2' });
    // The name is selected, so typing replaces it; the double click on the current pen sent no selection.
    const input = field as HTMLInputElement;
    expect([input.selectionStart, input.selectionEnd]).toEqual([0, 'Pen 2'.length]);
    expect(sent(h, 'tool.pen.select')).toEqual([]);
    act(() => {
      fireEvent.change(field, { target: { value: 'Pen 1' } });
      fireEvent.keyDown(field, { key: 'Enter' });
    });
    await waitFor(() => {
      expect(el('pen.renameProblem').textContent).toContain('Pen 1 exists');
    });
    expect(sent(h, 'tool.pen.rename').map((a) => a.params)).toEqual([{ penId: 'pen_2', name: 'Pen 1' }]);
    // The panel shows the clash itself: no error box.
    expect(screen.queryByRole('alertdialog')).toBeNull();
    act(() => {
      fireEvent.keyDown(field, { key: 'Escape' });
    });
    expect(screen.queryByRole('textbox', { name: 'Rename Pen 2' })).toBeNull();
    expect(sent(h, 'tool.pen.rename')).toHaveLength(1);
  });

  it('Delete is disabled with the kernel reason for the last pen of a type', async () => {
    const h = createScriptedHost({
      'app.state.get': () => ok({ ...PROJECT }),
      'tool.pen.list': () => ok(pens()),
      'tool.active.get': () => ok({ toolId: 'brush' }),
      'tool.settings.get': () => ok({ placeBehind: false }),
    });
    h.available = (ref) =>
      ref.commandId === 'tool.pen.delete'
        ? { enabled: false, reason: '`Pen 2` is the last brush pen' }
        : { enabled: true };
    render(
      <HostProvider api={createHostApi(h)}>
        <App floatPanel="pen" />
      </HostProvider>,
    );
    await screen.findByText('Pen 2');
    await waitFor(() => {
      expect(el('pen.delete')).toHaveProperty('disabled', true);
    });
    expect(el('pen.delete').getAttribute('title')).toContain('last brush pen');
  });

  it("shows an agent's change from the kernel event alone, and the Eraser's pens while the Eraser is active", async () => {
    let answer = pens();
    let tool = 'brush';
    const h = await panel({
      'tool.pen.list': () => ok(answer),
      'tool.active.get': () => ok({ toolId: tool }),
    });
    answer = pens(['Pen 1', 'Renamed by agent'], 'pen_1');
    act(() => {
      h.event('tool.pen.changed', { penId: 'pen_2', change: 'modified' });
    });
    await screen.findByText('Renamed by agent');
    await waitFor(() => {
      expect(el('pen.row.pen_1').getAttribute('aria-selected')).toBe('true');
    });
    tool = 'eraser';
    act(() => {
      h.event('tool.active.changed', { toolId: 'eraser' });
    });
    await screen.findByText('Eraser 1');
    expect(document.querySelector('[data-tt-id="pen.row.pen_1"]')).toBeNull();
  });
});
