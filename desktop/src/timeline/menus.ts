// The Timeline's three context menus (04 §11.9 lines 697-708; reviews/M5-design-timeline-view.md section 5): the row,
// cell and ruler contexts, as data for the shared `ContextMenu` (D-60, D-66). Each item is a command reference with the
// clicked row's and frame's parameters; its availability, check state and disabled reason come from the kernel (O-10),
// so an item whose command belongs to a later checkpoint shows that owner. Items that collect a value open a dialog
// (`dialog`), handled by the panel's dialog layer (section 7).
//
// Every command 04 lines 704-706 require is present (ruling E-47; `menus.test.ts` holds the three models against a table
// transcribed from those lines): a command a later checkpoint owns is shown disabled with that owner (the deferred list,
// or the kernel's "arrives with" for a command not implemented yet), and an item no command performs yet carries its
// owner (`owner`). A toggle or a radio item shows its state (04 line 708): `check`, with the state the kernel completes.
import type { MenuGroup } from '../shell/contextMenuModel';
import { CELL_BASED, loopable, PARENTABLE, type TimelineRow } from './model';

/** The dialogs a menu item opens (view note section 7). */
export type TimelineDialog =
  | 'newElements'
  | 'rename'
  | 'note'
  | 'markerColour'
  | 'backgroundColour'
  | 'changeStart'
  | 'changeDuration'
  | 'changeLoops'
  | 'extendChildren'
  | 'currentFrame'
  | 'rangeStart'
  | 'rangeEnd';

/** A dialog item: the dialog and the target it acts on. */
export interface DialogItem {
  readonly id: string;
  readonly label: string;
  readonly dialog: TimelineDialog;
  readonly elementId?: string;
  readonly frame?: number;
}

/** A menu: the command groups and the dialog items (shown after the group they belong to). */
export interface TimelineMenu {
  readonly groups: readonly MenuGroup[];
  readonly dialogs: readonly DialogItem[];
}

/** The element types of Add (04 §3.1; `element.create`'s `type`). */
export const ADD_TYPES: readonly { readonly type: string; readonly label: string }[] = [
  { type: 'drawing', label: 'Drawing' },
  { type: 'annotation', label: 'Annotation' },
  { type: 'image', label: 'Image' },
  { type: 'sound', label: 'Sound' },
  { type: 'media', label: 'Media' },
  { type: 'peg', label: 'Peg' },
  { type: 'camera', label: 'Camera' },
  { type: 'skeleton', label: 'Skeleton' },
  { type: 'colourTransform', label: 'Colour Transform' },
  { type: 'clipping', label: 'Clipping' },
  { type: 'dropShadow', label: 'Drop Shadow' },
  { type: 'blur', label: 'Blur' },
  { type: 'shake', label: 'Shake' },
  { type: 'particleNature', label: 'Particle Nature' },
  { type: 'particleImpact', label: 'Particle Impact' },
];

const EFFECT_TYPES = new Set([
  'colourTransform',
  'clipping',
  'dropShadow',
  'blur',
  'shake',
  'particleNature',
  'particleImpact',
]);

