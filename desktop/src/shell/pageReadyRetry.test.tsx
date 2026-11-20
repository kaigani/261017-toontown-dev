// D-72 item 1: the host takes `page_ready` only once its own page-load event says the load finished; a report it
// ignored is sent again until accepted, then never again. In its own file: the once-per-document flag is the module's.
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

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
