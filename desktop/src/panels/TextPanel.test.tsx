// The Text panel as the defaults for new text (02 §7.6.1; M4 C4.4a, reviews/M4-design-panels.md §5): each control
// sends `tool.text.options.set` with its declared reference; fields commit on Enter; an empty font clears it; the
// outline can be removed; the panel follows an agent's change from the event alone.
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

const DEFAULTS = {
  font: 'Inter',
  size: 36,
  bold: false,
  italic: false,
  baseline: 'normal',
  kerning: 0,
  autoKern: true,
  align: 'left',
  leftMargin: 0,
  rightMargin: 0,
  indent: 0,
  lineSpacing: 0,
  outlineColourId: 'cl_1' as string | null,
};

async function panel(options: () => typeof DEFAULTS): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok({ ...PROJECT }),
    'tool.options.get': () => ok({ toolId: 'text', options: options() }),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App floatPanel="text" />
    </HostProvider>,
  );
  await screen.findByText('Defaults for new text');
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const sent = (h: ScriptedHost) =>
  h
    .actions()
    .filter((a) => a.command === 'tool.text.options.set')
    .map((a) => a.params);

describe('the Text panel (02 §7.6.1)', () => {
  it('marks the font as not bundled (D-57: only a bundled font lays out the same on both systems; none is yet)', async () => {
    await panel(() => DEFAULTS);
    await waitFor(() => {
      expect(el('text.fontNotBundled').textContent).toContain('is not bundled');
    });
  });

  it('each control sends its declared reference; fields commit on Enter; an empty font clears it', async () => {
    const h = await panel(() => DEFAULTS);
    act(() => {
      fireEvent.click(el('text.bold'));
    });
    act(() => {
      fireEvent.change(el('text.align'), { target: { value: 'centre' } });
    });
    act(() => {
      fireEvent.change(el('text.size'), { target: { value: '48' } });
      fireEvent.keyDown(el('text.size'), { key: 'Enter' });
    });
    act(() => {
      fireEvent.change(el('text.font'), { target: { value: '' } });
      fireEvent.keyDown(el('text.font'), { key: 'Enter' });
    });
    act(() => {
      fireEvent.click(el('text.removeOutline'));
    });
    await waitFor(() => {
      expect(sent(h)).toEqual([
        { bold: true },
        { align: 'centre' },
        { size: 48 },
        { font: null },
        { outlineColourId: null },
      ]);
    });
    expect(JSON.parse(el('text.align.centre').getAttribute('data-tt-params') ?? '{}')).toEqual({
      align: 'centre',
    });
  });

  it("follows an agent's change from the event alone; Remove outline is disabled without one", async () => {
    let options = DEFAULTS;
    const h = await panel(() => options);
    options = { ...DEFAULTS, bold: true, outlineColourId: null };
    act(() => {
      h.event('tool.options.changed', { toolId: 'text', options });
    });
    await waitFor(() => {
      expect(el('text.bold')).toHaveProperty('checked', true);
    });
    expect(el('text.removeOutline')).toHaveProperty('disabled', true);
  });

  // Ruling D-85 item 2 (finding C44B-R2-02): the panel reads the Text tool's session when it mounts (the scene answer's
  // `textEdit`), then follows the host-only `shell.textEdit` notice. Opened before, during and after a session.
  const SESSION = {
    drawingId: 'dr_1',
    textId: 'tx_1',
    viewId: 'drawingView',
    anchor: 1,
    caretIndex: 1,
    format: { ...DEFAULTS, outlineColourId: undefined },
  };
  async function panelWithScene(textEdit: unknown, role: string): Promise<ScriptedHost> {
    const h = createScriptedHost({
      'app.state.get': () => ok({ ...PROJECT }),
      'tool.options.get': () => ok({ toolId: 'text', options: DEFAULTS }),
    });
    // The host's answer, as `canvas_scene` sends it: `{generation, scene}`.
    h.scenes['drawingView'] = { generation: 0, scene: { textEdit } };
    render(
      <HostProvider api={createHostApi(h)}>
        <App floatPanel="text" />
      </HostProvider>,
    );
    await screen.findByText(role);
    return h;
  }
  const formatted = (h: ScriptedHost) =>
    h
      .actions()
      .filter((a) => a.command === 'draw.textFormat.set')
      .map((a) => a.params);

  it('opened during a session: it shows the session and Bold formats the next typed text', async () => {
    const h = await panelWithScene(SESSION, 'Next typed text');
    act(() => {
      fireEvent.click(el('text.bold'));
    });
    await waitFor(() => {
      expect(formatted(h)).toEqual([{ drawingId: 'dr_1', textId: 'tx_1', start: 1, length: 0, bold: true }]);
    });
    expect(sent(h)).toEqual([]);
  });

  it('opened before a session: it follows the notice that opens it, and the one that ends it', async () => {
    const h = await panelWithScene(null, 'Defaults for new text');
    act(() => {
      h.event('shell.textEdit', {
        drawingId: 'dr_1',
        textId: 'tx_1',
        viewId: 'drawingView',
        anchor: 0,
        caret: 2,
        format: DEFAULTS,
      });
    });
    await screen.findByText('Selected text');
    act(() => {
      h.event('shell.textEdit', null);
    });
    await screen.findByText('Defaults for new text');
  });

  it('opened after a session ended: the defaults, and Bold sets the default', async () => {
    const h = await panelWithScene(null, 'Defaults for new text');
    act(() => {
      fireEvent.click(el('text.bold'));
    });
    await waitFor(() => {
      expect(sent(h)).toEqual([{ bold: true }]);
    });
    expect(formatted(h)).toEqual([]);
  });
});
