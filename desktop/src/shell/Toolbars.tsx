// The main toolbar (01 §5.7) and the tool-palette rail (binding UI convention: palettes vertical on the left; view
// toolbars live in their views; workspace toolbars at the top). Every item is a command reference; group items and
// drop-downs show their members. Items whose command is not implemented yet stay visible and disabled, with their
// owner in the tooltip.
import { useSlice } from '../state/context';
import type { Arrangement, Layout, WorkspaceEntry } from '../state/types';
import { COMMAND_INDEX } from '../ipc/generated/commands';
import { TOOL_PALETTES } from '../panels/catalogue';
import { useActions } from './actionsApi';

/** Display names of the group items and command references of the main toolbar (01 §5.7). */
const ITEM_LABELS: Readonly<Record<string, string>> = {
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

const INDEX = COMMAND_INDEX as unknown as Readonly<
  Record<string, { readonly label?: string; readonly milestone?: string } | undefined>
>;

/** A command reference's display label (the registry's). */
function commandLabel(id: string): string {
  return INDEX[id]?.label ?? id;
}

/** Items whose behaviour belongs to a later checkpoint (shown disabled with the owner). */
const ITEM_OWNERS: Readonly<Record<string, string>> = {
  gridType: 'M4 C4.2',
  gridOverlay: 'M4 C4.2',
  onionSkinPrevious: 'M4 C4.2',
  onionSkinNext: 'M4 C4.2',
  playStop: 'M5 C5.1',
  cameraList: 'M4 C4.3',
  sceneList: 'M9',
};

export function MainToolbar({ layout, fullScreen }: { layout: Layout | undefined; fullScreen: boolean }) {
  const actions = useActions();
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
        const frameStep: Record<string, Record<string, unknown>> = {
          firstFrame: { frame: 1 },
          previousFrame: { direction: 'previous' },
          nextFrame: { direction: 'next' },
        };
        // A command whose implementation belongs to a later checkpoint stays visible, disabled, with its owner.
        const owner = ITEM_OWNERS[item] ?? INDEX[item]?.milestone;
        const isCommand = item.includes('.');
        const command = isCommand
          ? item
          : item in frameStep
            ? item === 'firstFrame'
              ? 'app.frame.set'
              : 'app.frame.move'
            : null;
        const params = isCommand ? {} : (frameStep[item] ?? {});
        const label = ITEM_LABELS[item] ?? commandLabel(item);
        return (
          <button
            key={i}
            type="button"
            disabled={owner !== undefined || command === null}
            title={owner ? `${label} — arrives with ${owner}` : label}
            data-tt-role="button"
            data-tt-id={`mainToolbar.${item}`}
            data-tt-label={label}
            {...(command ? { 'data-tt-command': command, 'data-tt-params': JSON.stringify(params) } : {})}
            onClick={() => {
              if (command) void actions.perform(command, params);
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
