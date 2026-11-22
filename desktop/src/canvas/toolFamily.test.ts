// SI-148 / SI-152: the family chosen last runs in the Camera view, even in a canvas that mounts after the choice (the
// reviewer's l1b step 7 showed its panel after choosing a drawing tool and got the scene menu).
import { describe, expect, it } from 'vitest';

import type { EngineApi } from '../ipc';
import { createStore } from '../state/store';
import { noteToolFamily, sceneFamilyChosen } from './toolFamily';

describe('the tool family chosen last', () => {
  it('a new session runs the scene tools; a drawing tool, then a scene tool, set it', () => {
    createStore({} as EngineApi);
    expect(sceneFamilyChosen()).toBe(true);
    noteToolFamily('tool.active.changed');
    expect(sceneFamilyChosen()).toBe(false);
    noteToolFamily('doc.changed');
    expect(sceneFamilyChosen()).toBe(false);
    noteToolFamily('anim.tool.changed');
    expect(sceneFamilyChosen()).toBe(true);
  });

  it("is noted by the window's store for every event, before any canvas mounts", async () => {
    const store = createStore({ setGeneration: () => undefined } as unknown as EngineApi);
    expect(sceneFamilyChosen()).toBe(true);
    await store
      .apply({ name: 'tool.active.changed', seq: 1, payload: { toolId: 'brush' } })
      .catch(() => undefined);
    // A Camera view canvas mounting now starts with the Drawing family.
    expect(sceneFamilyChosen()).toBe(false);
  });
});
