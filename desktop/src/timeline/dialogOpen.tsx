// Opening the Timeline's dialogs (reviews/M5-design-timeline-view.md section 7): each menu item's dialog, the value it
// collects and the command reference its OK completes; Cancel answers nothing and nothing is sent. A dialog whose field
// is typed text (a name, a note, a colour) sends it itself and stays open on a refusal (ruling E-53).
import type React from 'react';

import type { DialogsApi } from '../shell/dialogsApi';
import { NewElementsDialog, NumberDialog, TextDialog } from './dialogs';
import type { DialogItem } from './menus';
import type { RowDisplay, RowsAnswer, TimelineRow } from './model';

/** A completed command reference: what the dialog's OK sends. */
export interface CompletedRef {
  readonly command: string;
  readonly params: Record<string, unknown>;
}

/** What a dialog may read to prefill its field: a dialog that edits an existing value opens with it (C51B-R1S-05). */
export interface DialogContext {
  readonly rows: readonly TimelineRow[];
  readonly selected: readonly string[];
  readonly scene: RowsAnswer | null;
  /** The rows' display fields as read (`element.get`): the note and the element's own colours. */
  readonly display?: ReadonlyMap<string, RowDisplay>;
  /** The current frame the Timeline shows. */
  readonly currentFrame?: number;
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

/** What a dialog with a typed field sends at OK (ruling E-53): null when the kernel took it, else the refusal. */
export type SubmitTyped = (ref: CompletedRef) => Promise<string | null>;

/** A text dialog's render: OK sends the completed reference through `submit` (the dialog then answers null: the
 *  reference was sent); a refusal keeps it open. */
function textDialog(
  id: string,
  title: string,
  label: string,
  command: string,
  options: {
    initial?: string;
    multiline?: boolean;
  },
  complete: (t: string) => CompletedRef,
  submit: SubmitTyped,
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
        submit={(t) => submit(complete(t))}
        onDone={() => {
          close(null);
        }}
      />
    );
  }
  return Rendered;
}

/** Opens the dialog of a menu item; resolves with the reference to send, or null when cancelled or when the dialog sent
 *  its typed value itself through `submit` (ruling E-53). Each title is written where the dialog opens, so the D-44
 *  inventory (`scripts/c43_d44_coverage.py`) finds every dialog. */
export function openTimelineDialog(
  dialogs: DialogsApi,
  item: DialogItem,
  ctx: DialogContext,
  submit: SubmitTyped,
): Promise<CompletedRef | null> {
  const row = ctx.rows.find((r) => r.elementId === item.elementId);
  const targets = item.elementId
    ? ctx.selected.includes(item.elementId)
      ? [...ctx.selected]
      : [item.elementId]
    : [...ctx.selected];
  const length = ctx.scene?.sceneLength ?? 1;
  const shown = item.elementId ? ctx.display?.get(item.elementId) : undefined;
  const colour = (key: 'marker' | 'background') => (t: string) => ({
    command: 'element.colour.set',
    params: { elementIds: targets, [key]: t },
  });
  switch (item.dialog) {
    case 'newElements':
      return dialogs.open<CompletedRef>('New Elements', (close) => (
        <NewElementsDialog
          above={item.elementId}
          submit={submit}
          onDone={() => {
            close(null);
          }}
        />
      ));
    case 'rename': {
      const render = textDialog(
        'timelineRename',
        'Rename Element',
        'Name',
        'element.rename',
        { initial: row?.name ?? '' },
        (t) => ({ command: 'element.rename', params: { elementId: item.elementId, name: t } }),
        submit,
      );
      return dialogs.open<CompletedRef>('Rename Element', render);
    }
    case 'note': {
      // 04 lines 405-408: with a note, the action is Update Element Note and the editor opens with the note's text (OK
      // without typing keeps it).
      const note = shown?.note ?? '';
      const complete = (t: string) => ({
        command: 'element.note.set',
        params: { elementId: item.elementId, text: t },
      });
      if (note !== '') {
        const render = textDialog(
          'timelineNote',
          'Update Element Note',
          'Note (empty removes it)',
          'element.note.set',
          { initial: note, multiline: true },
          complete,
          submit,
        );
        return dialogs.open<CompletedRef>('Update Element Note', render);
      }
      const render = textDialog(
        'timelineNote',
        'Element Note',
        'Note (empty removes it)',
        'element.note.set',
        { multiline: true },
        complete,
        submit,
      );
      return dialogs.open<CompletedRef>('Element Note', render);
    }
    case 'markerColour': {
      const render = textDialog(
        'timelineMarkerColour',
        'Marker Colour',
        'Colour (#RRGGBB; empty reverts to the default)',
        'element.colour.set',
        // The element's own colour, or empty while it shows the default (OK without typing keeps either).
        { initial: shown?.ownMarkerColour ?? '' },
        colour('marker'),
        submit,
      );
      return dialogs.open<CompletedRef>('Marker Colour', render);
    }
    case 'backgroundColour': {
      const render = textDialog(
        'timelineBackgroundColour',
        'Background Colour',
        'Colour (#RRGGBB; empty reverts to the default)',
        'element.colour.set',
        { initial: shown?.backgroundColour ?? '' },
        colour('background'),
        submit,
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
        // The value it edits: the current frame (the clicked frame when the Timeline has none to show).
        ctx.currentFrame ?? item.frame ?? 1,
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
