// V9 (views note; SI-142): the canvas reads the active view from `app.state.get`, whose `activeView` is an object.
import { describe, expect, it } from 'vitest';

import { activeViewOf } from './activeView';

describe('activeViewOf', () => {
  it('reads the view id of app.state.get', () => {
    expect(
      activeViewOf({ activeView: { viewId: 'cameraView', panelId: 'cameraView', type: 'camera' } }),
    ).toBe('cameraView');
  });
  it('is null without an active view, and for a bare string (not the answer shape)', () => {
    expect(activeViewOf({ activeView: null })).toBeNull();
    expect(activeViewOf({})).toBeNull();
    expect(activeViewOf(undefined)).toBeNull();
    expect(activeViewOf({ activeView: 'drawingView' })).toBeNull();
  });
});
