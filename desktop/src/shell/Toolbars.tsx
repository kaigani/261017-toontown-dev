// The main toolbar (01 §5.7) and the tool-palette rail (binding UI convention: palettes vertical on the left; view
// toolbars live in their views; workspace toolbars at the top). Every item is a command reference; group items and
// drop-downs show their members. Whether an item is enabled, checked and how it is labelled comes from the kernel's
// availability (ruling O-10), the source the native menu and the key dispatch use too; until it has answered, an
// item is disabled. Items whose command belongs to a later checkpoint stay visible and disabled, with the reason.
import { COMMAND_INDEX } from '../ipc/generated/commands';
import { TOOL_PALETTES } from '../panels/catalogue';
import { useSlice } from '../state/context';
import type { Arrangement, Layout, WorkspaceEntry } from '../state/types';
import { useActions } from './actionsApi';
import { refKey, type Availability } from './availability';
import { ITEM_LABELS, ITEM_OWNERS, itemRef } from './toolbarItems';

const INDEX = COMMAND_INDEX as unknown as Readonly<
  Record<string, { readonly label?: string; readonly milestone?: string } | undefined>
>;

/** A command reference's display label (the registry's). */
function commandLabel(id: string): string {
  return INDEX[id]?.label ?? id;
}

export function MainToolbar({ layout, fullScreen }: { layout: Layout | undefined; fullScreen: boolean }) {
  const actions = useActions();
  const availability = (useSlice('availability') as Readonly<Record<string, Availability>> | undefined) ?? {};
  const a: Arrangement | undefined = layout ? (fullScreen ? layout.fullScreen : layout) : undefined;
  if (!a?.mainToolbar.visible) return null;
  return (
    <div
      className="main-toolbar"
      role="toolbar"
      aria-label="Main toolbar"
      data-tt-role="toolbar"
      data-tt-id="mainToolbar"
    >
      {a.mainToolbar.items.map((item, i) => {
        if (item === 'separator') return <span key={i} className="toolbar-separator" role="separator" />;
        if (item === 'space') return <span key={i} className="toolbar-space" />;
        if (item === 'flexibleSpace') return <span key={i} className="toolbar-flex" />;
        if (item === 'workspaceList') return <WorkspaceList key={i} />;
        const ref = itemRef(item);
        const facts = ref ? availability[refKey(ref)] : undefined;
        const owner = ITEM_OWNERS[item];
        const staticLabel = ITEM_LABELS[item] ?? commandLabel(item);
        const label = facts?.label ?? staticLabel;
        const enabled = ref !== null && facts?.enabled === true;
        const why = owner ? `arrives with ${owner}` : facts?.reason;
        return (
          <button
            key={i}
            type="button"
            disabled={!enabled}
            aria-label={label}
            {...(facts?.checked !== undefined ? { 'aria-pressed': facts.checked } : {})}
            title={why ? `${label} — ${why}` : label}
            data-tt-role="button"
            data-tt-id={`mainToolbar.${item}`}
            data-tt-label={label}
            {...(ref
              ? { 'data-tt-command': ref.commandId, 'data-tt-params': JSON.stringify(ref.params) }
              : {})}
            onClick={() => {
              if (ref) void actions.perform(ref.commandId, ref.params);
            }}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}

/** The workspace drop-down: lists the workspaces and switches on selection (01 §5.7). */
function WorkspaceList() {
  const actions = useActions();
  const workspaces = (useSlice('workspaces') as readonly WorkspaceEntry[] | undefined) ?? [];
  const current = workspaces.find((w) => w.current);
  return (
    <select
      aria-label="Workspace"
      value={current?.name ?? ''}
      data-tt-role="field"
      data-tt-id="mainToolbar.workspaceList"
      data-tt-command="workspace.select"
      onChange={(e) => {
        void actions.run('workspace.select', { name: e.target.value });
      }}
    >
      {[...workspaces]
        .sort((x, y) => x.position - y.position)
        .map((w) => (
          <option key={w.name} value={w.name}>
            {w.name}
            {w.current && w.modified ? ' *' : ''}
          </option>
        ))}
    </select>
  );
}

/** The tool palettes, docked vertically on the left (binding convention). Their tools arrive with their owners. */
export function ToolPaletteRail({ layout, fullScreen }: { layout: Layout | undefined; fullScreen: boolean }) {
  const a: Arrangement | undefined = layout ? (fullScreen ? layout.fullScreen : layout) : undefined;
  const visible = (a?.toolPalettes ?? []).filter((p) => p.visible && !p.floating);
  if (visible.length === 0) return null;
  return (
    <nav
      className="tool-rail"
      aria-label="Tool palettes"
      data-tt-role="toolPalettes"
      data-tt-id="toolPalettes"
    >
      {visible.map((p) => {
        const info = TOOL_PALETTES.find((t) => t.id === p.id);
        return (
          <div
            key={p.id}
            className="tool-palette"
            data-tt-role="toolPalette"
            data-tt-id={p.id}
            data-tt-label={info?.title ?? p.id}
          >
            <span
              className="tool-palette-title"
              title={info ? `${info.title}: tools arrive with ${info.owner}` : p.id}
            >
              {(info?.title ?? p.id).slice(0, 1)}
            </span>
          </div>
        );
      })}
    </nav>
  );
}
