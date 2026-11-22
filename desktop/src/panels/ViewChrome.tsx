// A Drawing or Camera view window's chrome (M4 C4.3b; ruling D-30, group 1; 01 §5.3, 02 §10): the zoom and rotation
// readout (`view.transform.get`), the window's current type (`view.list`), the drawing aids (`drawAid.*`, their state
// from `drawAid.state.get`) and symmetry (`tool.symmetry.set`, its state from `tool.settings.get`). Every control runs
// its command through the one entry point (`actions.run`); a toggle sends the negation of the state it shows, so its
// parameters are not fixed and the parity manifest binds the command alone. Each control is enabled only when the
// kernel's availability of its reference says so (ruling O-10, C43B-R1S-09: disabled without a project and when its
// precheck refuses), and shows the kernel's state, re-read on the kernel's change events, so a change an agent makes
// shows too.

import { useCallback, useEffect, useMemo, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import type { CommandRef } from '../shell/availability';
import { enabledIn, useAvailability } from '../shell/useAvailability';
import { useShell } from '../state/context';
import { useReadGate } from '../state/readGate';

type Json = Record<string, unknown>;

const obj = (v: unknown): Json => (typeof v === 'object' && v !== null ? (v as Json) : {});
const flag = (v: unknown): boolean => v === true;

interface Aid {
  id: string;
  label: string;
  command: string;
  /** Whether the toggle shows as on, from `drawAid.state.get`'s answer. */
  on?: (s: Json) => boolean;
  /** The parameters of a click, from the state shown. */
  params: (s: Json) => Json;
}

/** The drawing aids of 02 §10 a view window offers, in order. */
const AIDS: Aid[] = [
  {
    id: 'grid',
    label: 'Grid',
    command: 'drawAid.grid.set',
    on: (s) => flag(obj(s['grid'])['visible']),
    params: (s) => ({ visible: !flag(obj(s['grid'])['visible']) }),
  },
  {
    id: 'onionSkin',
    label: 'Onion skin',
    command: 'drawAid.onionSkin.set',
    on: (s) => flag(obj(s['onionSkin'])['enabled']),
    params: (s) => ({ enabled: !flag(obj(s['onionSkin'])['enabled']) }),
  },
  {
    id: 'onionPrevious',
    label: 'Previous drawings',
    command: 'drawAid.onionSkinCycle.run',
    params: () => ({ direction: 'previous' }),
  },
  {
    id: 'onionNext',
    label: 'Next drawings',
    command: 'drawAid.onionSkinCycle.run',
    params: () => ({ direction: 'next' }),
  },
  {
    id: 'onionOutline',
    label: 'Onion skin outlines',
    command: 'drawAid.onionSkinStyle.set',
    on: (s) => flag(obj(s['onionSkinStyle'])['outline']),
    params: (s) => ({ outline: !flag(obj(s['onionSkinStyle'])['outline']) }),
  },
  {
    id: 'autoLightTable',
    label: 'Auto light table',
    command: 'drawAid.autoLightTable.set',
    // `drawAid.state.get` answers these three as booleans (C4.3b round 1, D-39: read as objects, the buttons never
    // showed as on and always sent the same value).
    on: (s) => flag(s['autoLightTable']),
    params: (s) => ({ enabled: !flag(s['autoLightTable']) }),
  },
  {
    id: 'lightTableShade',
    label: 'Shade light tables',
    command: 'drawAid.lightTableShade.set',
    on: (s) => flag(obj(s['lightTableShade'])['auto']),
    params: (s) => {
      const v = !flag(obj(s['lightTableShade'])['auto']);
      return { auto: v, static: v };
    },
  },
  {
    id: 'staticLightTablePanel',
    label: 'Static light table panel',
    command: 'drawAid.staticLightTablePanel.set',
    // The state names the entries, not the panel: the button shows the panel.
    params: () => ({ visible: true }),
  },
  {
    id: 'staticLightTableAdd',
    label: 'Send to static light table',
    command: 'drawAid.staticLightTable.add',
    params: () => ({}),
  },
  {
    id: 'staticLightTableShowAll',
    label: 'Show all static light table entries',
    command: 'drawAid.staticLightTable.set',
    params: () => ({ all: true, visible: true }),
  },
  {
    id: 'staticLightTableClear',
    label: 'Clear the static light table',
    command: 'drawAid.staticLightTable.remove',
    params: () => ({ all: true }),
  },
  {
    id: 'strokes',
    label: 'Show strokes',
    command: 'drawAid.strokes.set',
    on: (s) => flag(s['strokes']),
    params: (s) => ({ visible: !flag(s['strokes']) }),
  },
  {
    id: 'currentOnTop',
    label: 'Current drawing on top',
    command: 'drawAid.currentOnTop.set',
    on: (s) => flag(s['currentOnTop']),
    params: (s) => ({ enabled: !flag(s['currentOnTop']) }),
  },
  {
    id: 'boxHighlighting',
    label: 'Box highlighting',
    command: 'drawAid.selectionDisplay.set',
    on: (s) => flag(obj(s['selectionDisplay'])['boxHighlighting']),
    params: (s) => ({ boxHighlighting: !flag(obj(s['selectionDisplay'])['boxHighlighting']) }),
  },
  {
    id: 'outlineOnSelection',
    label: 'Outline on selection',
    command: 'drawAid.selectionDisplay.set',
    on: (s) => flag(obj(s['selectionDisplay'])['outlineOnSelection']),
    params: (s) => ({ outlineOnSelection: !flag(obj(s['selectionDisplay'])['outlineOnSelection']) }),
  },
  {
    id: 'keepSelectionVisible',
    label: 'Selection always visible',
    command: 'drawAid.selectionDisplay.set',
    on: (s) => flag(obj(s['selectionDisplay'])['keepSelectionVisible']),
    params: (s) => ({ keepSelectionVisible: !flag(obj(s['selectionDisplay'])['keepSelectionVisible']) }),
  },
  {
    id: 'sceneBackground',
    label: 'Use the scene background colour',
    command: 'drawAid.background.set',
    on: (s) => flag(obj(s['background'])['useSceneColour']),
    params: (s) => ({ useSceneColour: !flag(obj(s['background'])['useSceneColour']) }),
  },
];

/** The symmetry mode in a settings map (`tool.settings.get`, `tool.options.changed` with no tool). */
const modeOf = (settings: Json): string | null => {
  const m = obj(settings['symmetry'])['mode'];
  return typeof m === 'string' ? m : null;
};

export function ViewChrome({ viewId }: { viewId: string }) {
  const { engine } = useShell();
  const actions = useActions();
  const [aids, setAids] = useState<Json>({});
  const [zoom, setZoom] = useState<{ zoom: number; rotation: number } | null>(null);
  const [symmetry, setSymmetry] = useState<string>('none');
  const [lightTableEntries, setLightTableEntries] = useState(0);
  const gate = useReadGate();
  const refresh = useCallback(() => {
    const live = gate();
    void engine
      .run('tool.settings.get', {})
      .done.then((env) => {
        if (!live()) return;
        const m = env.status === 'ok' ? modeOf(obj(env.result)) : null;
        if (m) setSymmetry(m);
      })
      .catch(() => undefined);
    void engine
      .run('drawAid.state.get', {})
      .done.then((env) => {
        if (!live()) return;
        if (env.status === 'ok') setAids(obj(env.result));
      })
      .catch(() => undefined);
    void engine
      .run('drawAid.staticLightTable.list', {})
      .done.then((env) => {
        if (!live()) return;
        const entries = obj(env.result)['entries'];
        if (Array.isArray(entries)) setLightTableEntries(entries.length);
      })
      .catch(() => undefined);
    void engine
      .run('view.transform.get', { viewId })
      .done.then((env) => {
        if (!live()) return;
        const r = obj(env.result);
        if (typeof r['zoom'] === 'number')
          setZoom({ zoom: r['zoom'], rotation: typeof r['rotation'] === 'number' ? r['rotation'] : 0 });
      })
      .catch(() => undefined);
  }, [engine, viewId, gate]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    void engine
      .onEvent((e) => {
        if (
          e.name === 'drawAid.changed' ||
          e.name === 'shell.view' ||
          e.name === 'project.opened' ||
          e.name === 'project.closed'
        )
          refresh();
        // The global drawing-tool settings changed (`tool.options.changed` with no tool): symmetry among them.
        const p = obj(e.payload);
        const m =
          e.name === 'tool.options.changed' && p['toolId'] === null ? modeOf(obj(p['options'])) : null;
        if (m) setSymmetry(m);
      })
      .then((u) => {
        if (stopped) {
          u();
          return;
        }
        off = u;
        // Read again once subscribed (D-86 item 2): nothing between the first read and the subscription is lost.
        refresh();
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, refresh]);
  const symmetryRef = useMemo(
    (): CommandRef => ({
      commandId: 'tool.symmetry.set',
      params: { mode: symmetry === 'none' ? 'mirror' : 'none' },
    }),
    [symmetry],
  );
  const refs = useMemo(
    () => [...AIDS.map((a): CommandRef => ({ commandId: a.command, params: a.params(aids) })), symmetryRef],
    [aids, symmetryRef],
  );
  const availability = useAvailability(refs);
  return (
    <div className="view-chrome" role="group" data-tt-role="group" data-tt-id={`${viewId}.aids`}>
      <span className="view-zoom" data-tt-role="text" data-tt-id={`${viewId}.zoom`}>
        {zoom
          ? `${String(Math.round(zoom.zoom * 100))}%${zoom.rotation ? ` ${String(Math.round(zoom.rotation))}°` : ''}`
          : ''}
      </span>
      {AIDS.map((a) => {
        const on = a.on?.(aids);
        const params = a.params(aids);
        return (
          <button
            key={a.id}
            type="button"
            disabled={!enabledIn(availability({ commandId: a.command, params }))}
            aria-pressed={on}
            title={a.label}
            data-tt-role="button"
            data-tt-id={`${viewId}.aid.${a.id}`}
            data-tt-command={a.command}
            data-tt-params={JSON.stringify(params)}
            onClick={() => {
              void actions.run(a.command, params);
            }}
          >
            {a.label}
            {a.id === 'staticLightTablePanel' && lightTableEntries > 0
              ? ` (${String(lightTableEntries)})`
              : ''}
          </button>
        );
      })}
      <button
        type="button"
        disabled={!enabledIn(availability(symmetryRef))}
        aria-pressed={symmetry !== 'none'}
        title="Mirror symmetry"
        data-tt-role="button"
        data-tt-id={`${viewId}.symmetry`}
        data-tt-command="tool.symmetry.set"
        data-tt-params={JSON.stringify(symmetryRef.params)}
        onClick={() => {
          // The shown state follows the kernel's `tool.options.changed`, not this click's answer.
          void actions.run('tool.symmetry.set', symmetryRef.params);
        }}
      >
        Symmetry
      </button>
    </div>
  );
}
