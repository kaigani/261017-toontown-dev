// Ruling D-87 (finding C44B-R3-01): every refusal of a person's action is shown the same way, whatever the input: the
// status line carries the command's label and the refusal's reason, never a message box. The key path (the key
// dispatch's refusal) and the native menu path (a menu item the window performs) here; the gesture path and the panels
// go through the same `refused`.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, fail, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';
import { commandLabel } from './commandLabel';
import { FONTS_READING_LINE, fontsNoticeLine, refusalLine } from './refusals';

afterEach(cleanup);

const PROJECT = {
  ...NO_PROJECT_STATE,
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

const LOCKED =
  'the element is locked: a scene view does not edit it (05 §6.6); unlock it, or address it by id';

const statusLine = () => document.querySelector('[data-tt-id="status.line"]')?.textContent ?? '';

describe('a refused person action is reported on the status line (D-87)', () => {
  it('the text: the label of the refused command and the reason; a disabled command by its reason', () => {
    expect(
      refusalLine(
        { code: 'element.locked', message: LOCKED, details: { refusedCommandId: 'draw.point.delete' } },
        'Delete',
      ),
    ).toBe(`${commandLabel('draw.point.delete')}: ${LOCKED}`);
    expect(
      refusalLine(
        {
          code: 'command.disabled',
          message: '`edit.undo` is not enabled: there is nothing to undo',
          details: { commandId: 'edit.undo', reason: 'there is nothing to undo' },
        },
        'Primary+Z',
      ),
    ).toBe(`${commandLabel('edit.undo')}: there is nothing to undo`);
    expect(refusalLine({ code: 'ui.busy', message: 'busy' }, 'x')).toBeNull();
  });

  it('a key whose edit is refused: the status line says why, no message box', async () => {
    const host = createScriptedHost({ 'app.state.get': () => ok(PROJECT) });
    host.keys['Primary+Z'] = {
      error: {
        code: 'element.locked',
        message: LOCKED,
        details: { elementId: 'el_1', refusedCommandId: 'edit.undo' },
      },
    };
    render(
      <HostProvider api={createHostApi(host)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    fireEvent.keyDown(window, { key: 'z', code: 'KeyZ', ctrlKey: true });
    await waitFor(() => {
      expect(statusLine()).toBe(`${commandLabel('edit.undo')}: ${LOCKED}`);
    });
    expect(host.actions().some((a) => a.command === 'edit.undo')).toBe(false);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('a native menu item whose edit is refused: the status line says why, no message box', async () => {
    const host = createScriptedHost({
      'app.state.get': () => ok(PROJECT),
      'edit.delete': () =>
        fail('element.locked', LOCKED, { elementId: 'el_1', refusedCommandId: 'edit.delete' }),
    });
    render(
      <HostProvider api={createHostApi(host)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('edit.delete', {});
    });
    await waitFor(() => {
      expect(statusLine()).toBe(`${commandLabel('edit.delete')}: ${LOCKED}`);
    });
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('a contained handler panic (E-29): the status line says the command failed unexpectedly, once, no message box', async () => {
    const panicked = {
      code: 'internal',
      message: '`edit.delete` failed unexpectedly; nothing was changed',
      details: { commandId: 'edit.delete', reason: 'handlerPanicked', panic: 'index out of range' },
    };
    expect(refusalLine(panicked, 'Delete')).toBe(
      `${commandLabel('edit.delete')}: failed unexpectedly; nothing was changed`,
    );
    const host = createScriptedHost({
      'app.state.get': () => ok(PROJECT),
      'edit.delete': () => fail(panicked.code, panicked.message, panicked.details),
    });
    render(
      <HostProvider api={createHostApi(host)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('edit.delete', {});
    });
    await waitFor(() => {
      expect(statusLine()).toBe(`${commandLabel('edit.delete')}: failed unexpectedly; nothing was changed`);
    });
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});

describe('the system fonts still being read (ruling E-43)', () => {
  it('a call refused past the bound says so on the status line, naming the command', () => {
    const refused = {
      code: 'command.timeout',
      message: 'the system fonts are still being read',
      details: { reason: 'fontsLoading', boundMs: 20000 },
    };
    expect(refusalLine(refused, 'draw.text.add')).toBe(
      `${commandLabel('draw.text.add')}: the system fonts are still being read; nothing was changed, try again in a moment`,
    );
    // Another timeout reason is not this line (a cancelled call is the person's choice, SI-111).
    expect(
      refusalLine({ code: 'command.timeout', details: { reason: 'cancelled' } }, 'draw.text.add'),
    ).toBeNull();
  });

  it('the notice shows the line while a call waits and nothing once the list is read', () => {
    expect(fontsNoticeLine({ waiting: true })).toBe(FONTS_READING_LINE);
    expect(fontsNoticeLine({ waiting: false })).toBeNull();
    expect(fontsNoticeLine(null)).toBeNull();
  });
});

describe('a Timeline gesture cancelled at its release (rulings E-10, E-28)', () => {
  it('says what happened on the status line, and that nothing was changed', () => {
    expect(
      refusalLine({ code: 'timeline.targetChanged', message: 'changed', details: {} }, 'timeline.gesture'),
    ).toBe('Timeline: what you dragged changed meanwhile; nothing was changed');
    expect(
      refusalLine(
        { code: 'timeline.targetChanged', details: { commandId: 'element.timing.set' } },
        'timeline.gesture',
      ),
    ).toBe(`${commandLabel('element.timing.set')}: what you dragged changed meanwhile; nothing was changed`);
  });
});
