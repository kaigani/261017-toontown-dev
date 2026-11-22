// C43B-R1S-09: the group 1 controls (ruling D-30) use the shared availability store, and show the kernel's state, so a
// change an agent makes (an event the kernel emits for any origin) shows on each control. One test per control.
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
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

const PROJECT_STATE = {
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

interface World {
  aids: Record<string, unknown>;
  symmetry: string;
  activeTool: string;
  viewType: string;
  zoom: number;
  running: number;
  /** The kernel's availability: every reference enabled unless listed. */
  disabled: Set<string>;
}

function setup(): { h: ScriptedHost; world: World } {
  const world: World = {
    aids: { grid: { visible: false }, onionSkin: { enabled: false }, strokes: false },
    symmetry: 'none',
    activeTool: 'brush',
    viewType: 'camera',
    zoom: 1,
    running: 0,
    disabled: new Set(),
  };
  const h = createScriptedHost({
    'app.state.get': () => ok(PROJECT_STATE),
    'workspace.get': () => ok({ ...DEFAULT_LAYOUT, fullScreen: DEFAULT_LAYOUT }),
    'drawAid.state.get': () => ok(world.aids),
    'drawAid.staticLightTable.list': () => ok({ entries: [] }),
    'tool.settings.get': () => ok({ symmetry: { mode: world.symmetry, angle: 0, mirrors: 6 } }),
    'tool.list': () =>
      ok({ tools: [{ toolId: 'brush' }, { toolId: 'pencil' }], activeToolId: world.activeTool }),
    'view.list': () => ok([{ viewId: 'cameraView', panelId: 'cameraView', type: world.viewType }]),
    'view.transform.get': () => ok({ zoom: world.zoom, rotation: 0 }),
    // `job.list` answers the array of jobs; the status bar asks for every session's (`all`, 11 §15.2).
    'job.list': (p) =>
      ok(p['all'] === true ? Array.from({ length: world.running }, () => ({ state: 'running' })) : []),
  });
  h.available = (ref) => ({ enabled: !world.disabled.has(ref.commandId) });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  return { h, world };
}

const button = (id: string) => document.querySelector<HTMLButtonElement>(`[data-tt-id="${id}"]`);

async function ready() {
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  await waitFor(() => {
    expect(button('cameraView.aid.grid')).not.toBeNull();
    expect(button('drawingTools.brush')).not.toBeNull();
  });
}

describe('group 1 controls: availability (C43B-R1S-09)', () => {
  it('a drawing aid, symmetry, a tool and a view type are disabled while the kernel refuses them', async () => {
    const { h, world } = setup();
    await ready();
    await waitFor(() => {
      expect(button('cameraView.aid.grid')?.disabled).toBe(false);
      expect(button('cameraView.symmetry')?.disabled).toBe(false);
      expect(button('drawingTools.brush')?.disabled).toBe(false);
      expect(button('cameraView.type.drawing')?.disabled).toBe(false);
    });
    // The kernel refuses them now (no project, or a precheck): the controls follow after the next refresh.
    world.disabled = new Set(['drawAid.grid.set', 'tool.symmetry.set', 'tool.active.set', 'view.type.set']);
    act(() => {
      h.event('project.closed', {});
    });
    await waitFor(() => {
      expect(button('cameraView.aid.grid')?.disabled).toBe(true);
      expect(button('cameraView.symmetry')?.disabled).toBe(true);
      expect(button('drawingTools.brush')?.disabled).toBe(true);
      expect(button('cameraView.type.drawing')?.disabled).toBe(true);
    });
  });
});

describe('group 1 controls: a change an agent makes shows (C43B-R1S-09)', () => {
  it('a drawing aid shows the kernel state after drawAid.changed', async () => {
    const { h, world } = setup();
    await ready();
    expect(button('cameraView.aid.grid')?.getAttribute('aria-pressed')).toBe('false');
    world.aids = { ...world.aids, grid: { visible: true } };
    act(() => {
      h.event('drawAid.changed', { aid: 'grid' });
    });
    await waitFor(() => {
      expect(button('cameraView.aid.grid')?.getAttribute('aria-pressed')).toBe('true');
    });
  });

  it('symmetry shows the kernel setting after tool.options.changed, not a local mirror', async () => {
    const { h } = setup();
    await ready();
    expect(button('cameraView.symmetry')?.getAttribute('aria-pressed')).toBe('false');
    act(() => {
      h.event('tool.options.changed', {
        toolId: null,
        options: { symmetry: { mode: 'mirror', angle: 0, mirrors: 6 } },
      });
    });
    await waitFor(() => {
      expect(button('cameraView.symmetry')?.getAttribute('aria-pressed')).toBe('true');
    });
  });

  it('the tools palette shows the active tool after tool.active.changed', async () => {
    const { h, world } = setup();
    await ready();
    await waitFor(() => {
      expect(button('drawingTools.brush')?.getAttribute('aria-pressed')).toBe('true');
    });
    world.activeTool = 'pencil';
    act(() => {
      h.event('tool.active.changed', { toolId: 'pencil', temporary: false });
    });
    await waitFor(() => {
      expect(button('drawingTools.pencil')?.getAttribute('aria-pressed')).toBe('true');
      expect(button('drawingTools.brush')?.getAttribute('aria-pressed')).toBe('false');
    });
  });

  it("a view's type buttons show its type after shell.view", async () => {
    const { h, world } = setup();
    await ready();
    await waitFor(() => {
      expect(button('cameraView.type.camera')?.getAttribute('aria-pressed')).toBe('true');
    });
    world.viewType = 'drawing';
    act(() => {
      h.event('shell.view', { viewId: 'cameraView' });
    });
    await waitFor(() => {
      expect(button('cameraView.type.drawing')?.getAttribute('aria-pressed')).toBe('true');
    });
  });

  it('the zoom readout shows the kernel zoom after shell.view', async () => {
    const { h, world } = setup();
    await ready();
    world.zoom = 2;
    act(() => {
      h.event('shell.view', { viewId: 'cameraView' });
    });
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="cameraView.zoom"]')?.textContent).toBe('200%');
    });
  });

  it('the status bar shows a job an agent started after job.progress', async () => {
    const { h, world } = setup();
    await ready();
    world.running = 1;
    act(() => {
      h.event('job.progress', { jobId: 'j1', fraction: 0.5 });
    });
    await waitFor(() => {
      expect(document.querySelector('[data-tt-id="status.jobs"]')?.textContent).toBe('1 job running');
    });
  });
});
