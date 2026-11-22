// The main toolbar (01 §5.7) and the tool-palette rail (binding UI convention: palettes vertical on the left; view
// toolbars live in their views; workspace toolbars at the top). Every item is a command reference; group items and
// drop-downs show their members. Whether an item is enabled, checked and how it is labelled comes from the kernel's
// availability (ruling O-10), the source the native menu and the key dispatch use too; until it has answered, an
// item is disabled. Items whose command belongs to a later checkpoint stay visible and disabled, with the reason.
import { useCallback, useEffect, useMemo, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { TOOL_PALETTES } from '../panels/catalogue';
import { useShell, useSlice } from '../state/context';
import type { Arrangement, Layout, WorkspaceEntry } from '../state/types';
import { useActions } from './actionsApi';
import { refKey, type Availability } from './availability';
import { optionRef, runChosen } from './listControl';
import { AidGroupItem } from './toolbarGroups';
import { AID_GROUPS, ITEM_LABELS, ITEM_OWNERS, itemRef } from './toolbarItems';
import { enabledIn, useAvailability } from './useAvailability';
import { useReadGate } from '../state/readGate';

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
        if (item === 'cameraList') return <CameraList key={i} />;
        if (AID_GROUPS.has(item)) return <AidGroupItem key={i} item={item} />;
        const ref = itemRef(item);
        const facts = ref ? availability[refKey(ref)] : undefined;
        const owner = ITEM_OWNERS[item];
        const staticLabel = ITEM_LABELS[item] ?? commandLabel(item);
        const label = facts?.label ?? staticLabel;
        const enabled = ref !== null && facts?.enabled === true;
        // The parameters as the kernel completes them for the current state (O-10): a toggle's reference leaves its
        // state open and the answer gives the next one (C43B-R1-08: sending the open reference changed nothing). The
        // element declares what a click sends (11 §7.5: the command each widget invokes).
        const run =
          facts && facts.params !== null && typeof facts.params === 'object'
            ? (facts.params as Record<string, unknown>)
            : undefined;
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
              ? { 'data-tt-command': ref.commandId, 'data-tt-params': JSON.stringify(run ?? ref.params) }
              : owner
                ? { 'data-tt-gui-only': 'placeholder' }
                : {})}
            onClick={() => {
              if (ref) void actions.perform(ref.commandId, run ?? ref.params);
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
      onChange={runChosen(actions)}
    >
      {[...workspaces]
        .sort((x, y) => x.position - y.position)
        .map((w) => {
          const label = `${w.name}${w.current && w.modified ? ' *' : ''}`;
          return (
            <option
              key={w.name}
              value={w.name}
              {...optionRef('mainToolbar.workspaceList', w.name, label, 'workspace.select', { name: w.name })}
            >
              {label}
            </option>
          );
        })}
    </select>
  );
}

/** The camera list (01 §5.7 lines 453-460; M4 C4.4a, design note 1 §8): the current scene's cameras, the current one
 *  selected; a choice sends `anim.camera.current.set` ("Set Current Camera", undoable, D-40 answer 10). Disabled
 *  without a project or a camera; re-read on the camera, document, undo and project events. */
