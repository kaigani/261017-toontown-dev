# Twenty added/changed rows inspected

| Command | Code | Class | Reason | States |
|---|---|---|---|---|
| `colour.gap.close` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.paint.apply` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.paint.remove` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.paintUnpainted.apply` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.pick` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `draw.centreLine.apply` | `drawing.centreLineOnFill` | precheck | the selection holds painted zones but no brush strokes | refuse: drawing+painted-zone-selected · pass: drawing-selected |
| `drawAid.background.set` | `arg.invalid` | parameter | the colour the caller gives is not `#RRGGBB` or `#RRGGBBAA` |  |
| `drawAid.onionSkin.set` | `tool.valueOutOfRange` | parameter | an onion-skin count the caller gives is outside 0-3 |  |
| `drawAid.onionSkinStyle.set` | `arg.invalid` | parameter | a colour the caller gives is not `#RRGGBB` or `#RRGGBBAA` |  |
| `drawAid.staticLightTable.add` | `drawing.noTarget` | precheck | no current drawing (the current element's cell at the frame is empty) | refuse: no-drawing · pass: drawing |
| `drawAid.staticLightTable.add` | `drawing.notFound` | parameter | the drawing the caller names is not used by any element |  |
| `drawAid.staticLightTable.add` | `element.notFound` | parameter | the element the caller names does not exist or has no scene |  |
| `drawAid.staticLightTable.list` | `command.disabled` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `drawAid.staticLightTable.remove` | `arg.invalid` | parameter | neither `entryIds` nor `all` is given |  |
| `drawAid.staticLightTable.remove` | `command.disabled` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `drawAid.staticLightTable.set` | `arg.invalid` | parameter | neither `entryIds`, `all` nor `onlyVisibleEntryId` is given |  |
| `drawAid.staticLightTable.set` | `command.disabled` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `render.frame.run` | `command.notImplemented` | unreachable | `active_view` runs only while the active view window shows the Drawing view, the window `drawing_view(None)` then returns |  |
| `render.frame.run` | `drawing.noTarget` | precheck | `composition: "activeView"` with the Drawing view active and no current drawing | refuse: no-drawing+drawing-view {"composition": "activeView"} · pass: drawing+drawing-view {"composition": "activeView"} |
| `render.frame.run` | `render.compositionUnavailable` | precheck | `xsheet` (M5 C5.1), or `activeView` while the active view window is not the Drawing view (M4 C4.3) | refuse: drawing {"composition": "activeView"} · pass: drawing+drawing-view {"composition": "activeView"} |

The first five view-space rows depend on caller space/view; colour.rs resolves those through view::drawing_view. Aid validation rows depend on supplied colour/count/id/selector. Static table no-target and no-scene rows match precheck_add/precheck_scene; render rows match render::precheck and active_view guard. Centre-line row now has two constructible states.
