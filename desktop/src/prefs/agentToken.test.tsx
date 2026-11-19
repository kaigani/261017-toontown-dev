// Preferences › Agent access shows the agent token (11 §4 lines 107-108; ruling D-62 item 2, C44A-R1-41). The
// invariant: no surface an agent can read ever contains the token. Each surface is checked with the field revealed:
// the window tree the host reports to `inspect_ui`, the capture handshake (`token-hide`), the console, and the
// commands the page sends (none carries it, so the activity log and its export cannot).
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, type ScriptedHost } from '../test-support/scriptedHost';

const writeText = vi.fn<(text: string) => Promise<void>>();

beforeEach(() => {
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

async function agentPage(
  setup?: (h: ScriptedHost) => void,
): Promise<{ h: ScriptedHost; dialog: HTMLElement }> {
  const h = createScriptedHost();
  setup?.(h);
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  act(() => {
    h.event('shell.preferences', { category: 'agent' });
  });
  const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
  await within(dialog).findByRole('textbox', { name: 'Agent token' });
  return { h, dialog };
}

const field = (dialog: HTMLElement) =>
  within(dialog).getByRole<HTMLInputElement>('textbox', { name: 'Agent token' });
const button = (dialog: HTMLElement, name: string) => within(dialog).getByRole('button', { name });

describe('the agent token in Preferences › Agent access', () => {
  it('is masked by default, and the page has not even fetched it', async () => {
    const { h, dialog } = await agentPage();
    expect(field(dialog).value).not.toContain(h.token);
    expect(field(dialog).value).toMatch(/^•+$/);
    expect(h.tokenReports).toEqual([]);
  });

  it('Show reveals it after telling the host, and Hide masks it and reports the mask', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    expect(h.tokenReports).toEqual([true]);
    expect(button(dialog, 'Hide').getAttribute('aria-pressed')).toBe('true');
    act(() => {
      fireEvent.click(button(dialog, 'Hide'));
    });
    expect(field(dialog).value).not.toContain(h.token);
    await waitFor(() => {
      expect(h.tokenReports).toEqual([true, false]);
    });
  });

  it('the window tree reports the field redacted while it is revealed', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    // The tree the window reports to the host (what `inspect_ui` answers) after the reveal.
    await waitFor(() => {
      expect(h.rawReports.some((r) => r.includes('agentAccess.token'))).toBe(true);
    });
    for (const r of h.rawReports) expect(r.includes(h.token)).toBe(false);
    expect(h.rawReports.at(-1)).toContain('redacted');
  });

  it('a window capture first has the mask painted (token-hide), and no reveal starts while one runs', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    act(() => {
      h.emit('token-hide', null);
    });
    expect(field(dialog).value).not.toContain(h.token);
    await waitFor(() => {
      expect(h.tokenReports.at(-1)).toBe(false);
    });
    // While the host captures, a reveal is refused: the field stays masked and says why.
    h.tokenAllowed = false;
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await within(dialog).findByText(/cannot be shown while the window is being captured/);
    expect(field(dialog).value).not.toContain(h.token);
  });

  it('Copy puts the token on the clipboard and nowhere else', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Copy'));
    });
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(h.token);
    });
    expect(field(dialog).value).not.toContain(h.token);
    expect(h.tokenReports).toEqual([]);
  });

  it('nothing the page sends or logs carries the token, revealed or copied', async () => {
    const logged: unknown[][] = [];
    for (const m of ['log', 'info', 'warn', 'error', 'debug'] as const)
      vi.spyOn(console, m).mockImplementation((...a: unknown[]) => {
        logged.push(a);
      });
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    act(() => {
      fireEvent.click(button(dialog, 'Copy'));
    });
    await waitFor(() => {
      expect(writeText).toHaveBeenCalled();
    });
    // No command (so no activity entry, no export) and no log line carries it.
    expect(JSON.stringify(h.sent).includes(h.token)).toBe(false);
    expect(JSON.stringify(logged).includes(h.token)).toBe(false);
  });

  it('leaving the page masks it and tells the host', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    act(() => {
      fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    });
    await waitFor(() => {
      expect(h.tokenReports.at(-1)).toBe(false);
    });
  });
});
