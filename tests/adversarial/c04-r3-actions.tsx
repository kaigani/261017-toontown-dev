// Standalone reviewer regressions; run with c04-r3-page.py. No product file is changed.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { createHostApi, createMockTransport, HostProvider } from '../ipc';
import { TabletSpike } from '../spikes/tablet/TabletSpike';

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers(); });

let clock = 1000;

async function capture(script: () => void, auto = false) {
  clock = 1000;
  vi.spyOn(performance, 'now').mockImplementation(() => clock);
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const transport = createMockTransport();
  let saved: Record<string, unknown> | undefined;
  const invoke = transport.invoke.bind(transport);
  transport.invoke = (command, args) => {
    if (command === 'spike_save_log') {
      saved = args?.['log'] as Record<string, unknown>;
      return Promise.resolve({ path: 'review.json', bytes: 1 });
    }
    return invoke(command, args);
  };
  render(<HostProvider api={createHostApi(transport)}><TabletSpike mode={{name:'tablet', logPath:'review.json',
    auto, penSimulation:false, nativeBackend:null}} /></HostProvider>);
  act(script);
  await act(async () => { fireEvent.click(screen.getByText('Save log')); await Promise.resolve(); });
  if (!saved) throw new Error('no saved log');
  return saved;
}

it('control: metadata typing does not create action markers', async () => {
  const log = await capture(() => {
    const input = screen.getByLabelText('Notes');
    input.focus();
    fireEvent.change(input, {target:{value:'k space words'}});
    fireEvent.keyDown(input, {key:'k'});
    fireEvent.keyDown(input, {key:' '});
  });
  expect(log['actions']).toEqual([]);
  expect(log['metadata']).toMatchObject({notes:'k space words'});
});

it('control: auto mode ignores all markers, including keys from pen buttons', async () => {
  vi.useFakeTimers();
  const log = await capture(() => {
    fireEvent.keyDown(window, {key:' '});
    fireEvent.keyDown(window, {key:'k'});
    fireEvent.keyDown(window, {key:' ', repeat:true});
  }, true);
  expect(log['actions']).toEqual([]);
});

it('control: explicit Space on a focused toolbar button marks an action and suppresses activation', async () => {
  const log = await capture(() => {
    const button=screen.getByText('Start native listener');
    button.focus();
    expect(fireEvent.keyDown(button, {key:' ', cancelable:true})).toBe(false);
    fireEvent.keyUp(button, {key:' '});
    clock += 500;
    fireEvent.keyDown(button, {key:' '});
    fireEvent.keyUp(button, {key:' '});
  });
  expect(log['actions']).toMatchObject([{id:'A1',state:'done'}]);
});

it('C04-R3-02: held Space must not finish A1 or advance into A2', async () => {
  const log = await capture(() => {
    fireEvent.keyDown(window, {key:' '});
    fireEvent.keyDown(window, {key:' ', repeat:true});
    fireEvent.keyDown(window, {key:' ', repeat:true});
    fireEvent.keyUp(window, {key:' '});
  });
  expect(log['actions']).toMatchObject([{id:'A1',state:'unfinished'}]);
  expect(log['actions']).toHaveLength(1);
});

it('C04-R3-02: one held K skips only the current optional action', async () => {
  const log = await capture(() => {
    for (let i=0;i<3;i++) {
      fireEvent.keyDown(window, {key:' '});
      fireEvent.keyUp(window, {key:' '});
      clock += 500;
      fireEvent.keyDown(window, {key:' '});
      fireEvent.keyUp(window, {key:' '});
      clock += 500;
    }
    fireEvent.keyDown(window, {key:'k'});
    fireEvent.keyDown(window, {key:'k', repeat:true});
    fireEvent.keyDown(window, {key:'k', repeat:true});
    fireEvent.keyUp(window, {key:'k'});
  });
  expect(log['actions']).toHaveLength(4);
});
