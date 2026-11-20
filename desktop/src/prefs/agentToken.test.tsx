// Preferences › Agent access shows the agent token (11 §4 lines 107-108; ruling D-62 item 2, C44A-R1-41). The
// invariant: no surface an agent can read ever contains the token. Each surface is checked with the field revealed:
// the window tree the host reports to `inspect_ui`, the capture handshake (`token-hide`), the console, and the
// commands the page sends (none carries it, so the activity log and its export cannot). D-65 item 2: every answer of
// `agent_token` is a numbered delivery the host counts as possibly on screen until the page acknowledges it hidden
// (`token_hidden`); the scripted host numbers them as the desktop host does and can hold an answer in transit, so the
// reviewer's order (C44A-R1-41) is replayed with a late answer.
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
    expect(h.tokenDeliveries).toEqual([]);
  });

  it('Show reveals one delivery, and Hide masks it and acknowledges that delivery hidden', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    expect(h.tokenDeliveries).toEqual([1]);
    expect([...h.tokenOutstanding]).toEqual([1]);
    expect(button(dialog, 'Hide').getAttribute('aria-pressed')).toBe('true');
    act(() => {
      fireEvent.click(button(dialog, 'Hide'));
    });
    expect(field(dialog).value).not.toContain(h.token);
    await waitFor(() => {
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    expect(h.tokenOutstanding.size).toBe(0);
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

  it('a window capture first has the mask painted (token-hide), and nothing is delivered while one runs', async () => {
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
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    expect(h.tokenOutstanding.size).toBe(0);
    // While the host captures, `agent_token` delivers nothing: the field stays masked and says why.
    h.capturing = true;
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await within(dialog).findByText(/cannot be shown now \(the window is being captured\)/);
    expect(field(dialog).value).not.toContain(h.token);
    expect(h.tokenDeliveries).toEqual([1]);
  });

  // D-65 item 2, the reviewer's order (reviews/M4-C4.4a-r2-static.md, C44A-R1-41): Show twice before the answer,
  // the answer painted, Hide, then a capture. The second Show is not a second reveal (one reveal in flight), so after
  // Hide nothing is outstanding and the capture finds no delivery that could paint during it.
  it('the reviewer order: a second Show while one is in flight is no second delivery', async () => {
    const { h, dialog } = await agentPage((s) => {
      s.holdToken = true;
    });
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(h.tokenDeliveries).toEqual([1]);
    });
    // D-67: Show is disabled while the reveal is in flight.
    expect(button(dialog, 'Show').hasAttribute('disabled')).toBe(true);
    await act(async () => {
      expect(h.releaseToken()).toBe(true);
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    expect(button(dialog, 'Hide').hasAttribute('disabled')).toBe(false);
    expect(h.releaseToken()).toBe(false);
    act(() => {
      fireEvent.click(button(dialog, 'Hide'));
    });
    await waitFor(() => {
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    // The capture would be granted now: nothing is outstanding, and nothing is in flight to paint during it.
    expect(h.tokenOutstanding.size).toBe(0);
    expect(field(dialog).value).not.toContain(h.token);
    expect(h.tokenDeliveries).toEqual([1]);
  });

  // The same order with the answer arriving late: each event that ends a reveal (the capture's token-hide, the window
  // losing focus or closing, a regenerated token) ends its generation, and the answer then arriving is acknowledged
  // hidden without ever being painted, so the host's capture guard is granted only after it.
  const enders: [string, (h: ScriptedHost) => void][] = [
    [
      'a capture (token-hide)',
      (h) => {
        h.emit('token-hide', null);
      },
    ],
    ['the window losing focus', () => window.dispatchEvent(new Event('blur'))],
    ['the window closing', () => window.dispatchEvent(new Event('pagehide'))],
    [
      'a regenerated token',
      (h) => {
        h.event('shell.agentToken', {});
      },
    ],
  ];
  for (const [what, end] of enders) {
    it(`a late answer after ${what} is acknowledged hidden and never painted`, async () => {
      const { h, dialog } = await agentPage((s) => {
        s.holdToken = true;
      });
      act(() => {
        fireEvent.click(button(dialog, 'Show'));
      });
      await waitFor(() => {
        expect(h.tokenDeliveries).toEqual([1]);
      });
      // The delivery is outstanding while its answer is in transit: a capture now would wait for it.
      expect([...h.tokenOutstanding]).toEqual([1]);
      act(() => {
        end(h);
      });
      const seen: string[] = [];
      const watch = new MutationObserver(() => {
        seen.push(field(dialog).value);
      });
      watch.observe(dialog, { subtree: true, attributes: true, childList: true, characterData: true });
      await act(async () => {
        expect(h.releaseToken()).toBe(true);
        await Promise.resolve();
      });
      await waitFor(() => {
        expect(h.tokenHiddenAcks).toEqual([1]);
      });
      watch.disconnect();
      expect(h.tokenOutstanding.size).toBe(0);
      expect(field(dialog).value).not.toContain(h.token);
      expect(seen.some((v) => v.includes(h.token))).toBe(false);
      expect(button(dialog, 'Show').getAttribute('aria-pressed')).toBe('false');
    });
  }

  it('a late answer after leaving the page is acknowledged hidden', async () => {
    const { h, dialog } = await agentPage((s) => {
      s.holdToken = true;
    });
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(h.tokenDeliveries).toEqual([1]);
    });
    act(() => {
      fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    });
    await act(async () => {
      expect(h.releaseToken()).toBe(true);
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    expect(h.tokenOutstanding.size).toBe(0);
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
    // Never painted: the delivery is acknowledged hidden once copied.
    await waitFor(() => {
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    expect(h.tokenOutstanding.size).toBe(0);
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

  it('leaving the page masks it and acknowledges the delivery hidden', async () => {
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
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    expect(h.tokenOutstanding.size).toBe(0);
  });
});

// D-69 item 1 (C44A-R1-41, the cleanup schedule): an acknowledgement comes from the window's controller after the
// replacement has been painted, never from the unmounting component; a lost acknowledgement is sent again; the field's
// presence and place are reported for the host's overpaint of captures.
describe('the window-level token controller (D-69 item 1)', () => {
  it('closing Preferences with the token shown acknowledges it only after the page without it has painted', async () => {
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
    // The dialog is gone in this commit; its pixels are still on screen until the next frames are painted.
    expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
    expect(h.tokenHiddenAcks).toEqual([]);
    expect([...h.tokenOutstanding]).toEqual([1]);
    await waitFor(() => {
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
    expect(h.tokenOutstanding.size).toBe(0);
  });

  it('Hide acknowledges after the mask has painted, not in the same commit', async () => {
    const { h, dialog } = await agentPage();
    act(() => {
      fireEvent.click(button(dialog, 'Show'));
    });
    await waitFor(() => {
      expect(field(dialog).value).toBe(h.token);
    });
    act(() => {
      fireEvent.click(button(dialog, 'Hide'));
    });
    expect(field(dialog).value).not.toContain(h.token);
    expect(h.tokenHiddenAcks).toEqual([]);
    await waitFor(() => {
      expect(h.tokenHiddenAcks).toEqual([1]);
    });
  });

  it('an acknowledgement that does not reach the host is sent again until it does', async () => {
    const { h, dialog } = await agentPage((s) => {
      s.tokenHiddenFailures = 2;
    });
    act(() => {
      fireEvent.click(button(dialog, 'Copy'));
    });
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(h.token);
    });
    // Two messages lost: the third attempt reaches the host.
    await waitFor(
      () => {
        expect(h.tokenHiddenAcks).toEqual([1]);
      },
      { timeout: 3000 },
    );
    expect(h.tokenHiddenFailures).toBe(0);
    expect(h.tokenOutstanding.size).toBe(0);
  });

  it("reports the field's presence and answers the host's rectangle requests", async () => {
    const { h, dialog } = await agentPage();
    await waitFor(() => {
      expect(h.tokenFieldReports.at(-1)).toBe(true);
    });
    act(() => {
      h.emit('token-rect', { request: 7 });
    });
    await waitFor(() => {
      expect(h.tokenRectAnswers.find((a) => a.request === 7)?.rect).toMatchObject({
        x: expect.any(Number) as unknown,
        width: expect.any(Number) as unknown,
      });
    });
    act(() => {
      fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    });
    await waitFor(() => {
      expect(h.tokenFieldReports.at(-1)).toBe(false);
    });
    act(() => {
      h.emit('token-rect', { request: 8 });
    });
    await waitFor(() => {
      expect(h.tokenRectAnswers.find((a) => a.request === 8)).toEqual({ request: 8, rect: null });
    });
  });
});
