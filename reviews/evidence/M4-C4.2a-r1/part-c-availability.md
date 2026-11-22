# Part C availability audit

Candidate c745a55; Windows. All execution/parameter rows in the requested command families are reproduced below. Source audit decisions for fifteen rows follow. Locks and hidden elements do not affect these user settings; project checks are static. An explicit id/name/value remains parameter-dependent even if whether it resolves depends on the document.

| Command / code | Handler evidence and decision |
|---|---|
| tool.active.set / arg.invalid | tool.rs tool_param checks supplied toolId; no implicit target. Parameter. |
| tool.brush.options.set / pen.notFound | select_pen/find_pen resolves supplied penId; omitted pen leaves current unchanged. Parameter. |
| tool.brush.options.set / tool.notApplicable | select_pen checks explicit pen family. Parameter. |
| tool.eraser.options.set / pen.notFound | Same resolver with explicit penId. Parameter. |
| tool.eraser.options.set / tool.notApplicable | Same resolver, requested Eraser family. Parameter. |
| tool.pen.create / arg.invalid | check_name and apply_settings inspect caller's values. Parameter. |
| tool.pen.create / pen.nameExists | Only explicit name calls check_name; omitted name searches next free name. Parameter. |
| tool.pen.create / pen.notFound | Only explicit fromPenId resolves it; no current pen synthesizes a default base. Parameter. |
| tool.pen.create / tool.notApplicable | Explicit fromPenId compared with requested family. Parameter. |
| tool.pen.rename / arg.invalid | Supplied name length/whitespace. Parameter. |
| tool.pen.rename / pen.nameExists | Explicit name checked against other ids. Parameter. |
| drawAid.onionSkin.set / tool.valueOutOfRange | count checks supplied previous/next only. Parameter. |
| drawAid.onionSkinStyle.set / arg.invalid | colour validates supplied strings only. Parameter. |
| drawAid.background.set / arg.invalid | colour validates supplied string only. Parameter. |
| view.coordinates.convert / arg.invalid | finite point and space choices are parameter checks, but existing view type also refuses with all arguments present: state omission, C42A-R1-08. |

Additional source checks: tool.pen.delete requires explicit penId for lastPen; static remove/set require named entries or all and have precheck_entries; empty table with all is legal; no current drawing in static add is covered by precheck_add; view navigation one-of groups have matching prechecks. No reported-unavailable/would-succeed example was established.

## Requested inventory rows

| `drawAid.background.set` | `arg.invalid` | parameter | the colour the caller gives is not `#RRGGBB` or `#RRGGBBAA` |  |
| `drawAid.onionSkin.set` | `tool.valueOutOfRange` | parameter | an onion-skin count the caller gives is outside 0-3 |  |
| `drawAid.onionSkinStyle.set` | `arg.invalid` | parameter | a colour the caller gives is not `#RRGGBB` or `#RRGGBBAA` |  |
| `drawAid.staticLightTable.add` | `drawing.notFound` | parameter | the drawing the caller names is not used by any element |  |
| `drawAid.staticLightTable.add` | `element.notFound` | parameter | the element the caller names does not exist or has no scene |  |
| `drawAid.staticLightTable.remove` | `arg.invalid` | parameter | neither `entryIds` nor `all` is given |  |
| `drawAid.staticLightTable.set` | `arg.invalid` | parameter | neither `entryIds`, `all` nor `onlyVisibleEntryId` is given |  |
| `tool.active.set` | `arg.invalid` | parameter | the tool id the caller gives is not a tool |  |
| `tool.brush.options.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.brush.options.set` | `tool.notApplicable` | parameter | the pen the caller names is an eraser pen |  |
| `tool.eraser.options.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.eraser.options.set` | `tool.notApplicable` | parameter | the pen the caller names is a brush pen |  |
| `tool.options.get` | `arg.invalid` | parameter | the tool id the caller gives is not a tool |  |
| `tool.pen.create` | `arg.invalid` | parameter | the name the caller gives is empty or longer than 128 characters, or a setting is unknown |  |
| `tool.pen.create` | `pen.nameExists` | parameter | the name the caller gives is taken |  |
| `tool.pen.create` | `pen.notFound` | parameter | the `fromPenId` the caller names does not exist |  |
| `tool.pen.create` | `tool.notApplicable` | parameter | the `fromPenId` the caller names is of the other family |  |
| `tool.pen.delete` | `pen.lastPen` | parameter | the pen the caller names is the last of its family (02 Â§6.1) |  |
| `tool.pen.delete` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.pen.rename` | `arg.invalid` | parameter | the name the caller gives is empty or longer than 128 characters |  |
| `tool.pen.rename` | `pen.nameExists` | parameter | the name the caller gives is taken by another pen |  |
| `tool.pen.rename` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.pen.select` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.pen.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.symmetry.set` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `tool.tablet.set` | `arg.invalid` | parameter | the input interface name the caller gives is empty or longer than 64 characters |  |
| `view.coordinates.convert` | `anim.notFound` | parameter | scene space (the caller's `from` or `to`) evaluates the current element, which the evaluator does not know |  |
| `view.coordinates.convert` | `anim.valueOutOfRange` | parameter | scene space (the caller's `from` or `to`) evaluates a transform out of range |  |
| `view.coordinates.convert` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `view.pan.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `view.pan.set` | `arg.missing` | parameter | none of `centre`, `by` and `atPointer` is given |  |
| `view.rotation.set` | `arg.invalid` | parameter | both `degrees` and `steps` are given, or a value is not finite |  |
| `view.rotation.set` | `arg.missing` | parameter | neither `degrees` nor `steps` is given |  |
| `view.zoom.set` | `arg.invalid` | parameter | both `zoom` and `steps` are given, or a value is not finite |  |
| `view.zoom.set` | `arg.missing` | parameter | neither `zoom` nor `steps` is given |  |

## Performance receipt

All five c41 availability tests passed, including both oracles: 332 references and 160 precheck pairs (both states). Inventory checker: 276 commands, 1142 pairs; 13 asks-user, 1 deferred, 240 execution, 647 parameter, 220 precheck, 21 unreachable.

10,101 command-created elements: empty pass 6.2487 ms; large pass 6.1664 ms. Large selection first pass 58.8943 ms, longest request 10.4798 ms; steady 6.0418 ms. Clipboard first 7.614 ms / hold 1.205 ms / steady 6.4878 ms. 10,000 exposure runs first 9.2687 ms / hold 2.5892 ms / steady 6.4628 ms. 991 cameras first 8.1864 ms / hold 1.2527 ms / steady 6.7309 ms. Longest chunk 1.0902 ms. Slowest user query 8.36 ms quiet / 9.8862 ms with continuous passes.
