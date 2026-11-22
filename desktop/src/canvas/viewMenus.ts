// The views' context menus (M4 C4.4a, reviews/M4-design-panels.md §9; D-40 answers 11 and 12). Each item sends the
// command its main-menu item sends (02 §4.4 line 296; `menu.json`), with the main menu's availability.
import type { MenuGroup } from '../shell/contextMenuModel';

/**
 * The general edit items (01 §6), each naming the selection context the menu was opened for (D-60 item 2,
 * C44A-R1-02; 01 §6.4 keeps a selection per context): the Drawing family acts on the drawing's selection and
 * the scene family on the staging selection, in either view, whichever panel has the focus. The commands'
 * `context` parameter (spec A: `auto|drawing|staging|...`) is the command layer's own way to say so.
 */
function editItems(context: 'drawing' | 'staging'): MenuGroup {
  return {
    items: [
      { id: 'cut', label: 'Cut', command: 'edit.cut', params: { context } },
      { id: 'copy', label: 'Copy', command: 'edit.copy', params: { context } },
      { id: 'paste', label: 'Paste', command: 'edit.paste', params: { context } },
      { id: 'selectAll', label: 'Select All', command: 'edit.selectAll', params: { context } },
      { id: 'deselectAll', label: 'Deselect All', command: 'edit.deselectAll', params: { context } },
    ],
  };
}

/**
 * The Drawing family (02 §4.4 lines 291-296): the Drawing view's menu, and the Camera view's while a drawing tool is
 * active. Feather Edges has no item until its owner implements `draw.feather.apply` (D-40 answer 11).
 */
export const DRAWING_MENU: readonly MenuGroup[] = [
  editItems('drawing'),
  {
    label: 'Arrange',
    items: [
      {
        id: 'arrange.front',
        label: 'Bring to Front',
        command: 'draw.arrange.apply',
        params: { to: 'front' },
      },
      {
        id: 'arrange.forward',
        label: 'Bring Forward',
        command: 'draw.arrange.apply',
        params: { to: 'forward' },
      },
      {
        id: 'arrange.backward',
        label: 'Send Backward',
        command: 'draw.arrange.apply',
        params: { to: 'backward' },
      },
      { id: 'arrange.back', label: 'Send to Back', command: 'draw.arrange.apply', params: { to: 'back' } },
    ],
  },
  {
    label: 'Transform',
    items: [
      {
        id: 'flip.horizontal',
        label: 'Flip Horizontal',
        command: 'draw.flip.apply',
        params: { axis: 'horizontal' },
      },
      {
        id: 'flip.vertical',
        label: 'Flip Vertical',
        command: 'draw.flip.apply',
        params: { axis: 'vertical' },
      },
      { id: 'rotate.90cw', label: 'Rotate 90° CW', command: 'draw.rotate.apply', params: { by: '90cw' } },
      { id: 'rotate.90ccw', label: 'Rotate 90° CCW', command: 'draw.rotate.apply', params: { by: '90ccw' } },
      { id: 'rotate.180', label: 'Rotate 180°', command: 'draw.rotate.apply', params: { by: '180' } },
    ],
  },
  {
    items: [
      { id: 'group', label: 'Group', command: 'draw.group.create' },
      { id: 'ungroup', label: 'Ungroup', command: 'draw.group.delete' },
    ],
  },
  {
    items: [
      { id: 'flatten', label: 'Flatten', command: 'draw.flatten.apply' },
      { id: 'optimize', label: 'Optimize', command: 'draw.optimize.apply' },
      { id: 'smooth', label: 'Smooth', command: 'draw.smooth.apply' },
      { id: 'centreLine', label: 'Extract Centre Line', command: 'draw.centreLine.apply' },
      { id: 'linesToBrush', label: 'Convert Lines to Brush', command: 'draw.linesToBrush.apply' },
    ],
  },
  { items: [{ id: 'breakApart', label: 'Break Text Apart', command: 'draw.breakApart.apply' }] },
];

/**
 * The scene family (the Camera view with a scene tool; D-40 answer 12): the general edit items only. The items of 05
 * §3.2 lines 326-328 are M6 C6.1's (D-30) and keep their `deferred` owners; the menu grows in M6.
 */
export const SCENE_MENU: readonly MenuGroup[] = [editItems('staging')];
