// The main toolbar's items (01 §5.7): display names, the command reference each item runs, and the group items whose
// behaviour belongs to a later checkpoint. Shared by the toolbar and by the shell, which registers the references with
// the store so their availability comes from the kernel (ruling O-10).
import type { CommandRef } from './availability';

/** Display names of the group items and command references of the main toolbar (01 §5.7). */
export const ITEM_LABELS: Readonly<Record<string, string>> = {
  gridType: 'Grid type',
  gridOverlay: 'Overlay',
  onionSkinPrevious: 'Previous drawings',
  onionSkinNext: 'Next drawings',
  firstFrame: 'First frame',
  previousFrame: 'Previous frame',
  playStop: 'Play/Stop',
  nextFrame: 'Next frame',
  lastFrame: 'Last frame',
  cameraList: 'Camera',
  sceneList: 'Scene',
  workspaceList: 'Workspace',
};

/** Group items whose behaviour belongs to a later checkpoint (shown disabled with the owner; they run no command): the
 *  scene list (M9). The drawing aids' group items and the camera list (M4 C4.4a), Play/Stop (M5 C5.1b; `Toolbars.tsx`,
 *  from `playback.state.get`) are built. */
export const ITEM_OWNERS: Readonly<Record<string, string>> = {
  sceneList: 'M9',
};

/** Items drawn by their own component (not one command reference). */
export const OWN_ITEMS: ReadonlySet<string> = new Set(['playStop']);

/** The drawing aids' group items: each member is its own reference (`toolbarGroups.tsx`). */
export const AID_GROUPS: ReadonlySet<string> = new Set([
  'gridType',
  'gridOverlay',
  'onionSkinPrevious',
  'onionSkinNext',
]);

const FRAME_STEPS: Readonly<Record<string, CommandRef>> = {
  // 09 line 127: the first and last frame of the playback range (M5 C5.1b; it sent `app.frame.set {frame: 1}` before).
  firstFrame: { commandId: 'timeline.currentFrame.move', params: { direction: 'first', withinRange: true } },
  lastFrame: { commandId: 'timeline.currentFrame.move', params: { direction: 'last', withinRange: true } },
  previousFrame: { commandId: 'app.frame.move', params: { direction: 'previous' } },
  nextFrame: { commandId: 'app.frame.move', params: { direction: 'next' } },
};

/** The command reference a toolbar item runs (`null`: a separator, a list or a group item without a command). */
export function itemRef(item: string): CommandRef | null {
  if (item in ITEM_OWNERS || AID_GROUPS.has(item) || OWN_ITEMS.has(item)) return null;
  if (item.includes('.')) return { commandId: item, params: {} };
  return FRAME_STEPS[item] ?? null;
}

/** The references of a list of toolbar items. */
export function toolbarRefs(items: readonly string[]): CommandRef[] {
  return items.map(itemRef).filter((r): r is CommandRef => r !== null);
}
