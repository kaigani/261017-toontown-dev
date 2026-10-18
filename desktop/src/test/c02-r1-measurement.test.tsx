import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { createHostApi, createMockTransport, HostProvider, type PenSimReport } from '../ipc';
import { summarizeNative, summarizeWeb, type WebSample } from '../spikes/tablet/analysis';
import { WebCapture } from '../spikes/tablet/capture';
import { TabletSpike } from '../spikes/tablet/TabletSpike';

const point = (t: number, kind: WebSample['kind'] = 'move'): WebSample => ({
  t,
  handlerT: t + 1,
  kind,
  pointerType: 'pen',
  x: t,
  y: 1,
  pressure: kind === 'up' ? 0 : 0.5,
  tiltX: 0,
  tiltY: 0,
  twist: 0,
  dispatch: t,
  coalescedCount: 1,
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

it('C02-R1-03: lifting between two perfect 250 Hz strokes is not a drop or contact time', () => {
  const samples = [
    point(0, 'down'),
    ...[4, 8, 12, 16].map((t) => point(t)),
    point(20, 'up'),
    point(1000, 'down'),
    ...[1004, 1008, 1012, 1016].map((t) => point(t)),
    point(1020, 'up'),
  ];
  const result = summarizeWeb(samples, []);
  expect(result.gaps).toBe(0);
  expect(result.rateHz).toBe(250);
});

it('C02-R1-03: native contact summaries exclude pen-up time too', () => {
  const samples = [
    point(0, 'down'),
    ...[4, 8, 12, 16].map((t) => point(t)),
    point(20, 'up'),
    point(1000, 'down'),
    ...[1004, 1008, 1012, 1016].map((t) => point(t)),
    point(1020, 'up'),
  ];
  const result = summarizeNative(
    samples.map((s) => ({
      kind: s.kind === 'move' ? 'pose' : s.kind,
      hostEpochMs: 1_000_000 + s.t,
      deviceMs: s.t,
      x: s.x,
      y: s.y,
      pressure: s.pressure,
      tiltX: 0,
      tiltY: 0,
      twist: 0,
      tool: 'pen',
    })),
  );
  expect(result.gaps).toBe(0);
  expect(result.rateHz).toBe(250);
});

it('C02-R1-06: API presence is not inferred from having observed a batch larger than one', () => {
  class EventWithCoalescing extends Event {
    getCoalescedEvents() {
      return [];
    }
  }
  vi.stubGlobal('PointerEvent', EventWithCoalescing);
  const capture = new WebCapture(document.createElement('canvas'));
  try {
    expect(capture.coalescedApi).toBe(true);
    // A low-rate pen is still on a browser supporting coalescing.
    const result = summarizeWeb([point(0), point(100), point(200)], []);
    expect(result.coalescedSupported).toBe(true);
  } finally {
    capture.stop();
  }
});

async function completedSimulation() {
  vi.useFakeTimers();
  class TestPointer extends Event {}
  vi.stubGlobal('PointerEvent', TestPointer);
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const transport = createMockTransport();
  const report: PenSimReport = {
    method: 'InjectSyntheticPointerInput',
    frames: 1024,
    hz: 240,
    achievedHz: 240,
    injectEpochMs: Array.from({ length: 1024 }, (_, i) => 1_000_000 + (i * 1000) / 240),
    pressure: Array.from({ length: 1024 }, (_, i) => i + 1),
    tiltX: Array(1024).fill(0) as number[],
    tiltY: Array(1024).fill(0) as number[],
    rotation: Array(1024).fill(0) as number[],
  };
  let saved: Record<string, unknown> | undefined;
  const invoke = transport.invoke.bind(transport);
  transport.invoke = (command, args) => {
    if (command === 'spike_pen_simulate') {
      const canvas = screen.getByTestId('spike-canvas');
      Object.assign(canvas, { setPointerCapture: () => undefined });
      // Deliver every contact frame, including the initial pointerdown, with no loss.
      for (let i = 0; i < 1024; i++) {
        const event = new TestPointer(i === 0 ? 'pointerdown' : 'pointermove', { cancelable: true });
        Object.assign(event, {
          pointerId: 1,
          pointerType: 'pen',
          pressure: (i + 1) / 1024,
          tiltX: 0,
          tiltY: 0,
          twist: 0,
          clientX: i,
          clientY: 1,
        });
        canvas.dispatchEvent(event);
      }
      return Promise.resolve(report);
    }
    if (command === 'spike_save_log') {
      saved = args?.['log'] as Record<string, unknown>;
      return Promise.resolve({ path: 'test.json', bytes: 1 });
    }
    return invoke(command, args);
  };
  render(
    <HostProvider api={createHostApi(transport)}>
      <TabletSpike
        mode={{
          name: 'tablet',
          logPath: 'test.json',
          auto: false,
          penSimulation: true,
          nativeBackend: 'octotablet-ink',
        }}
      />
    </HostProvider>,
  );
  await act(async () => {
    fireEvent.click(screen.getByText('Simulate pen 240 Hz (synthetic)'));
    await Promise.resolve();
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  await act(async () => {
    fireEvent.click(screen.getByText('Save log'));
    await Promise.resolve();
  });
  expect(saved).toBeDefined();
  if (!saved) throw new Error('Save log did not call the host');
  return { saved, report };
}

it('C02-R1-01: exported evidence retains injection timing and frame values for independent analysis', async () => {
  const { saved, report } = await completedSimulation();
  const simulations = saved['simulations'] as { report?: PenSimReport }[];
  expect(simulations[0]?.report).toBeDefined();
  expect(simulations[0]?.report).toEqual(report);
  expect(saved['environment']).toHaveProperty('timeOrigin');
});

it('C02-R1-05: a lossless 1024-contact-frame run reports zero missing frames', async () => {
  const { saved } = await completedSimulation();
  const simulations = saved['simulations'] as { web: { received: number; missing: number } }[];
  expect(simulations[0]?.web.received).toBe(1024);
  expect(simulations[0]?.web.missing).toBe(0);
});
