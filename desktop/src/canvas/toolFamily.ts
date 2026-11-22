// The tool family chosen last in this window (SI-148: the family chosen last runs in the Camera view; SI-152: a new
// session runs the scene tools). The store notes every engine event from the window's start, so a Camera view canvas
// that mounts after the choice (its panel shown later) starts with the family the person chose, not the scene family.

let scene = true;

/** A new window's store: a new session runs the scene tools. */
export function resetToolFamily(): void {
  scene = true;
}

/** Notes an engine event: choosing a drawing tool or a scene tool sets the family. */
export function noteToolFamily(eventName: string): void {
  if (eventName === 'tool.active.changed') scene = false;
  else if (eventName === 'anim.tool.changed') scene = true;
}

/** Whether the scene family was chosen last. */
export function sceneFamilyChosen(): boolean {
  return scene;
}
