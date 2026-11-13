// The rendered UI tree (11 §7.5 `inspect_ui`; M4 C4.1). Components mark the elements an agent or a test should see
// with `data-tt-*` attributes; this walk turns them into the tree the host reports to the executor, with the command
// id each widget invokes. Nothing else of the DOM is reported.

export interface UiNode {
  role: string;
  id?: string;
  label?: string;
  value?: string;
  enabled?: boolean;
  checked?: boolean;
  visible?: boolean;
  command?: { commandId: string; params?: unknown };
  children?: UiNode[];
}

function parseParams(text: string | null): unknown {
  if (!text) return undefined;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return undefined;
  }
}

function nodeOf(el: Element): UiNode {
  const n: UiNode = { role: el.getAttribute('data-tt-role') ?? 'group' };
  const id = el.getAttribute('data-tt-id');
  if (id) n.id = id;
  const label = el.getAttribute('data-tt-label') ?? el.getAttribute('aria-label');
  if (label) n.label = label;
  else if (n.role === 'button' || n.role === 'menuitem' || n.role === 'tab' || n.role === 'text') {
    const t = el.textContent.trim();
    if (t) n.label = t.length > 120 ? `${t.slice(0, 117)}...` : t;
  }
  const value = el.getAttribute('data-tt-value');
  if (value !== null) n.value = value;
  else if (
    el instanceof HTMLInputElement ||
    el instanceof HTMLSelectElement ||
    el instanceof HTMLTextAreaElement
  ) {
    n.value = el instanceof HTMLInputElement && el.type === 'checkbox' ? String(el.checked) : el.value;
  }
  const command = el.getAttribute('data-tt-command');
  if (command) {
    const params = parseParams(el.getAttribute('data-tt-params'));
    n.command = params === undefined ? { commandId: command } : { commandId: command, params };
  }
  if (el.hasAttribute('disabled') || el.getAttribute('aria-disabled') === 'true') n.enabled = false;
  const checked =
    el.getAttribute('aria-checked') ?? el.getAttribute('aria-pressed') ?? el.getAttribute('aria-selected');
  if (checked === 'true' || checked === 'false') n.checked = checked === 'true';
  if (el instanceof HTMLInputElement && el.type === 'checkbox') n.checked = el.checked;
  if (el.getAttribute('data-tt-visible') === 'false') n.visible = false;
  return n;
}

/** Walks `root` and returns the marked elements as a tree (unmarked elements are transparent). */
export function collectUiTree(root: Element, rootNode: UiNode = { role: 'window', id: 'main' }): UiNode {
  const walk = (el: Element, into: UiNode) => {
    for (const child of Array.from(el.children)) {
      if (child.hasAttribute('data-tt-role')) {
        const n = nodeOf(child);
        walk(child, n);
        (into.children ??= []).push(n);
      } else {
        walk(child, into);
      }
    }
  };
  const out: UiNode = { ...rootNode };
  walk(root, out);
  return out;
}
