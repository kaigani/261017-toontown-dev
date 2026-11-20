// D-69 item 1 (C44A-R1-41, the reload schedule): a window's document tells the host once it has painted after its
// bootstrap (`page_ready`), so the host retires the token deliveries of the document it replaced and lets captures of
// the window through again. Once per document.
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

describe('page_ready (D-69 item 1)', () => {
  it('is sent once, after the first frames are painted, and not again on a later render', async () => {
    const h = createScriptedHost();
    const { rerender } = render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    expect(h.pageReadyCalls).toBe(0);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    await waitFor(() => {
      expect(h.pageReadyCalls).toBe(1);
    });
    rerender(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await new Promise((r) => setTimeout(r, 100));
    expect(h.pageReadyCalls).toBe(1);
  });
});

describe('page_ready until the host accepts it (D-72 item 1)', () => {
  it('a report the host ignored (the load had not finished) is sent again until accepted, then never again', async () => {
    const h = createScriptedHost();
    h.pageReadyRefusals = 2;
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    await waitFor(
      () => {
        expect(h.pageReadyCalls).toBe(3);
      },
      { timeout: 3000 },
    );
    await new Promise((r) => setTimeout(r, 500));
    expect(h.pageReadyCalls).toBe(3);
  });
});
