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
 *  owners of their commands (`timeline.currentFrame.move` and playback: M5 C5.1; the scene list: M9). The drawing aids'
 *  group items and the camera list (M4 C4.4a) are built (`toolbarGroups.tsx`, `Toolbars.tsx`). */
export const ITEM_OWNERS: Readonly<Record<string, string>> = {
  playStop: 'M5 C5.1',
  lastFrame: 'M5 C5.1',
  sceneList: 'M9',
};

/** The drawing aids' group items: each member is its own reference (`toolbarGroups.tsx`). */
export const AID_GROUPS: ReadonlySet<string> = new Set([
  'gridType',
  'gridOverlay',
  'onionSkinPrevious',
  'onionSkinNext',
]);

const FRAME_STEPS: Readonly<Record<string, CommandRef>> = {
  firstFrame: { commandId: 'app.frame.set', params: { frame: 1 } },
  previousFrame: { commandId: 'app.frame.move', params: { direction: 'previous' } },
  nextFrame: { commandId: 'app.frame.move', params: { direction: 'next' } },
};

/** The command reference a toolbar item runs (`null`: a separator, a list or a group item without a command). */
export function itemRef(item: string): CommandRef | null {
  if (item in ITEM_OWNERS || AID_GROUPS.has(item)) return null;
  if (item.includes('.')) return { commandId: item, params: {} };
  return FRAME_STEPS[item] ?? null;
}

/** The references of a list of toolbar items. */
export function toolbarRefs(items: readonly string[]): CommandRef[] {
  return items.map(itemRef).filter((r): r is CommandRef => r !== null);
}