/** The row context (04 line 704): the clicked row `row`, the rows the Timeline shows `rows`, the selected rows. */
export function rowMenu(
  row: TimelineRow,
  rows: readonly TimelineRow[],
  selected: readonly string[],
  currentFrame = 1,
  hasNote = false,
): TimelineMenu {
  const id = row.elementId;
  // Toggle Constant Z acts on the segment at the current frame: the motion keyframe that starts it.
  const segmentStart = [...row.keyframes]
    .filter((k) => k.frame <= currentFrame && k.channels.includes('position'))
    .map((k) => k.frame)
    .at(-1);
  const targets = selected.includes(id) ? [...selected] : [id];
  const ids = { elementIds: targets };
  const index = rows.findIndex((r) => r.elementId === id);
  const next = index >= 0 ? rows[index + 1] : undefined;
  const groups: MenuGroup[] = [
    {
      label: 'Add',
      items: [
        ...ADD_TYPES.map((t) => ({
          id: `timeline.row.add.${t.type}`,
          label: `Add ${t.label}`,
          command: 'element.create',
          params: { type: t.type, above: id },
        })),
        {
          id: 'timeline.row.addParentPeg',
          label: 'Add Parent Peg',
          command: 'element.parentPeg.create',
          params: ids,
        },
      ],
    },
    {
      // 04 line 704 with 09: the imports (each owned by a later checkpoint until its command is bound).
      label: 'Import',
      items: [
        { id: 'timeline.row.importImage', label: 'Import Image…', command: 'import.file.run', params: {} },
        {
          id: 'timeline.row.importVectorize',
          label: 'Import and Vectorize…',
          command: 'draw.vectorize.run',
          params: {},
        },
        {
          id: 'timeline.row.importArtwork',
          label: 'Import Artwork…',
          command: 'import.file.run',
          params: {},
        },
        { id: 'timeline.row.importSound', label: 'Import Sound…', command: 'sound.import', params: {} },
        {
          id: 'timeline.row.importMovie',
          label: 'Import Movie as Image Sequence…',
          command: 'import.movie.run',
          params: {},
        },
      ],
    },
    {
      items: [
        { id: 'timeline.row.cut', label: 'Cut', command: 'edit.cut', params: { context: 'timeline' } },
        { id: 'timeline.row.copy', label: 'Copy', command: 'edit.copy', params: { context: 'timeline' } },
        { id: 'timeline.row.paste', label: 'Paste', command: 'edit.paste', params: { context: 'timeline' } },
        { id: 'timeline.row.delete', label: 'Delete', command: 'element.delete', params: ids },
      ],
    },
    {
      label: 'Arrange',
      items: (
        [
          ['front', 'Bring to Front'],
          ['forward', 'Bring Forward'],
          ['backward', 'Send Backward'],
          ['back', 'Send to Back'],
        ] as const
      ).map(([to, label]) => ({
        id: `timeline.row.arrange.${to}`,
        label,
        command: 'element.order.move',
        params: { ...ids, to, order: 'timeline' },
      })),
    },
    {
      items: [
        row.collapsed
          ? {
              id: 'timeline.row.expand',
              label: 'Expand',
              command: 'element.collapsed.set',
              params: { ...ids, collapsed: false },
            }
          : {
              id: 'timeline.row.collapse',
              label: 'Collapse',
              command: 'element.collapsed.set',
              params: { ...ids, collapsed: true },
            },
      ],
    },
    {
      label: 'Select',
      items: (
        [
          ['parent', 'Select Parent'],
          ['children', 'Select Children'],
          ['firstChild', 'Select Child'],
          ['previousSibling', 'Select Previous Sibling'],
          ['nextSibling', 'Select Next Sibling'],
          ['skeletonRoot', 'Select Skeleton Root'],
        ] as const
      ).map(([relation, label]) => ({
        id: `timeline.row.select.${relation}`,
        label,
        command: 'element.hierarchy.select',
        params: { relation, elementId: id },
      })),
    },
    {
      items: [
        // Attach to Next Object (04 §9.2): the row below when it can be a parent.
        ...(next && PARENTABLE.has(next.type)
          ? [
              {
                id: 'timeline.row.attachNext',
                label: 'Attach to Next Object',
                command: 'element.parent.set',
                params: { ...ids, parentId: next.elementId },
              },
            ]
          : []),
        {
          id: 'timeline.row.detach',
          label: 'Detach from Parent',
          command: 'element.parent.remove',
          params: ids,
        },
      ],
    },
    {
      label: 'Visibility',
      items: [
        {
          id: 'timeline.row.showHide',
          label: row.visibleInScene ? 'Hide' : 'Show',
          command: 'element.visibility.set',
          params: { ...ids, scope: 'scene', visible: !row.visibleInScene },
        },
        {
          id: 'timeline.row.showAll',
          label: 'Show All',
          command: 'element.visibility.set',
          params: { scope: 'scene', visible: true, apply: 'all' },
        },
        {
          id: 'timeline.row.hideOthers',
          label: 'Hide All Others',
          command: 'element.visibility.set',
          params: { ...ids, scope: 'scene', visible: false, apply: 'others' },
        },
        {
          id: 'timeline.row.showOthers',
          label: 'Show All Others',
          command: 'element.visibility.set',
          params: { ...ids, scope: 'scene', visible: true, apply: 'others' },
        },
        {
          id: 'timeline.row.hideAll',
          label: 'Hide All',
          command: 'element.visibility.set',
          params: { scope: 'scene', visible: false, apply: 'all' },
        },
        {
          id: 'timeline.row.showAnnotations',
          label: 'Show All Annotation Elements',
          command: 'element.visibility.set',
          params: { scope: 'scene', visible: true, apply: 'all', types: ['annotation'] },
        },
        {
          id: 'timeline.row.hideAnnotations',
          label: 'Hide All Annotation Elements',
          command: 'element.visibility.set',
          params: { scope: 'scene', visible: false, apply: 'all', types: ['annotation'] },
        },
      ],
    },
    {
      label: 'Colour',
      items: [
        {
          id: 'timeline.row.defaultColour',
          label: 'Default Colour',
          command: 'element.colour.set',
          params: { ...ids, marker: '', background: '' },
        },
      ],
    },
    {
      label: 'Effect parameters',
      items: [
        ...(EFFECT_TYPES.has(row.type)
          ? [
              {
                // A toggle: the kernel completes `visible` to the next state and reports the current one.
                id: 'timeline.row.effectParameters',
                label: 'Show Effect Parameters',
                command: 'timeline.effectParameterRows.set',
                params: { elementId: id },
                check: true,
              },
            ]
          : []),
        {
          id: 'timeline.row.showAllParameters',
          label: 'Show All Effect Parameters',
          command: 'timeline.effectParameterRows.set',
          params: { visible: true },
        },
        {
          id: 'timeline.row.hideAllParameters',
          label: 'Hide All Effect Parameters',
          command: 'timeline.effectParameterRows.set',
          params: { visible: false },
        },
      ],
    },
    {
      items:
        row.type === 'drawing' || row.type === 'annotation'
          ? [
              {
                id: 'timeline.row.annotation',
                label: row.type === 'annotation' ? 'Set as Drawing Element' : 'Set as Annotation',
                command: 'element.annotation.set',
                params: { elementId: id, annotation: row.type !== 'annotation' },
              },
            ]
          : [],
    },
    {
      label: 'Lock',
      items: [
        {
          id: 'timeline.row.lock',
          label: row.locked ? 'Unlock' : 'Lock',
          command: 'element.lock.set',
          params: { ...ids, locked: !row.locked },
        },
        {
          id: 'timeline.row.lockOthers',
          label: 'Lock All Others',
          command: 'element.lock.set',
          params: { ...ids, locked: true, apply: 'others' },
        },
        {
          id: 'timeline.row.lockAll',
          label: 'Lock All',
          command: 'element.lock.set',
          params: { locked: true, apply: 'all' },
        },
        {
          id: 'timeline.row.unlockAll',
          label: 'Unlock All',
          command: 'element.lock.set',
          params: { locked: false, apply: 'all' },
        },
      ],
    },
    {
      items: [
        { id: 'timeline.row.clone', label: 'Clone', command: 'element.clone', params: { elementId: id } },
        { id: 'timeline.row.duplicate', label: 'Duplicate', command: 'element.duplicate', params: ids },
      ],
    },
    {
      label: 'Animation',
      items: [
        {
          id: 'timeline.row.pegReset',
          label: 'Peg Reset',
          command: 'anim.reset.apply',
          params: { nodeIds: targets },
        },
        {
          id: 'timeline.row.removeKeys',
          label: 'Remove All Keyframes',
          command: 'anim.key.remove',
          params: { nodeIds: targets, all: true },
        },
        {
          // A toggle (05 §4): the kernel completes `enabled` and reports the segment's state. Without a motion keyframe
          // at or before the current frame there is no segment: the reference is complete and the kernel refuses it.
          id: 'timeline.row.constantZ',
          label: 'Toggle Constant Z',
          command: 'anim.path.lockDepth.set',
          params:
            segmentStart === undefined
              ? { nodeId: id, frame: currentFrame, enabled: true }
              : { nodeId: id, frame: segmentStart },
          check: true,
        },
        { id: 'timeline.row.addDefaultKey', label: 'Add Default Keyframe', command: '', owner: 'M6 C6.2' },
      ],
    },
    {
      label: 'Sound',
      items: [
        {
          id: 'timeline.row.editSound',
          label: 'Edit Sound',
          command: 'sound.editor.open',
          params: { elementId: id },
        },
        {
          id: 'timeline.row.waveforms',
          label: 'Sound Waveforms',
          command: 'timeline.soundWaveforms.set',
          params: {},
          check: true,
        },
      ],
    },
  ];
  const dialogs: DialogItem[] = [
    { id: 'timeline.row.markerColour', label: 'Marker Colour…', dialog: 'markerColour', elementId: id },
    {
      id: 'timeline.row.backgroundColour',
      label: 'Background Colour…',
      dialog: 'backgroundColour',
      elementId: id,
    },
    { id: 'timeline.row.rename', label: 'Rename…', dialog: 'rename', elementId: id },
    // 04 lines 405-408: presented as Update Element Note when the element has one.
    {
      id: 'timeline.row.note',
      label: hasNote ? 'Update Element Note…' : 'Element Note…',
      dialog: 'note',
      elementId: id,
    },
    ...(row.bar && !CELL_BASED.has(row.type)
      ? [
          {
            id: 'timeline.row.changeStart',
            label: 'Change Start Frame…',
            dialog: 'changeStart' as const,
            elementId: id,
          },
          {
            id: 'timeline.row.changeDuration',
            label: 'Change Duration…',
            dialog: 'changeDuration' as const,
            elementId: id,
          },
          ...(loopable(row.type)
            ? [
                {
                  id: 'timeline.row.changeLoops',
                  label: 'Change Loops…',
                  dialog: 'changeLoops' as const,
                  elementId: id,
                },
              ]
            : []),
        ]
      : []),
  ];
  return { groups: groups.filter((g) => g.items.length > 0), dialogs };
}

