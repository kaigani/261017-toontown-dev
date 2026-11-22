// The load report window (10 §7.3; M4 C4.4a, design note 2 §7) and the `clipboard` reason's check (D-40 answer 6,
// design note 2 §8): shown once per open when something happened, never for a clean open; Copy details writes the
// report as text and sends no command; every element declared `clipboard` writes exactly once and runs nothing.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';
import { reportText, type LoadReport } from './loadReport';

const writeText = vi.fn<(text: string) => Promise<void>>();
beforeEach(() => {
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
});
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

const REPORT: LoadReport = {
  projectId: 'pr_1',
  path: '/p/Walk.animpkg',
  formatVersion: '1.0',
  migratedFrom: '0.9',
  readOnly: false,
  repairedCount: 3,
  issues: [
    {
      issueId: 'is_1',
      code: 'ref.dangling',
      severity: 'warning',
      path: 'scene:"Intro"',
      message: 'A reference was cut.',
    },
    {
      issueId: 'is_2',
      code: 'ref.dangling',
      severity: 'warning',
      path: 'scene:"Outro"',
      message: 'Another one.',
    },
    { issueId: 'is_3', code: 'meta.unknownKey', severity: 'info', message: 'An unknown key was kept.' },
  ],
};

async function app(report: () => LoadReport): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok(STATE),
    'doc.loadReport.get': () => ok(report()),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  return h;
}

const opened = (h: ScriptedHost) => {
  act(() => {
    h.event('project.opened', { projectId: 'pr_1' });
  });
};

/** D-40 answer 6: each element declared `clipboard` writes the clipboard exactly once and runs no command. */
function clipboardViolations(h: ScriptedHost): string[] {
  const bad: string[] = [];
  for (const e of document.querySelectorAll<HTMLElement>('[data-tt-gui-only="clipboard"]')) {
    writeText.mockClear();
    const before = h.actions().length;
    act(() => {
      e.click();
    });
    const ran = h.actions().slice(before);
    if (writeText.mock.calls.length !== 1 || ran.length > 0)
      bad.push(
        `${e.dataset['ttId'] ?? '?'}: ${String(writeText.mock.calls.length)} writes, ${String(ran.length)} commands`,
      );
  }
  return bad;
}

describe('the load report window (10 §7.3)', () => {
  it('is shown once per open with the summary and the issues grouped; Close changes nothing', async () => {
    const h = await app(() => REPORT);
    opened(h);
    const win = await screen.findByRole('dialog', { name: 'Load report' });
    expect(win.getAttribute('aria-modal')).toBe('false');
    expect(screen.getByText('Opened with 3 repairs and 2 warnings; migrated from format 0.9')).toBeTruthy();
    const group = screen.getByRole('button', { name: /warning · ref\.dangling \(2\)/ });
    act(() => {
      fireEvent.click(group);
    });
    expect(screen.getByText(/A reference was cut\./)).toBeTruthy();
    const before = h.actions().length;
    act(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    });
    expect(screen.queryByRole('dialog', { name: 'Load report' })).toBeNull();
    expect(h.actions().length).toBe(before);
    // Another event is not another open; the next open shows it again.
    act(() => {
      h.event('prefs.changed', { keys: ['interface.theme'] });
    });
    expect(screen.queryByRole('dialog', { name: 'Load report' })).toBeNull();
    opened(h);
    expect(await screen.findByRole('dialog', { name: 'Load report' })).toBeTruthy();
  });

  it('is not shown for an open with nothing but information', async () => {
    const h = await app(() => ({
      ...REPORT,
      migratedFrom: null,
      repairedCount: 0,
      issues: REPORT.issues.filter((i) => i.severity === 'info'),
    }));
    opened(h);
    await waitFor(() => {
      expect(h.sent.filter((s) => s.command === 'doc.loadReport.get')).toHaveLength(1);
    });
    expect(screen.queryByRole('dialog', { name: 'Load report' })).toBeNull();
  });

  it('Copy details writes the report as text; every clipboard element writes once and runs nothing', async () => {
    const h = await app(() => REPORT);
    opened(h);
    await screen.findByRole('dialog', { name: 'Load report' });
    act(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Copy details' }));
    });
    expect(writeText.mock.calls).toEqual([[reportText(REPORT)]]);
    expect(reportText(REPORT).split('\n')[3]).toBe(
      'warning ref.dangling scene:"Intro": A reference was cut.',
    );
    expect(clipboardViolations(h)).toEqual([]);
    // The check fails on an element that also runs a command.
    const rogue = document.createElement('button');
    rogue.dataset['ttGuiOnly'] = 'clipboard';
    rogue.dataset['ttId'] = 'rogue';
    rogue.addEventListener('click', () => {
      void navigator.clipboard.writeText('x');
      void h.invoke('run_command', { json: JSON.stringify({ command: 'edit.undo', params: {} }) });
    });
    document.body.append(rogue);
    expect(clipboardViolations(h)).toEqual(['rogue: 1 writes, 1 commands']);
    rogue.remove();
  });
});
