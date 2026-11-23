// Opening the Timeline's dialogs (reviews/M5-design-timeline-view.md section 7): each menu item's dialog, the value it
// collects and the command reference its OK completes; Cancel answers nothing and nothing is sent.
import type React from 'react';

import type { DialogsApi } from '../shell/dialogsApi';
import { NewElementsDialog, NumberDialog, TextDialog } from './dialogs';
import type { DialogItem } from './menus';
import type { RowsAnswer, TimelineRow } from './model';

/** A completed command reference: what the dialog's OK sends. */
export interface CompletedRef {
  readonly command: string;
  readonly params: Record<string, unknown>;
}

/** What a dialog may read to prefill its field. */
export interface DialogContext {
  readonly rows: readonly TimelineRow[];
  readonly selected: readonly string[];
  readonly scene: RowsAnswer | null;
}

const HEX = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/;

/** A positive whole number in 1..max, or null. */
export function wholeIn(text: string, min: number, max: number): number | null {
  if (!/^\s*\d+\s*$/.test(text)) return null;
  const n = Number(text);
  return Number.isSafeInteger(n) && n >= min && n <= max ? n : null;
}

/** A whole-number dialog's render, completing `complete` with the number typed. */
function numberDialog(
  id: string,
  title: string,
  label: string,
  initial: number,
  complete: (n: number) => CompletedRef,
): (close: (v: CompletedRef | null) => void) => React.ReactNode {
  function Rendered(close: (v: CompletedRef | null) => void) {
    return (
      <NumberDialog
        id={id}
        title={title}
        label={label}
        initial={initial}
        min={1}
        max={100_000}
        command={complete(1).command}
        onDone={(n) => {
          close(n === null ? null : complete(n));
        }}
      />
    );
  }
  return Rendered;
}

/** A text dialog's render. */
function textDialog(
  id: string,
  title: string,
  label: string,
  command: string,
  options: {
    initial?: string;
    multiline?: boolean;
    required?: boolean;
    validate?: (t: string) => string | null;
  },
  complete: (t: string) => CompletedRef,
): (close: (v: CompletedRef | null) => void) => React.ReactNode {
  function Rendered(close: (v: CompletedRef | null) => void) {
    return (
      <TextDialog
        id={id}
        title={title}
        label={label}
        initial={options.initial ?? ''}
        command={command}
        {...(options.multiline ? { multiline: true } : {})}
        {...(options.required ? { required: true } : {})}
        {...(options.validate ? { validate: options.validate } : {})}
        onDone={(t) => {
          close(t === null ? null : complete(t));
        }}
      />
    );
  }
  return Rendered;
}

/** Opens the dialog of a menu item; resolves with the reference to send, or null when cancelled. Each title is written
 *  where the dialog opens, so the D-44 inventory (`scripts/c43_d44_coverage.py`) finds every dialog. */