/** The cell context (04 line 705): the clicked row and frame. */
export function cellMenu(row: TimelineRow, frame: number): TimelineMenu {
  const id = row.elementId;
  const cells = { elementIds: [id], from: frame, to: frame };
  // The segment the clicked cell lies in (05 §10): `anim.segment.set` and `anim.segment.uniform.apply` name a key, so
  // the items name the key that starts the segment (at or before the frame) and, for Uniform Left, the key that ends it
  // (at or after the frame). Without such a key the clicked frame is sent and the kernel changes nothing or refuses.
  const keys = row.keyframes.map((k) => k.frame).sort((x, y) => x - y);
  const startKey = keys.filter((k) => k <= frame).at(-1) ?? frame;
  const endKey = keys.find((k) => k >= frame) ?? frame;
  const groups: MenuGroup[] = [
    {
      // Ruling E-2 (a): the cell commands, disabled with their owner M5 C5.2 until it binds them in both views.
      label: 'Exposure',
      items: [
        {
          id: 'timeline.cell.addExposure',
          label: 'Add Exposure',
          command: 'cell.exposure.add',
          params: cells,
        },
        {
          id: 'timeline.cell.removeExposure',
          label: 'Remove Exposure',
          command: 'cell.exposure.remove',
          params: cells,
        },
        ...([1, 2, 3] as const).map((n) => ({
          id: `timeline.cell.exposure${String(n)}`,
          label: `Set Exposure to ${String(n)}`,
          command: 'cell.exposure.set',
          params: { ...cells, length: n },
        })),
        {
          id: 'timeline.cell.exposureN',
          label: 'Set Exposure to N…',
          command: 'cell.exposure.set',
          params: cells,
        },
        {
          id: 'timeline.cell.fill',
          label: 'Extend Exposure',
          command: 'cell.fill.apply',
          params: { elementIds: [id], frame },
        },
      ],
    },
    {
      label: 'Cells',
      items: [
        {
          id: 'timeline.cell.insertCells',
          label: 'Insert Cells…',
          command: 'cell.range.add',
          params: { elementIds: [id], frame },
        },
        {
          id: 'timeline.cell.insertBlank',
          label: 'Insert Blank Cell',
          command: 'cell.blank.add',
          params: { elementIds: [id], frame },
        },
        {
          id: 'timeline.cell.deleteBlank',
          label: 'Delete Blank Cell',
          command: 'cell.blank.remove',
          params: { elementIds: [id], frame },
        },
        {
          id: 'timeline.cell.cycle',
          label: 'Create Cycle…',
          command: 'cell.cycle.create',
          params: { elementId: id, from: frame, to: frame },
        },
        {
          id: 'timeline.cell.columnate',
          label: 'Columnate…',
          command: 'cell.columnate.apply',
          params: { elementId: id, frame },
        },
        {
          id: 'timeline.cell.duplicateDrawing',
          label: 'Duplicate Drawing',
          command: 'cell.drawing.duplicate',
          params: { elementId: id, from: frame, to: frame },
        },
      ],
    },
    {
      items: [
        { id: 'timeline.cell.cut', label: 'Cut', command: 'edit.cut', params: { context: 'timeline' } },
        { id: 'timeline.cell.copy', label: 'Copy', command: 'edit.copy', params: { context: 'timeline' } },
        { id: 'timeline.cell.paste', label: 'Paste', command: 'edit.paste', params: { context: 'timeline' } },
        {
          id: 'timeline.cell.delete',
          label: 'Delete',
          command: 'edit.delete',
          params: { context: 'timeline' },
        },
      ],
    },
    {
      // Ruling E-2 (b): bound in C5.1b on every row (a row that cannot be keyed is refused by the kernel, with its
      // reason on the item).
      label: 'Keyframes',
      items: [
        {
          id: 'timeline.cell.addKey',
          label: 'Add Keyframe',
          command: 'anim.key.add',
          params: { nodeIds: [id], frame },
        },
        {
          id: 'timeline.cell.removeKey',
          label: 'Remove Keyframe',
          command: 'anim.key.remove',
          params: { nodeIds: [id], frame },
        },
        {
          id: 'timeline.cell.constant',
          label: 'Set Constant Segment',
          command: 'anim.segment.set',
          params: { nodeIds: [id], frame: startKey, constant: true },
        },
        {
          id: 'timeline.cell.nonConstant',
          label: 'Set Non-Constant Segment',
          command: 'anim.segment.set',
          params: { nodeIds: [id], frame: startKey, constant: false },
        },
        {
          id: 'timeline.cell.uniformLeft',
          label: 'Uniform Left',
          command: 'anim.segment.uniform.apply',
          params: { nodeId: id, frame: endKey, side: 'left' },
        },
        {
          id: 'timeline.cell.uniformRight',
          label: 'Uniform Right',
          command: 'anim.segment.uniform.apply',
          params: { nodeId: id, frame: startKey, side: 'right' },
        },
        { id: 'timeline.cell.addDefaultKey', label: 'Add Default Keyframe', command: '', owner: 'M6 C6.2' },
      ],
    },
    {
      // 07 §6: the sound items, owned by M8 (Fit Sound has no command yet).
      label: 'Sound',
      items: [
        {
          id: 'timeline.cell.editSound',
          label: 'Edit Sound',
          command: 'sound.editor.open',
          params: { elementId: id },
        },
        {
          id: 'timeline.cell.splitSound',
          label: 'Split Sound',
          command: 'sound.placement.split',
          params: { frame },
        },
        { id: 'timeline.cell.fitSound', label: 'Fit Sound', command: '', owner: 'M8' },
      ],
    },
  ];
  const dialogs: DialogItem[] =
    row.type === 'peg'
      ? [
          {
            id: 'timeline.cell.extendChildren',
            label: 'Extend Children Exposure…',
            dialog: 'extendChildren',
            elementId: id,
            frame,
          },
        ]
      : [];
  return { groups, dialogs };
}