function CameraList() {
  const { engine } = useShell();
  const actions = useActions();
  const [cameras, setCameras] = useState<{ id: string; name: string }[]>([]);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const gate = useReadGate();
  const refresh = useCallback(() => {
    const live = gate();
    void engine
      .run('anim.camera.list', {})
      .done.then((env) => {
        if (!live()) return;
        const list = (env.result as { cameras?: unknown } | undefined)?.cameras;
        setCameras(env.status === 'ok' && Array.isArray(list) ? (list as typeof cameras) : []);
      })
      .catch(() => undefined);
    void engine
      .run('anim.camera.current.get', {})
      .done.then((env) => {
        if (!live()) return;
        const id = (env.result as { cameraId?: unknown } | undefined)?.cameraId;
        setCurrentId(env.status === 'ok' && typeof id === 'string' ? id : null);
      })
      .catch(() => undefined);
  }, [engine, gate]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    void engine
      .onEvent((e) => {
        if (
          e.name === 'anim.camera.currentChanged' ||
          e.name === 'doc.changed' ||
          e.name === 'doc.undone' ||
          e.name === 'doc.redone' ||
          e.name === 'app.state.changed' ||
          e.name === 'project.opened' ||
          e.name === 'project.closed'
        )
          refresh();
      })
      .then((u) => {
        if (stopped) u();
        else off = u;
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, refresh]);
  return (
    <select
      aria-label="Camera"
      value={currentId ?? ''}
      disabled={cameras.length === 0}
      title={cameras.length === 0 ? 'No camera in the current scene' : 'Current camera'}
      data-tt-role="field"
      data-tt-id="mainToolbar.cameraList"
      data-tt-command="anim.camera.current.set"
      onChange={runChosen(actions)}
    >
      {cameras.map((c) => (
        <option
          key={c.id}
          value={c.id}
          {...optionRef('mainToolbar.cameraList', c.id, c.name, 'anim.camera.current.set', {
            cameraId: c.id,
          })}
        >
          {c.name}
        </option>
      ))}
    </select>
  );
}

/** The Drawing tools palette's tools (M4 C4.3b; ruling D-30, group 1; 02 §1.3): `tool.list`'s tools, each a button
 *  that makes it the active tool (`tool.active.set`), the active one pressed. Each button is enabled only when the
 *  kernel's availability of its reference says so (C43B-R1S-09); the list and the active tool are re-read on the
 *  kernel's events, a project closed included (its tools go away with it). */
function DrawingToolButtons() {
  const { engine } = useShell();
  const actions = useActions();
  const [tools, setTools] = useState<{ toolId: string; group?: string }[]>([]);
  const [active, setActive] = useState<string | null>(null);
  useEffect(() => {
    let stopped = false;
    let off: (() => void) | null = null;
    const read = () => {
      void engine
        .run('tool.list', {})
        .done.then((env) => {
          const r = (env.status === 'ok' ? (env.result ?? {}) : {}) as {
            tools?: { toolId: string; group?: string }[];
            activeToolId?: string;
          };
          if (stopped) return;
          setTools(r.tools ?? []);
          setActive(r.activeToolId ?? null);
        })
        .catch(() => undefined);
    };
    read();
    void engine
      .onEvent((e) => {
        if (e.name === 'tool.active.changed' || e.name === 'project.opened' || e.name === 'project.closed')
          read();
      })
      .then((u) => {
        if (stopped) u();
        else off = u;
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine]);
  const refs = useMemo(
    () => tools.map((t) => ({ commandId: 'tool.active.set', params: { toolId: t.toolId } })),
    [tools],
  );
  const availability = useAvailability(refs);
  return (
    <>
      {tools.map((t) => (
        <button
          key={t.toolId}
          type="button"
          className="tool-button"
          disabled={!enabledIn(availability({ commandId: 'tool.active.set', params: { toolId: t.toolId } }))}
          aria-pressed={active === t.toolId}
          title={t.toolId}
          data-tt-role="button"
          data-tt-id={`drawingTools.${t.toolId}`}
          data-tt-command="tool.active.set"
          data-tt-params={JSON.stringify({ toolId: t.toolId })}
          onClick={() => {
            void actions.run('tool.active.set', { toolId: t.toolId });
          }}
        >
          {t.toolId}
        </button>
      ))}
    </>
  );
}

/** The tool palettes, docked vertically on the left (binding convention). The Drawing tools palette holds the drawing
 *  tools (M4 C4.3b); the other palettes' tools arrive with their owners. */
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
            {p.id === 'drawingTools' && <DrawingToolButtons />}
          </div>
        );
      })}
    </nav>
  );
}
