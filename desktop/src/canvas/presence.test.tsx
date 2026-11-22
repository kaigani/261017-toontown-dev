// A view's presence (which window shows it, the host's routing and captures): a canvas that unmounts while another
// canvas of the same view is mounted reports no "gone", and the one still mounted reports itself again (a workspace
// switch re-creates the panel; CI run 44401155529 lost the Drawing view's presence that way).
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

const STATE = {
  ...NO_PROJECT_STATE,
  project: {
    id: 'pr_1',
    name: 'P',
    path: '/p/P.animpkg',
    dirty: false,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

function presenceOf(h: ScriptedHost): { viewId: unknown; rect: unknown }[] {
  const calls: { viewId: unknown; rect: unknown }[] = [];
  const invoke = h.invoke.bind(h);
  h.invoke = (command, args) => {
    if (command === 'canvas_presence') {
      const p = JSON.parse(typeof args?.['json'] === 'string' ? args['json'] : '{}') as Record<
        string,
        unknown
      >;
      calls.push({ viewId: p['viewId'], rect: p['rect'] ?? null });
      return Promise.resolve(null);
    }
    return invoke(command, args);
  };
  return calls;
}

describe("a view's presence", () => {
  it('a canvas unmounting while another of its view is mounted sends no "gone"', async () => {
    const first = createScriptedHost({ 'app.state.get': () => ok(STATE) });
    const second = createScriptedHost({ 'app.state.get': () => ok(STATE) });
    const a = presenceOf(first);
    const b = presenceOf(second);
    const one = render(
      <HostProvider api={createHostApi(first)}>
        <App />
      </HostProvider>,
    );
    const two = render(
      <HostProvider api={createHostApi(second)}>
        <App />
      </HostProvider>,
    );
    await screen.findAllByRole('toolbar', { name: 'Main toolbar' });
    expect(document.querySelectorAll('[data-tt-id="cameraView.canvas"]')).toHaveLength(2);
    act(() => {
      one.unmount();
    });
    expect(a.filter((c) => c.viewId === 'cameraView' && c.rect === null)).toEqual([]);
    // The last canvas of the view does report "gone".
    act(() => {
      two.unmount();
    });
    expect(b.filter((c) => c.viewId === 'cameraView' && c.rect === null)).toHaveLength(1);
  });
});