/** The ruler context (04 line 706, 09 line 85). */
export function rulerMenu(frame: number): TimelineMenu {
  return {
    groups: [
      {
        label: 'Playback range',
        items: [
          // A radio pair: the kernel reports which mode is current (04 line 708).
          {
            id: 'timeline.ruler.rangeAuto',
            label: 'Automatic Fit',
            command: 'timeline.playbackRange.set',
            params: { mode: 'auto' },
            check: true,
          },
          {
            id: 'timeline.ruler.rangeFree',
            label: 'Free',
            command: 'timeline.playbackRange.set',
            params: { mode: 'free' },
            check: true,
          },
          {
            id: 'timeline.ruler.startHere',
            label: 'Set Playback Start Here',
            command: 'timeline.playbackRange.set',
            params: { start: frame },
          },
          {
            id: 'timeline.ruler.endHere',
            label: 'Set Playback End Here',
            command: 'timeline.playbackRange.set',
            params: { end: frame },
          },
        ],
      },
      {
        label: 'Sound',
        items: [
          {
            id: 'timeline.ruler.waveforms',
            label: 'Sound Waveforms',
            command: 'timeline.soundWaveforms.set',
            params: {},
            check: true,
          },
          {
            id: 'timeline.ruler.soundPlayback',
            label: 'Sound Playback',
            command: 'sound.playback.set',
            params: {},
            check: true,
          },
          {
            id: 'timeline.ruler.soundScrubbing',
            label: 'Sound Scrubbing',
            command: 'sound.scrubbing.set',
            params: {},
            check: true,
          },
        ],
      },
    ],
    dialogs: [
      { id: 'timeline.ruler.currentFrame', label: 'Set Current Frame…', dialog: 'currentFrame', frame },
      { id: 'timeline.ruler.rangeStart', label: 'Set Playback Start Frame…', dialog: 'rangeStart', frame },
      { id: 'timeline.ruler.rangeEnd', label: 'Set Playback End Frame…', dialog: 'rangeEnd', frame },
    ],
  };
}
