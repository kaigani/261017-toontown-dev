// The toolbar customisation sheet (01 §5.7 lines 445-464; M4 C4.4a, design note 1 §7): edits are staged; Apply sends
// one `window.toolbar.items.set` with the edited items, Cancel sends nothing, Restore Default sends the reset.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, ok, type ScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

async function sheet(): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'window.toolbar.list': () =>
      ok([
        {
          toolbarId: 'main',
          visible: true,
          items: ['project.save', 'separator', 'edit.undo'],
          allowedItems: ['project.save', 'edit.undo', 'edit.redo', 'separator'],
        },
      ]),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('dialog', { name: 'Start screen' });
  act(() => {
    h.menu('window.toolbar.items.set', {});
  });
  await screen.findByRole('dialog', { name: 'Customize Toolbar' });
  await waitFor(() => {
    expect(document.querySelector('[data-tt-id="toolbarSheet.item.2"]')).not.toBeNull();
  });
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const sent = (h: ScriptedHost, command: string) =>
  h
    .actions()
    .filter((a) => a.command === command)
    .map((a) => a.params);

describe('the toolbar customisation sheet (01 §5.7)', () => {
  it('stages removing, adding and moving, and Apply sends the items once', async () => {
    const h = await sheet();
    act(() => {
      fireEvent.click(el('toolbarSheet.item.1'));
    });
    act(() => {
      fireEvent.click(el('toolbarSheet.remove'));
    });
    act(() => {
      fireEvent.click(el('toolbarSheet.allowed.edit.redo'));
    });
    act(() => {
      fireEvent.click(el('toolbarSheet.add'));
    });
    act(() => {
      fireEvent.click(el('toolbarSheet.up'));
    });
    expect(sent(h, 'window.toolbar.items.set')).toEqual([]);
    act(() => {
      fireEvent.click(el('toolbarSheet.submit'));
    });
    await waitFor(() => {
      expect(sent(h, 'window.toolbar.items.set')).toEqual([
        { toolbarId: 'main', items: ['project.save', 'edit.redo', 'edit.undo'] },
      ]);
    });
  });

  it('Cancel sends nothing; Restore Default sends the reset', async () => {
    const h = await sheet();
    act(() => {
      fireEvent.click(el('toolbarSheet.item.0'));
    });
    act(() => {
      fireEvent.click(el('toolbarSheet.remove'));
    });
    act(() => {
      fireEvent.click(el('toolbarSheet.cancel'));
    });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Customize Toolbar' })).toBeNull();
    });
    expect(sent(h, 'window.toolbar.items.set')).toEqual([]);
    act(() => {
      h.menu('window.toolbar.items.set', {});
    });
    await screen.findByRole('dialog', { name: 'Customize Toolbar' });
    act(() => {
      fireEvent.click(el('toolbarSheet.restore'));
    });
    await waitFor(() => {
      expect(sent(h, 'window.toolbar.items.reset')).toEqual([{}]);
    });
  });
});
