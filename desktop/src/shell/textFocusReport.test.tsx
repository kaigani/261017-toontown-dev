// The text-focus sensor follows the focused field out of the page (run 45471231219, macOS). WebKit fires no `focusout`
// for a focused element that is removed (neither does jsdom, which follows the specification's focus fixup), so a
// report made only on focus events kept `textFocus: true` after the field left, and the kernel refused an agent's plain
// keys and Shift+F10 with `ui.busy`. The report also follows a change of the page, and sends only a changed fact.
import { cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { EngineApi } from '../ipc';
import type { ShellServices } from '../state/context';
import { useTextFocusReport } from './windowHooks';

afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
});

function Probe({ services }: { services: ShellServices }) {
  useTextFocusReport(services);
  return null;
}

function reporting(): { services: ShellServices; reports: unknown[] } {
  const reports: unknown[] = [];
  const engine = {
    report: (r: unknown) => {
      reports.push(r);
      return Promise.resolve();
    },
  } as unknown as EngineApi;
  return { services: { engine } as unknown as ShellServices, reports };
}

const last = (reports: unknown[]) => reports[reports.length - 1] as { textFocus?: boolean } | undefined;

describe('the text-focus sensor', () => {
  it('reports the focus leaving with a field removed from the page (no focusout)', async () => {
    const { services, reports } = reporting();
    render(<Probe services={services} />);
    const field = document.createElement('input');
    document.body.appendChild(field);
    field.focus();
    await waitFor(() => {
      expect(last(reports)?.textFocus).toBe(true);
    });
    let focusout = false;
    window.addEventListener('focusout', () => {
      focusout = true;
    });
    field.remove();
    await waitFor(() => {
      expect(last(reports)?.textFocus).toBe(false);
    });
    // The case the fix is for: the field left without a focusout event.
    expect(focusout).toBe(false);
  });

  it('sends a fact only when it changes', async () => {
    const { services, reports } = reporting();
    render(<Probe services={services} />);
    const field = document.createElement('input');
    document.body.appendChild(field);
    field.focus();
    await waitFor(() => {
      expect(last(reports)?.textFocus).toBe(true);
    });
    const sent = reports.length;
    for (let i = 0; i < 5; i++) document.body.appendChild(document.createElement('div'));
    field.blur();
    field.focus();
    await waitFor(() => {
      expect(document.activeElement).toBe(field);
    });
    // Unchanged facts (the focus back on the same field, unrelated nodes added) send nothing more than the blur's.
    expect(reports.slice(sent).filter((r) => (r as { textFocus: boolean }).textFocus)).toHaveLength(1);
  });
});
