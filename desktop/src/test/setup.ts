import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

// jsdom has no ResizeObserver; dockview needs one to lay out.
class ResizeObserverStub {
  observe = (): undefined => undefined;
  unobserve = (): undefined => undefined;
  disconnect = (): undefined => undefined;
}
if (!('ResizeObserver' in globalThis)) {
  Object.assign(globalThis, { ResizeObserver: ResizeObserverStub });
}

afterEach(() => {
  cleanup();
});
