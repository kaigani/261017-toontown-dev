// C43B-R2-03: every option of a list control reports, in the UI tree, its label, its selected state and the reference
// choosing it sends; choosing it sends exactly that reference; the parity collector refuses a commanded list whose
// option declares none; a form's list reports its options with no reference (choosing one sends nothing).
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, DEFAULT_LAYOUT, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';
import { collect } from '../ui-bindings/parity';
import { collectUiTree, type UiNode } from './uiTree';

afterEach(cleanup);

const PROJECT_STATE = {
  ...NO_PROJECT_STATE,
  project: {
    id: 'pr_1',
    name: 'Walk',
    path: '/p/Walk.animpkg',
    dirty: false,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

function find(n: UiNode, id: string): UiNode | undefined {
  if (n.id === id) return n;
  for (const c of n.children ?? []) {
    const f = find(c, id);
    if (f) return f;
  }
  return undefined;
}

describe('list controls declare each option (C43B-R2-03)', () => {
  it('every option of the main toolbar lists reports the reference choosing it sends, and sends it', async () => {
    const items = ['gridType', 'onionSkinPrevious', 'onionSkinNext', 'workspaceList'];
    const layout = { ...DEFAULT_LAYOUT, mainToolbar: { visible: true, items } };
    const h = createScriptedHost({
      'app.state.get': () => ok(PROJECT_STATE),
      'workspace.get': () => ok({ ...layout, fullScreen: layout }),
      'workspace.list': () =>
        ok([
          { name: 'Default', builtIn: true, modified: false, position: 1, current: true },
          { name: 'Inking', builtIn: false, modified: false, position: 2, current: false },
        ]),
      'drawAid.state.get': () =>
        ok({ grid: { visible: true, type: 'field12', overlay: false }, onionSkin: { previous: 2, next: 0 } }),
      'drawAid.grid.set': () => ok({}),
      'drawAid.onionSkin.set': () => ok({}),
      'workspace.select': () => ok({}),
    });
    h.available = () => ({ enabled: true });
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    const grid = await screen.findByRole('combobox', { name: 'Grid type' });
    await waitFor(() => {
      expect(grid).toHaveProperty('disabled', false);
      expect((grid as HTMLSelectElement).value).toBe('field12');
    });
    const tree = collectUiTree(document.body);
    const expected: Record<string, [string, string, Record<string, unknown>, boolean][]> = {
      'mainToolbar.gridType': [
        ['Square grid', 'drawAid.grid.set', { type: 'normal' }, false],
        ['12-field grid', 'drawAid.grid.set', { type: 'field12' }, true],
        ['16-field grid', 'drawAid.grid.set', { type: 'field16' }, false],
      ],
      'mainToolbar.onionSkinPrevious': [0, 1, 2, 3].map((n) => [
        String(n),
        'drawAid.onionSkin.set',
        { previous: n },
        n === 2,
      ]),
      'mainToolbar.onionSkinNext': [0, 1, 2, 3].map((n) => [String(n), 'drawAid.onionSkin.set', { next: n }, n === 0]),
      'mainToolbar.workspaceList': [
        ['Default', 'workspace.select', { name: 'Default' }, true],
        ['Inking', 'workspace.select', { name: 'Inking' }, false],
      ],
    };
    for (const [listId, options] of Object.entries(expected)) {
      const list = find(tree, listId);
      expect(list, listId).toBeDefined();
      const reported = (list?.children ?? []).map((o) => [
        o.role,
        o.label,
        o.command?.commandId,
        o.command?.params,
        o.selected,
      ]);
      expect(reported, listId).toEqual(options.map(([label, cmd, params, sel]) => ['option', label, cmd, params, sel]));
      // Choosing each option sends exactly the reference it reports.
      const select = document.querySelector<HTMLSelectElement>(`[data-tt-id="${listId}"]`);
      expect(select, listId).not.toBeNull();
      for (const node of list?.children ?? []) {
        const before = h.actions().length;
        fireEvent.change(select as HTMLSelectElement, { target: { value: node.value } });
        await waitFor(() => {
          expect(h.actions().slice(before).map((a) => ({ commandId: a.command, params: a.params }))).toEqual([
            node.command,
          ]);
        });
      }
    }
    // The collector counts every option as an action with its own reference.
    const collected = collect(document.body, 'main');
    expect(collected.unbound).toEqual([]);
    expect(collected.actions.find((a) => a.id === 'mainToolbar.gridType.field16')).toEqual({
      id: 'mainToolbar.gridType.field16',
      surface: 'main',
      commands: ['drawAid.grid.set'],
      params: { type: 'field16' },
    });
  });

  it('a commanded list whose option declares no reference is unbound; a form list reports options without one', () => {
    const root = document.createElement('div');
    root.innerHTML = [
      '<select data-tt-role="field" data-tt-id="fixture.list" data-tt-command="drawAid.grid.set">',
      '<option value="normal" data-tt-id="fixture.list.normal" data-tt-command="drawAid.grid.set" data-tt-params=\'{"type":"normal"}\'>Square grid</option>',
      '<option value="field12">12-field grid</option>',
      '</select>',
      '<form><select data-tt-role="field" data-tt-id="fixture.form" data-tt-gui-only="typing">',
      '<option value="a">A</option><option value="b" selected>B</option>',
      '</select></form>',
    ].join('');
    document.body.appendChild(root);
    const c = collect(root, 'main');
    expect(c.unbound).toEqual([
      { surface: 'main', what: 'fixture.list: option "12-field grid" does not declare the reference choosing it sends' },
    ]);
    const tree = collectUiTree(root);
    expect(find(tree, 'fixture.form')?.children).toEqual([
      { role: 'option', id: 'fixture.form.a', label: 'A', value: 'a', selected: false },
      { role: 'option', id: 'fixture.form.b', label: 'B', value: 'b', selected: true },
    ]);
    root.remove();
  });
});
