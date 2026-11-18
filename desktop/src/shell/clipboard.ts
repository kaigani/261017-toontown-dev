// The front end's one writer of the system clipboard (M4 C4.4a, design note 2 §8; D-40 answer 6): an element declared
// GUI-only with the reason `clipboard` writes text here and changes no document or application state. The canvas's
// Cut, Copy and Paste are commands (`edit.*`) and never come here.

/** Writes `text` to the system clipboard (from a user gesture: the WebView allows it on both OSes). */
export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}
