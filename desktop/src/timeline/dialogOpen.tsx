// Opening the Timeline's dialogs (reviews/M5-design-timeline-view.md section 7): each menu item's dialog, the value it
// collects and the command reference its OK completes; Cancel answers nothing and nothing is sent.
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

/** Opens the dialog of a menu item; resolves with the reference to send, or null when cancelled. */
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
  const number = (
    id: string,
    title: string,
    label: string,
    initial: number,
    min: number,
    max: number,
    complete: (n: number) => CompletedRef,
  ) =>
    dialogs.open<CompletedRef>(title, (close) => (
      <NumberDialog
        id={id}
        title={title}
        label={label}
        initial={initial}
        min={min}
        max={max}
        command={complete(min).command}
        onDone={(n) => {
          close(n === null ? null : complete(n));
        }}
      />
    ));
  const length = ctx.scene?.sceneLength ?? 1;
  switch (item.dialog) {
    case 'newElements':
      return dialogs.open<CompletedRef>('New Elements', (close) => (
        <NewElementsDialog above={item.elementId} onDone={close} />
      ));
    case 'rename':
      return dialogs.open<CompletedRef>('Rename Element', (close) => (
        <TextDialog
          id="timelineRename"
          title="Rename Element"
          label="Name"
          initial={row?.name ?? ''}
          command="element.rename"
          required
          onDone={(t) => {
            close(
              t === null
                ? null
                : { command: 'element.rename', params: { elementId: item.elementId, name: t } },
            );
          }}
        />
      ));
    case 'note':
      return dialogs.open<CompletedRef>('Element Note', (close) => (
        <TextDialog
          id="timelineNote"
          title="Element Note"
          label="Note (empty removes it)"
          initial=""
          multiline
          command="element.note.set"
          onDone={(t) => {
            close(
              t === null
                ? null
                : { command: 'element.note.set', params: { elementId: item.elementId, text: t } },
            );
          }}
        />
      ));
    case 'markerColour':
    case 'backgroundColour': {
      const key = item.dialog === 'markerColour' ? 'marker' : 'background';
      const title = item.dialog === 'markerColour' ? 'Marker Colour' : 'Background Colour';
      return dialogs.open<CompletedRef>(title, (close) => (
        <TextDialog
          id={item.dialog === 'markerColour' ? 'timelineMarkerColour' : 'timelineBackgroundColour'}
          title={title}
          label="Colour (#RRGGBB; empty reverts to the default)"
          initial=""
          command="element.colour.set"
          validate={(t) =>
            t === '' || HEX.test(t) ? null : 'Type a colour as #RRGGBB or #RRGGBBAA, or leave it empty.'
          }
          onDone={(t) => {
            close(
              t === null
                ? null
                : { command: 'element.colour.set', params: { elementIds: targets, [key]: t } },
            );
          }}
        />
      ));
    }
    case 'changeStart':
      return number(
        'timelineStart',
        'Change Start Frame',
        'Start frame',
        row?.bar?.start ?? 1,
        1,
        100_000,
        (n) => ({
          command: 'element.timing.set',
          params: { elementIds: targets, start: n },
        }),
      );
    case 'changeDuration':
      return number(
        'timelineDuration',
        'Change Duration',
        'Duration (frames)',
        row?.bar ? row.bar.end - row.bar.start + 1 : 1,
        1,
        100_000,
        (n) => ({ command: 'element.timing.set', params: { elementIds: targets, duration: n } }),
      );
    case 'changeLoops':
      return number('timelineLoops', 'Change Loops', 'Loops', row?.bar?.loops ?? 1, 1, 100_000, (n) => ({
        command: 'element.timing.set',
        params: { elementIds: targets, loops: n },
      }));
    case 'extendChildren':
      return number(
        'timelineExtendChildren',
        'Extend Children Exposure',
        'To frame',
        item.frame ?? length,
        1,
        100_000,
        (n) => ({
          command: 'element.childExposure.apply',
          params: { pegId: item.elementId, toFrame: n },
        }),
      );
    case 'currentFrame':
      return number(
        'timelineCurrentFrame',
        'Set Current Frame',
        'Frame',
        item.frame ?? 1,
        1,
        100_000,
        (n) => ({
          command: 'timeline.currentFrame.set',
          params: { frame: n },
        }),
      );
    case 'rangeStart':
      return number(
        'timelineRangeStart',
        'Set Playback Start Frame',
        'Start frame',
        ctx.scene?.playbackRange.start ?? 1,
        1,
        100_000,
        (n) => ({
          command: 'timeline.playbackRange.set',
          params: { start: n },
        }),
      );
    case 'rangeEnd':
      return number(
        'timelineRangeEnd',
        'Set Playback End Frame',
        'End frame',
        ctx.scene?.playbackRange.end ?? length,
        1,
        100_000,
        (n) => ({
          command: 'timeline.playbackRange.set',
          params: { end: n },
        }),
      );
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
