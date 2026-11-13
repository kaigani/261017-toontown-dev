import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { MainToolbar } from '../shell/Toolbars';
import { DEFAULT_LAYOUT, NO_PROJECT_STATE } from '../test-support/scriptedHost';
import type { Layout } from '../state/types';
vi.mock('../shell/actionsApi', () => ({ useActions: () => ({ perform: vi.fn(), run: vi.fn() }) }));
vi.mock('../state/context', () => ({ useSlice: (name: string) => name === 'appState' ? NO_PROJECT_STATE : name === 'history' ? { undo: [], redo: [], canUndo: false, canRedo: false } : [] }));
afterEach(cleanup);
it('disables undo when the kernel has no project and no undo step', () => {
  const layout = { ...DEFAULT_LAYOUT, fullScreen: DEFAULT_LAYOUT } as unknown as Layout;
  render(<MainToolbar layout={layout} fullScreen={false} />);
  expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(true);
});
