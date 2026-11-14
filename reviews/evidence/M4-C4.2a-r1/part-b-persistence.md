# Part B persistence audit

Spec scope authority: spec/02_drawing.md ?12.3 lines 1105?1123. `drawing.json` refers to UserDirs.settings, `session.json` to project session state. Normal persistence is tested by real executor destruction and recreation in private user directories plus project close/reopen, not by calling the deserializer alone.

| Setting | Spec scope | Storage | Same-user executor restart | Project close/reopen | Evidence |
|---|---|---|---|---|---|
| Pen list | project | project content | after project reopened | yes | part_b_pens round trip |
| Current brush/eraser | project | session.json | after project reopened | yes | part_b_pens round trip selects brush; eraser default remains |
| Active tool, group last-used, options | user | drawing.json | yes | yes | part_b_aids normal persistence: polyline/group and snap/radius |
| Symmetry mode/angle/mirrors | user | drawing.json | yes | yes | same test: mirror/30/default 6 |
| Draw Behind/Draw on Top Layer/Snap to Contour | user | drawing.json | yes | yes | same test: all enabled |
| Tablet settings | user | drawing.json | yes | yes | same test: three booleans enabled; interface default |
| Grid visible/type/overlay | user | drawing.json | yes | yes | same test: true/field16/true |
| Onion enabled/previous/next | user | drawing.json | yes | yes | same test: true/3/2 |
| Onion appearance | user | drawing.json | yes | yes | same test: all switches inverted and colours changed |
| Auto light table / auto and static shade | user | drawing.json | yes | yes | same test: true/false/false |
| Show Strokes/current on top/selection display | user | drawing.json | yes | yes | same test: all switches inverted |
| Background/use scene colour | user | drawing.json | yes | yes | same test: #123456/true |
| Display quality | user | preferences.json | yes | yes | same test: antialiasing/filtering/overlay/fast pencil/resolution/cache budget |
| Static light-table entries | scene | in-memory scene session | no | no | SI-085, SI-126, SI-131 explicitly defer session-file persistence to M5 C5.2 |
| Automatic gap closing | user | preferences.json | existing C4.1 path | existing C4.1 path | source audit; no new independent restart probe |

`user_aids_and_options_survive_restart_without_document_content` compares canonical document entries before/after settings edits and exercises save/reopen. This checks that those edits do not enter content. It does not compare two complete saved archives, which contain session data and save metadata.

Settings recovery: independent `unsupported_version_and_whole_shape_are_told_once_and_kept` passes for unsupported major, array data and non-object aids, asserts one report on two calls and distinct preserved copies. `drawing_write_failure_is_reported_once` passes with a portable file blocking the settings directory. `stale_aid_values_fall_back_individually` fails: semantically invalid members are restored without validation (C42A-R1-04).
