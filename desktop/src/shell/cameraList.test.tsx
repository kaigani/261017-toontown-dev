// The main toolbar's camera list (01 §5.7 lines 453-460; M4 C4.4a, design note 1 §8): the current scene's cameras, the
// current one selected; a choice sends `anim.camera.current.set`; disabled without a camera; follows a change made by
// anyone from the kernel's event.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import {
  createScriptedHost,
  DEFAULT_LAYOUT,
  NO_PROJECT_STATE,
  ok,
  type ScriptedHost,
} from '../test-support/scriptedHost';

afterEach(cleanup);

const STATE = {
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

async function toolbar(
  cameras: () => { id: string; name: string }[],
  current: () => string | null,
): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok(STATE),
    'workspace.get': () =>
      ok({
        ...DEFAULT_LAYOUT,
        mainToolbar: { visible: true, items: ['cameraList'] },
        fullScreen: DEFAULT_LAYOUT,
      }),
    'anim.camera.list': () => ok({ cameras: cameras() }),
    'anim.camera.current.get': () => ok({ cameraId: current() }),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  return h;
}

const list = () => {
  const e = document.querySelector('[data-tt-id="mainToolbar.cameraList"]');
  if (!(e instanceof HTMLSelectElement)) throw new Error('no camera list');
  return e;
};

describe('the camera list (01 §5.7)', () => {
  it('lists the cameras with the current one selected; a choice sends anim.camera.current.set', async () => {
    let current = 'cam_1';
    const h = await toolbar(
      () => [
        { id: 'cam_1', name: 'Camera' },
        { id: 'cam_2', name: 'Wide' },
      ],
      () => current,
    );
    await waitFor(() => {
      expect(list().value).toBe('cam_1');
    });
    act(() => {
      fireEvent.change(list(), { target: { value: 'cam_2' } });
    });
    await waitFor(() => {
      expect(
        h
          .actions()
          .filter((a) => a.command === 'anim.camera.current.set')
          .map((a) => a.params),
      ).toEqual([{ cameraId: 'cam_2' }]);
    });
    // The kernel's event alone shows a change made by anyone.
    current = 'cam_2';
    act(() => {
      h.event('anim.camera.currentChanged', { cameraId: 'cam_2' });
    });
    await waitFor(() => {
      expect(list().value).toBe('cam_2');
    });
  });

  it('is disabled without a camera', async () => {
    await toolbar(
      () => [],
      () => null,
    );
    await waitFor(() => {
      expect(list().disabled).toBe(true);
    });
  });
});
