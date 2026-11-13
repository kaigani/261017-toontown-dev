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

/** Group items whose behaviour belongs to a later checkpoint (shown disabled with the owner; they run no command). */
export const ITEM_OWNERS: Readonly<Record<string, string>> = {
  gridType: 'M4 C4.2',
  gridOverlay: 'M4 C4.2',
  onionSkinPrevious: 'M4 C4.2',
  onionSkinNext: 'M4 C4.2',
  playStop: 'M5 C5.1',
  cameraList: 'M4 C4.3',
  sceneList: 'M9',
};

const FRAME_STEPS: Readonly<Record<string, CommandRef>> = {
  firstFrame: { commandId: 'app.frame.set', params: { frame: 1 } },
  previousFrame: { commandId: 'app.frame.move', params: { direction: 'previous' } },
  nextFrame: { commandId: 'app.frame.move', params: { direction: 'next' } },
};

/** The command reference a toolbar item runs (`null`: a separator, a list or a group item without a command). */
export function itemRef(item: string): CommandRef | null {
  if (item in ITEM_OWNERS) return null;
  if (item.includes('.')) return { commandId: item, params: {} };
  return FRAME_STEPS[item] ?? null;
}

/** The references of a list of toolbar items. */
export function toolbarRefs(items: readonly string[]): CommandRef[] {
  return items.map(itemRef).filter((r): r is CommandRef => r !== null);
}
