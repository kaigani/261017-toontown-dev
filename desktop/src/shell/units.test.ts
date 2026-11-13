// M4 C4.1 unit tests (builder-owned): dock layout plans and drop translation, key sequences and native notation, the
// UI-tree walk, theme application, the shared name rule and the new-project field rules.
import { describe, expect, it } from 'vitest';

import { dropToMove, planLayout } from '../dock/layoutPlan';
import { nativeNotation, sequenceOf, type KeyLike } from '../keys/sequence';
import type { Arrangement } from '../state/types';
import { applyTheme, cssColour } from '../theme/theme';
import { nameProblem } from './names';
import { projectFieldProblems } from './projectFields';
import { collectUiTree } from './uiTree';

const DEFAULT: Arrangement = {
  centralPanel: 'cameraView',
  corners: 'sides',
  docks: {
    left: [],
    right: [
      { tabs: ['colour', 'properties', 'library'], current: 'colour', size: 330 },
      { tabs: ['pen', 'text', 'cells'], current: 'pen' },
    ],
    top: [],
    bottom: [{ tabs: ['timeline', 'xsheet'], current: 'timeline', size: 190 }],
  },
  floating: [],
  toolPalettes: [],
  mainToolbar: { visible: true, items: [] },
};

describe('layout plan (01 §5.4, §12.4)', () => {
  it('builds the central view first, inner areas next to it, outer areas at the window edges', () => {
    const plan = planLayout(DEFAULT);
    expect(plan.steps[0]).toEqual({ panelId: 'cameraView' });
    // sides: bottom is inner (next to the central view), right is outer (full height).
    expect(plan.steps[1]).toEqual({
      panelId: 'timeline',
      reference: 'cameraView',
      direction: 'below',
      initialHeight: 190,
    });
    expect(plan.steps.find((s) => s.panelId === 'colour')).toEqual({
      panelId: 'colour',
      direction: 'right',
      initialWidth: 330,
    });
    // The second right group stacks below the first; tabs join their group's first panel.
    expect(plan.steps.find((s) => s.panelId === 'pen')).toEqual({
      panelId: 'pen',
      reference: 'colour',
      direction: 'below',
    });
    expect(plan.steps.find((s) => s.panelId === 'library')).toEqual({
      panelId: 'library',
      reference: 'colour',
      direction: 'within',
    });
    expect(plan.active).toEqual(['colour', 'pen', 'timeline']);
  });

  it('topBottom corners make left and right the inner areas', () => {
    const plan = planLayout({ ...DEFAULT, corners: 'topBottom' });
    expect(plan.steps.find((s) => s.panelId === 'colour')).toEqual({
      panelId: 'colour',
      reference: 'cameraView',
      direction: 'right',
      initialWidth: 330,
    });
    expect(plan.steps.find((s) => s.panelId === 'timeline')).toEqual({
      panelId: 'timeline',
      direction: 'below',
      initialHeight: 190,
    });
  });

  it('a drop becomes the window.panel.move an agent would send', () => {
    expect(dropToMove(DEFAULT, 'history', 'colour', 'center')).toEqual({
      panelId: 'history',
      area: 'right',
      relativeTo: 'colour',
      position: 'tab',
    });
    expect(dropToMove(DEFAULT, 'history', 'timeline', 'right')).toEqual({
      panelId: 'history',
      area: 'bottom',
      relativeTo: 'timeline',
      position: 'after',
    });
    expect(dropToMove(DEFAULT, 'drawingView', 'cameraView', 'center')).toEqual({
      panelId: 'drawingView',
      area: 'central',
    });
    expect(dropToMove(DEFAULT, 'history', 'cameraView', 'left')).toEqual({
      panelId: 'history',
      area: 'left',
      position: 'after',
    });
    expect(dropToMove(DEFAULT, 'history', null, 'bottom')).toEqual({
      panelId: 'history',
      area: 'bottom',
      position: 'after',
    });
    expect(dropToMove(DEFAULT, 'colour', 'colour', 'center')).toBeNull();
  });
});

function key(k: Partial<KeyLike>): KeyLike {
  return { key: '', code: '', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...k };
}

