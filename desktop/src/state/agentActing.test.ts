// Ruling D-86 item 2 (finding C44B-R3S-02): the agents' running requests are host state; a window that mounts while an
// agent acts reads the host's snapshot after subscribing, and a `shell.agentActing` notice newer than the snapshot (a
// higher number) wins over it.
import { describe, expect, it } from 'vitest';

import { createHostApi } from '../ipc';
import { createScriptedHost } from '../test-support/scriptedHost';
import { createStore } from './store';

const RUNNING = { sessionId: 's1', client: 'Assistant', commandId: 'render.frames', acting: true };

describe("the agents' running requests (D-86 item 2)", () => {
  it('a window that mounts while an agent acts shows it acting from the snapshot, with no notice', async () => {
    const h = createScriptedHost();
    h.acting = { seq: 5, acting: [RUNNING] };
    const store = createStore(createHostApi(h).engine);
    await store.resync();
    expect(store.getState().acting.map((a) => [a.sessionId, a.client, a.commandId])).toEqual([
      ['s1', 'Assistant', 'render.frames'],
    ]);
    expect(store.getState().actingSeq).toBe(5);
  });

  it('a notice newer than the snapshot wins: the request ended after the snapshot was taken', async () => {
    const h = createScriptedHost();
    h.acting = { seq: 5, acting: [RUNNING] };
    const store = createStore(createHostApi(h).engine);
    // The subscription delivered the end of the request (number 7) before the snapshot (number 5) answered.
    await store.apply({ seq: 7, name: 'shell.agentActing', payload: { ...RUNNING, acting: false } });
    await store.resync();
    expect(store.getState().acting).toEqual([]);
    expect(store.getState().actingSeq).toBe(7);
  });

  it('a snapshot newer than the last notice replaces it', async () => {
    const h = createScriptedHost();
    const store = createStore(createHostApi(h).engine);
    await store.apply({ seq: 2, name: 'shell.agentActing', payload: { ...RUNNING, sessionId: 's0' } });
    h.acting = { seq: 9, acting: [RUNNING] };
    await store.resync();
    expect(store.getState().acting.map((a) => a.sessionId)).toEqual(['s1']);
  });
});