export function openTimelineDialog(
  dialogs: DialogsApi,
  item: DialogItem,
  ctx: DialogContext,
): Promise<CompletedRef | null> {
  const row = ctx.rows.find((r) => r.elementId === item.elementId);
  const targets = item.elementId
    ? ctx.selected.includes(item.elementId)
      ? [...ctx.selected]
      : [item.elementId]
    : [...ctx.selected];
  const length = ctx.scene?.sceneLength ?? 1;
  const colour = (key: 'marker' | 'background') => (t: string) => ({
    command: 'element.colour.set',
    params: { elementIds: targets, [key]: t },
  });
  const hex = (t: string) =>
    t === '' || HEX.test(t) ? null : 'Type a colour as #RRGGBB or #RRGGBBAA, or leave it empty.';
  switch (item.dialog) {
    case 'newElements':
      return dialogs.open<CompletedRef>('New Elements', (close) => (
        <NewElementsDialog above={item.elementId} onDone={close} />
      ));
    case 'rename': {
      const render = textDialog(
        'timelineRename',
        'Rename Element',
        'Name',
        'element.rename',
        { initial: row?.name ?? '', required: true },
        (t) => ({ command: 'element.rename', params: { elementId: item.elementId, name: t } }),
      );
      return dialogs.open<CompletedRef>('Rename Element', render);
    }
    case 'note': {
      const render = textDialog(
        'timelineNote',
        'Element Note',
        'Note (empty removes it)',
        'element.note.set',
        { multiline: true },
        (t) => ({ command: 'element.note.set', params: { elementId: item.elementId, text: t } }),
      );
      return dialogs.open<CompletedRef>('Element Note', render);
    }
    case 'markerColour': {
      const render = textDialog(
        'timelineMarkerColour',
        'Marker Colour',
        'Colour (#RRGGBB; empty reverts to the default)',
        'element.colour.set',
        { validate: hex },
        colour('marker'),
      );
      return dialogs.open<CompletedRef>('Marker Colour', render);
    }
    case 'backgroundColour': {
      const render = textDialog(
        'timelineBackgroundColour',
        'Background Colour',
        'Colour (#RRGGBB; empty reverts to the default)',
        'element.colour.set',
        { validate: hex },
        colour('background'),
      );
      return dialogs.open<CompletedRef>('Background Colour', render);
    }
    case 'changeStart': {
      const render = numberDialog(
        'timelineStart',
        'Change Start Frame',
        'Start frame',
        row?.bar?.start ?? 1,
        (n) => ({ command: 'element.timing.set', params: { elementIds: targets, start: n } }),
      );
      return dialogs.open<CompletedRef>('Change Start Frame', render);
    }
    case 'changeDuration': {
      const render = numberDialog(
        'timelineDuration',
        'Change Duration',
        'Duration (frames)',
        row?.bar ? row.bar.end - row.bar.start + 1 : 1,
        (n) => ({ command: 'element.timing.set', params: { elementIds: targets, duration: n } }),
      );
      return dialogs.open<CompletedRef>('Change Duration', render);
    }
    case 'changeLoops': {
      const render = numberDialog('timelineLoops', 'Change Loops', 'Loops', row?.bar?.loops ?? 1, (n) => ({
        command: 'element.timing.set',
        params: { elementIds: targets, loops: n },
      }));
      return dialogs.open<CompletedRef>('Change Loops', render);
    }
    case 'extendChildren': {
      const render = numberDialog(
        'timelineExtendChildren',
        'Extend Children Exposure',
        'To frame',
        item.frame ?? length,
        (n) => ({ command: 'element.childExposure.apply', params: { pegId: item.elementId, toFrame: n } }),
      );
      return dialogs.open<CompletedRef>('Extend Children Exposure', render);
    }
    case 'currentFrame': {
      const render = numberDialog(
        'timelineCurrentFrame',
        'Set Current Frame',
        'Frame',
        item.frame ?? 1,
        (n) => ({ command: 'timeline.currentFrame.set', params: { frame: n } }),
      );
      return dialogs.open<CompletedRef>('Set Current Frame', render);
    }
    case 'rangeStart': {
      const render = numberDialog(
        'timelineRangeStart',
        'Set Playback Start Frame',
        'Start frame',
        ctx.scene?.playbackRange.start ?? 1,
        (n) => ({ command: 'timeline.playbackRange.set', params: { start: n } }),
      );
      return dialogs.open<CompletedRef>('Set Playback Start Frame', render);
    }
    case 'rangeEnd': {
      const render = numberDialog(
        'timelineRangeEnd',
        'Set Playback End Frame',
        'End frame',
        ctx.scene?.playbackRange.end ?? length,
        (n) => ({ command: 'timeline.playbackRange.set', params: { end: n } }),
      );
      return dialogs.open<CompletedRef>('Set Playback End Frame', render);
    }
  }
}

/** The command a dialog item ends in, for its reference (availability, parity); the dialog completes it. */
export function dialogCommand(d: DialogItem): { command: string; params: Record<string, unknown> } {
  const ids = d.elementId ? { elementIds: [d.elementId] } : {};
  switch (d.dialog) {
    case 'newElements':
      return { command: 'element.create', params: {} };
    case 'rename':
      return { command: 'element.rename', params: d.elementId ? { elementId: d.elementId } : {} };
    case 'note':
      return { command: 'element.note.set', params: d.elementId ? { elementId: d.elementId } : {} };
    case 'markerColour':
    case 'backgroundColour':
      return { command: 'element.colour.set', params: ids };
    case 'changeStart':
    case 'changeDuration':
    case 'changeLoops':
      return { command: 'element.timing.set', params: ids };
    case 'extendChildren':
      return { command: 'element.childExposure.apply', params: d.elementId ? { pegId: d.elementId } : {} };
    case 'currentFrame':
      return { command: 'timeline.currentFrame.set', params: {} };
    case 'rangeStart':
    case 'rangeEnd':
      return { command: 'timeline.playbackRange.set', params: {} };
  }
}