describe('key sequences (01 §12.1 rule 17)', () => {
  it('spells chords as the kernel does', () => {
    expect(sequenceOf(key({ key: 'z', code: 'KeyZ', ctrlKey: true, shiftKey: true }), false)).toBe(
      'Primary+Shift+Z',
    );
    expect(sequenceOf(key({ key: 'z', code: 'KeyZ', metaKey: true }), true)).toBe('Primary+Z');
    expect(sequenceOf(key({ key: 'z', code: 'KeyZ', ctrlKey: true }), true)).toBe('Ctrl+Z');
    expect(sequenceOf(key({ key: '!', code: 'Digit1', shiftKey: true }), false)).toBe('Shift+1');
    expect(sequenceOf(key({ key: ',', code: 'Comma' }), false)).toBe(',');
    expect(sequenceOf(key({ key: 'F11', code: 'F11' }), false)).toBe('F11');
    expect(sequenceOf(key({ key: 'ArrowLeft', code: 'ArrowLeft', altKey: true }), false)).toBe('Alt+Left');
    expect(sequenceOf(key({ key: 'Shift', code: 'ShiftLeft', shiftKey: true }), false)).toBeNull();
  });

  it('shows native notation', () => {
    expect(nativeNotation('Primary+Shift+Z', true)).toBe('⇧⌘Z');
    expect(nativeNotation('Primary+Shift+Z', false)).toBe('Ctrl+Shift+Z');
    expect(nativeNotation('Primary+Ctrl+F', true)).toBe('⌃⌘F');
  });
});

describe('UI tree (11 §7.5)', () => {
  it('reports marked elements with the command each invokes', () => {
    const root = document.createElement('div');
    const html =
      '<div data-tt-role="toolbar" data-tt-id="mainToolbar"><span>' +
      '<button data-tt-role="button" data-tt-id="b1" data-tt-command="edit.undo" data-tt-params="{&quot;steps&quot;:1}">Undo</button>' +
      '<button data-tt-role="button" data-tt-id="b2" disabled>Later</button></span></div>' +
      '<input data-tt-role="field" data-tt-id="f1" type="checkbox" checked>';
    root.innerHTML = html;
    const tree = collectUiTree(root);
    expect(tree.role).toBe('window');
    const toolbar = tree.children?.[0];
    expect(toolbar?.id).toBe('mainToolbar');
    expect(toolbar?.children?.[0]).toEqual({
      role: 'button',
      id: 'b1',
      label: 'Undo',
      command: { commandId: 'edit.undo', params: { steps: 1 } },
    });
    expect(toolbar?.children?.[1]?.enabled).toBe(false);
    expect(tree.children?.[1]).toMatchObject({ role: 'field', id: 'f1', checked: true, value: 'true' });
  });
});

describe('themes (01 §5.8)', () => {
  it('applies built-in and user themes at once', () => {
    expect(applyTheme('dark', [])).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    const user = {
      themeId: 'night',
      name: 'Night',
      builtIn: false,
      dark: true,
      colours: { background: '#10203040' },
    };
    expect(applyTheme('night', [user])).toBe('night');
    expect(document.documentElement.style.getPropertyValue('--tt-background')).toBe(
      'rgba(16, 32, 48, 0.251)',
    );
    expect(applyTheme('light', [user])).toBe('light');
    expect(document.documentElement.style.getPropertyValue('--tt-background')).toBe('');
    expect(applyTheme('unknown', [])).toBe('system');
    expect(cssColour('#FFFFFF')).toBe('rgba(255, 255, 255, 1.000)');
  });
});

describe('field rules (01 §3.1, §9.6)', () => {
  it('follows the shared name rule', () => {
    expect(nameProblem('Walk')).toBeNull();
    expect(nameProblem('')).not.toBeNull();
    expect(nameProblem(' x')).not.toBeNull();
    expect(nameProblem('x.')).not.toBeNull();
    expect(nameProblem('a/b')).not.toBeNull();
    expect(nameProblem('x'.repeat(101))).not.toBeNull();
  });

  it('checks frame rate and camera size ranges, NTSC rates accepted', () => {
    expect(projectFieldProblems({ name: 'a', frameRate: '29.97', width: '16', height: '8192' })).toEqual({});
    expect(
      Object.keys(projectFieldProblems({ name: 'a', frameRate: '0', width: '15', height: '8193' })).sort(),
    ).toEqual(['frameRate', 'height', 'width']);
    expect(
      Object.keys(projectFieldProblems({ name: 'a', frameRate: '24', width: '1920.5', height: '1080' })),
    ).toEqual(['width']);
  });
});
