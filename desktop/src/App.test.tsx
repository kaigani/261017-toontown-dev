import { act, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { App } from './App';
import { handleMenuCommand } from './commands';
import { createHostApi, createMockTransport, HostProvider, MENU_COMMAND_EVENT } from './ipc';
import { PANEL_IDS } from './layout/defaultLayout';

function renderApp() {
  const transport = createMockTransport();
  render(
    <HostProvider api={createHostApi(transport)}>
      <App />
    </HostProvider>,
  );
  return transport;
}

describe('App shell', () => {
  it('renders the status bar with engine info from the host', async () => {
    renderApp();
    expect(await screen.findByText(/Toontown 0\.0\.1/)).toBeTruthy();
    expect(screen.getByTestId('ipc-latency').textContent).toContain('IPC ping (mock)');
  });

  it('shows a pending menu command in the status bar', async () => {
    const transport = renderApp();
    await vi.waitFor(() => {
      expect(transport.listenerCount(MENU_COMMAND_EVENT)).toBe(1);
    });
    act(() => {
      transport.emit(MENU_COMMAND_EVENT, {
        command: 'project.create',
        params: {},
        handler: 'pending',
        milestone: 'M1',
      });
    });
    expect(await screen.findByText('project.create is not implemented yet (M1)')).toBeTruthy();
  });
});

describe('menu command handling', () => {
  it('runs workspace.reset through the front-end context', () => {
    const resetWorkspace = vi.fn();
    const msg = handleMenuCommand(
      { resetWorkspace },
      { command: 'workspace.reset', params: { name: 'Default' }, handler: 'frontend', milestone: null },
    );
    expect(resetWorkspace).toHaveBeenCalledOnce();
    expect(msg.kind).toBe('info');
  });

  it('reports unknown front-end commands as errors', () => {
    const msg = handleMenuCommand(
      { resetWorkspace: vi.fn() },
      { command: 'nope.nope', params: {}, handler: 'frontend', milestone: null },
    );
    expect(msg.kind).toBe('error');
  });
});

describe('panels', () => {
  it('use the stable panel ids of 01 §5.2', () => {
    expect(PANEL_IDS).toEqual(['drawingView', 'timeline', 'properties']);
  });
});
