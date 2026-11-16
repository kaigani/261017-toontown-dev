// The active view window's id in an `app.state.get` result (SI-026 `activeView`: `{viewId, panelId, type}`), or null
// when it names none. Click to activate (views note V9) compares it with the canvas's own view.

export function activeViewOf(state: unknown): string | null {
  if (typeof state !== 'object' || state === null) return null;
  const v = (state as Record<string, unknown>)['activeView'];
  if (typeof v !== 'object' || v === null) return null;
  const id = (v as Record<string, unknown>)['viewId'];
  return typeof id === 'string' ? id : null;
}
