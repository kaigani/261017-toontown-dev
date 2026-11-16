// C43B-R2-08 and ruling D-44: the shortcut editor's questions are the application's message boxes with specific verbs
// (01 §9.5). Import offers Merge, Replace and Cancel (01 §8.3 item 8); Reset all offers Reset All and Cancel; each
// choice stages exactly its command, applied by OK (01 §8.3 item 9), and Cancel stages nothing.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, type ScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

async function keyboardPage(): Promise<{ h: ScriptedHost; prefs: HTMLElement }> {
  const h = createScriptedHost();
  h.picked = '/p/keys.json';
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('dialog', { name: 'Start screen' });
  act(() => {
    h.event('shell.preferences', { category: 'keyboard' });
  });
  const prefs = await screen.findByRole('dialog', { name: 'Preferences' });
  await waitFor(() => {
    expect(prefs.querySelector('[data-tt-id="keymap.import"]')).not.toBeNull();
  });
  return { h, prefs };
}

const keymapOps = (h: ScriptedHost) =>
  h
    .actions()
    .filter((a) => a.command.startsWith('keymap.') && !a.dryRun)
    .map((a) => ({ command: a.command, params: a.params }));

describe('the shortcut editor asks with the application message box (C43B-R2-08, D-44)', () => {
  for (const [choice, sent] of [
    ['merge', [{ command: 'keymap.import', params: { path: '/p/keys.json', mode: 'merge' } }]],
    ['replace', [{ command: 'keymap.import', params: { path: '/p/keys.json', mode: 'replace' } }]],
    ['cancel', []],
  ] as const) {
    it(`Import › ${choice}`, async () => {
      const { h, prefs } = await keyboardPage();
      fireEvent.click(prefs.querySelector('[data-tt-id="keymap.import"]') as Element);
      const box = await screen.findByRole('alertdialog');
      expect(box.getAttribute('data-tt-id')).toBe('message.keymap.importMode');
      expect(Array.from(box.querySelectorAll('button')).map((b) => b.textContent)).toEqual(
        expect.arrayContaining(['Merge', 'Replace', 'Cancel']),
      );
      fireEvent.click(box.querySelector(`[data-tt-id="message.${choice}"]`) as Element);
      await waitFor(() => {
        expect(screen.queryByRole('alertdialog')).toBeNull();
      });
      if (choice !== 'cancel') {
        await waitFor(() => {
          expect(prefs.querySelector('[data-tt-id="keymap.notice"]')?.textContent).toContain(`(${choice})`);
        });
      }
      fireEvent.click(prefs.querySelector('[data-tt-id="preferences.submit"]') as Element);
      await waitFor(() => {
        expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
      });
      expect(keymapOps(h)).toEqual(sent);
    });
  }

  for (const [choice, sent] of [
    ['reset', [{ command: 'keymap.reset', params: { all: true } }]],
    ['cancel', []],
  ] as const) {
    it(`Reset all › ${choice}`, async () => {
      const { h, prefs } = await keyboardPage();
      fireEvent.click(prefs.querySelector('[data-tt-id="keymap.resetAll"]') as Element);
      const box = await screen.findByRole('alertdialog');
      expect(box.getAttribute('data-tt-id')).toBe('message.keymap.resetAll');
      fireEvent.click(box.querySelector(`[data-tt-id="message.${choice}"]`) as Element);
      await waitFor(() => {
        expect(screen.queryByRole('alertdialog')).toBeNull();
      });
      fireEvent.click(prefs.querySelector('[data-tt-id="preferences.submit"]') as Element);
      await waitFor(() => {
        expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
      });
      expect(keymapOps(h)).toEqual(sent);
    });
  }
});
