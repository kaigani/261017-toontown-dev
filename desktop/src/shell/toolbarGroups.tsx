// The main toolbar's group items for the drawing aids (01 §5.7 lines 451-458; 02 §10): Grid type and the two onion-skin
// counts are pop-ups whose members each send the aid's command with that member; Overlay is a toggle. Their state is
// the kernel's (`drawAid.state.get`, re-read on `drawAid.changed`, so an agent's change shows), and they are enabled
// when the kernel's availability of the aid's command says so (ruling O-10). C4.3b round 1 (D-35): the collector
// rendered the default toolbar and found these items unbound with an owner that had passed (M4 C4.2).
import { useCallback, useEffect, useState } from 'react';

import { useShell } from '../state/context';
import { useActions } from './actionsApi';
import type { CommandRef } from './availability';
import { enabledIn, useAvailability } from './useAvailability';

type Json = Record<string, unknown>;
const obj = (v: unknown): Json => (v !== null && typeof v === 'object' ? (v as Json) : {});

/** The drawing aids' state, from the kernel, re-read when it changes. */
function useAids(): Json {
  const { engine } = useShell();
  const [aids, setAids] = useState<Json>({});
  const refresh = useCallback(() => {
    void engine
      .run('drawAid.state.get', {})
      .done.then((env) => {
        setAids(env.status === 'ok' ? obj(env.result) : {});
      })
      .catch(() => undefined);
  }, [engine]);
  useEffect(() => {
    refresh();
    let off: (() => void) | null = null;
    let stopped = false;
    void engine
      .onEvent((e) => {
        if (e.name === 'drawAid.changed' || e.name === 'project.opened' || e.name === 'project.closed')
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
  return aids;
}

const GRID_REF: CommandRef = { commandId: 'drawAid.grid.set', params: {} };
const ONION_REF: CommandRef = { commandId: 'drawAid.onionSkin.set', params: {} };
const REFS: readonly CommandRef[] = [GRID_REF, ONION_REF];

const GRID_TYPES: readonly [string, string][] = [
  ['normal', 'Square grid'],
  ['field12', '12-field grid'],
  ['field16', '16-field grid'],
];

/** One of the drawing-aid group items, by its toolbar id; `null` for any other item. */
export function AidGroupItem({ item }: { item: string }) {
  const actions = useActions();
  const aids = useAids();
  const grid = obj(aids['grid']);
  const onion = obj(aids['onionSkin']);
  const availability = useAvailability(REFS);
  const gridOn = enabledIn(availability(GRID_REF));
  const onionOn = enabledIn(availability(ONION_REF));
  if (item === 'gridType') {
    const current = typeof grid['type'] === 'string' ? grid['type'] : 'normal';
    return (
      <select
        aria-label="Grid type"
        title="Grid type"
        value={current}
        disabled={!gridOn}
        data-tt-role="field"
        data-tt-id="mainToolbar.gridType"
        data-tt-command="drawAid.grid.set"
        onChange={(e) => {
          void actions.run('drawAid.grid.set', { type: e.target.value });
        }}
      >
        {GRID_TYPES.map(([v, label]) => (
          <option key={v} value={v}>
            {label}
          </option>
        ))}
      </select>
    );
  }
  if (item === 'gridOverlay') {
    const on = grid['overlay'] === true;
    return (
      <button
        type="button"
        aria-label="Overlay"
        aria-pressed={on}
        title="Grid over the drawings"
        disabled={!gridOn}
        data-tt-role="button"
        data-tt-id="mainToolbar.gridOverlay"
        data-tt-command="drawAid.grid.set"
        data-tt-params={JSON.stringify({ overlay: !on })}
        onClick={() => {
          void actions.run('drawAid.grid.set', { overlay: !on });
        }}
      >
        Overlay
      </button>
    );
  }
  if (item === 'onionSkinPrevious' || item === 'onionSkinNext') {
    const key = item === 'onionSkinPrevious' ? 'previous' : 'next';
    const label = item === 'onionSkinPrevious' ? 'Previous drawings' : 'Next drawings';
    const current = typeof onion[key] === 'number' ? onion[key] : 0;
    return (
      <select
        aria-label={label}
        title={label}
        value={String(current)}
        disabled={!onionOn}
        data-tt-role="field"
        data-tt-id={`mainToolbar.${item}`}
        data-tt-command="drawAid.onionSkin.set"
        onChange={(e) => {
          void actions.run('drawAid.onionSkin.set', { [key]: Number(e.target.value) });
        }}
      >
        {[0, 1, 2, 3].map((n) => (
          <option key={n} value={String(n)}>
            {n}
          </option>
        ))}
      </select>
    );
  }
  return null;
}
