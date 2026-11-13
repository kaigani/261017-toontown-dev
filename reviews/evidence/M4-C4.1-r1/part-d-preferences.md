# Part D preference catalogue comparison

Columns preserve spec wording; empty path means platform default, empty timeline colour means theme default. Agent keys are compared with spec 11 section 15 command defaults; spec 11 does not define a separate complete preference-key table.

| Key | Catalogue type | Catalogue default | Catalogue range / values | Spec 01 type / default / range | Authority |
|---|---|---|---|---|---|
| user.name | string | "" | {"maxLength": 100} | string / "" (empty = the OS account's full name) / ≤ 100 chars | 01 section 7.3 |
| user.copyright | string | "" | {"maxLength": 200} | string / "" / ≤ 200 chars | 01 section 7.3 |
| library.globalPath | path | "" | {} | path / per-user documents folder / application library folder / existing writable folder | 01 section 7.3 |
| general.startup.behaviour | enum | "startScreen" | {"values": ["startScreen", "lastProject", "openDialog", "newDialog", "newDefaultProject"]} | enum / `startScreen` / `startScreen` / `lastProject` / `openDialog` / `newDialog` / `newDefaultProject` | 01 section 7.3 |
| general.language | enum | "en" | {"values": ["en"]} | enum / system language if installed, else `en` / installed localisations, shown by their own names | 01 section 7.3 |
| general.recentProjects.maxCount | int | 10 | {"min": 0, "max": 30} | int / 10 / 0–30 | 01 section 7.3 |
| general.workspace.autoSave | bool | true | {} | bool / true / — | 01 section 7.3 |
| files.autosave.enabled | bool | true | {} | bool / true / — | 01 section 7.3 |
| files.autosave.intervalMinutes | int | 5 | {"min": 1, "max": 60} | int / 5 / 1–60 | 01 section 7.3 |
| files.backups.count | int | 3 | {"min": 0, "max": 20} | int / 3 / 0–20 (0 = off) | 01 section 7.3 |
| project.defaults.frameRate | number | 24 | {"min": 1, "max": 120} | number / 24 / 1–120 | 01 section 7.3 |
| project.defaults.cameraWidth | int | 1920 | {"min": 16, "max": 8192} | int / 1920 / 16–8192 | 01 section 7.3 |
| project.defaults.cameraHeight | int | 1080 | {"min": 16, "max": 8192} | int / 1080 / 16–8192 | 01 section 7.3 |
| interface.theme | enum | "system" | {"values": ["system", "light", "dark"], "userThemes": true} | enum / `system` / `system` / `light` / `dark` / installed user themes | 01 section 7.3 |
| interface.statusBar.visible | bool | true | {} | bool / true / — | 01 section 7.3 |
| interface.mainToolbar.visible | bool | true | {} | bool / true / — | 01 section 7.3 |
| interface.timeMeasurement | enum | "frames" | {"values": ["frames", "feetAndFrames"]} | enum / `frames` / `frames` / `feetAndFrames` | 01 section 7.3 |
| interface.elementColours.drawing | colour | "#8FD98FFF" | {} | colour / #8FD98FFF / any | 01 section 7.3 |
| interface.elementColours.image | colour | "#A8E0A8FF" | {} | colour / #A8E0A8FF / any | 01 section 7.3 |
| interface.elementColours.sound | colour | "#B8A8D8FF" | {} | colour / #B8A8D8FF / any | 01 section 7.3 |
| interface.elementColours.peg | colour | "#8080FFFF" | {} | colour / #8080FFFF / any | 01 section 7.3 |
| interface.elementColours.skeleton | colour | "#6A8FB5FF" | {} | colour / #6A8FB5FF / any | 01 section 7.3 |
| interface.elementColours.bone | colour | "#7FA3C8FF" | {} | colour / #7FA3C8FF / any | 01 section 7.3 |
| interface.elementColours.media | colour | "#A0A050FF" | {} | colour / #A0A050FF / any | 01 section 7.3 |
| interface.elementColours.camera | colour | "#FF8080FF" | {} | colour / #FF8080FF / any | 01 section 7.3 |
| interface.elementColours.effect | colour | "#C88040FF" | {} | colour / #C88040FF / any | 01 section 7.3 |
| interface.elementColours.effectParameter | colour | "#A05030FF" | {} | colour / #A05030FF / any | 01 section 7.3 |
| interface.storylineColours.scene | colour | "#9CC3E6FF" | {} | colour / #9CC3E6FF / any | 01 section 7.3 |
| interface.storylineColours.sceneWarning | colour | "#F2A65AFF" | {} | colour / #F2A65AFF / any | 01 section 7.3 |
| interface.storylineColours.transition | colour | "#C9C9C9FF" | {} | colour / #C9C9C9FF / any | 01 section 7.3 |
| interface.background.timeline | colour | "" | {"themeDefault": true} | colour / theme default / any | 01 section 7.3 |
| interface.background.drawingView | colour | "#FFFFFFFF" | {} | colour / #FFFFFFFF / any | 01 section 7.3 |
| drawing.onionSkin.enabled | bool | false | {} | bool / false / — | 01 section 7.3 |
| drawing.onionSkin.previousCount | int | 1 | {"min": 0, "max": 3} | int / 1 / 0–3 | 01 section 7.3 |
| drawing.onionSkin.nextCount | int | 1 | {"min": 0, "max": 3} | int / 1 / 0–3 | 01 section 7.3 |
| drawing.onionSkin.transparency | bool | false | {} | bool / false / — | 01 section 7.3 |
| drawing.onionSkin.colourShading | bool | true | {} | bool / true / — | 01 section 7.3 |
| drawing.onionSkin.previousColour | colour | "#FF0000FF" | {} | colour / #FF0000FF / any | 01 section 7.3 |
| drawing.onionSkin.nextColour | colour | "#00C000FF" | {} | colour / #00C000FF / any | 01 section 7.3 |
| drawing.onionSkin.outlineOnly | bool | false | {} | bool / false / — | 01 section 7.3 |
| drawing.lightTable.staticShade | bool | true | {} | bool / true / — | 01 section 7.3 |
| drawing.lightTable.autoShade | bool | true | {} | bool / true / — | 01 section 7.3 |
| drawing.inspectorPreview | bool | true | {} | (no row) | SI-117 |
| paint.gapClosing | enum | "off" | {"values": ["off", "small", "medium", "large"]} | (no row) | SI-117 |
| display.antiAliasing | bool | true | {} | bool / true / — | 01 section 7.3 |
| display.smoothBitmaps | bool | true | {} | bool / true / — | 01 section 7.3 |
| display.smoothToolOverlays | bool | true | {} | bool / true / — | 01 section 7.3 |
| display.smoothMotionPaths | bool | true | {} | bool / true / — | 01 section 7.3 |
| display.fastPencilRendering | bool | false | {} | bool / false / — | 01 section 7.3 |
| display.bitmapPreviewMaxSize | int | 0 | {"min": 0, "max": 8192, "excludedRange": [1, 255]} | int / 0 / 0 (full) or 256–8192 | 01 section 7.3 |
| display.cacheMemoryPercent | int | 30 | {"min": 5, "max": 80} | int / 30 / 5–80 | 01 section 7.3 |
| display.sceneBackground.drawingView | bool | false | {} | bool / false / — | 01 section 7.3 |
| display.sceneBackground.stagingViews | bool | true | {} | bool / true / — | 01 section 7.3 |
| tablet.ignore | bool | false | {} | bool / false / — | 01 section 7.3 |
| tablet.alternativeInput | bool | false | {} | bool / false / — | 01 section 7.3 |
| tablet.portraitCorrection | bool | false | {} | bool / false / — | 01 section 7.3 |
| staging.motionPoint.defaultTension | number | 0.0 | {"min": -1.0, "max": 1.0} | number / 0.0 / −1.0–1.0 | 01 section 7.3 |
| staging.motionPoint.defaultContinuity | number | 0.0 | {"min": -1.0, "max": 1.0} | number / 0.0 / −1.0–1.0 | 01 section 7.3 |
| staging.motionPoint.defaultBias | number | 0.0 | {"min": -1.0, "max": 1.0} | number / 0.0 / −1.0–1.0 | 01 section 7.3 |
| staging.units.showSuffix | bool | true | {} | bool / true / — | 01 section 7.3 |
| staging.units.coordinate | enum | "fields" | {"values": ["fields", "inches", "centimetres"]} | enum / `fields` / `fields` / `inches` / `centimetres` | 01 section 7.3 |
| staging.units.angle | enum | "degrees" | {"values": ["degrees", "radians"]} | enum / `degrees` / `degrees` / `radians` | 01 section 7.3 |
| staging.selection.boxHighlight | bool | true | {} | bool / true / — | 01 section 7.3 |
| staging.selection.alwaysVisible | bool | true | {} | bool / true / — | 01 section 7.3 |
| staging.selection.outline | bool | false | {} | bool / false / — | 01 section 7.3 |
| staging.keyframes.linearVelocity | bool | true | {} | bool / true / — | 01 section 7.3 |
| staging.keyframes.constantByDefault | bool | false | {} | bool / false / — | 01 section 7.3 |
| staging.keyframes.snapLastFrame | bool | false | {} | bool / false / — | 01 section 7.3 |
| staging.depthNudgeStep | number | 0.01 | {"min": 0.0001, "max": 1.0} | (no row) | SI-117 |
| staging.pegOnlyMode | bool | false | {} | (no row) | SI-117 |
| timing.timeline.singleCellSelection | bool | true | {} | bool / true / — | 01 section 7.3 |
| edit.pasteSpecial.timeline | object | {"overwriteDrawings": true, "createDeleteExposure": true, "extendExposures": false, "updateColours": true, "keyframeSubstitution": false, "motionKeys": true, "rotationKeys": true, "scaleKeys": true, "pivotKeys": true, "velocityKeys": true, "skewKeys": true, "boundaryKeys": false} | {} | object / defaults of §6.3 / option set of §6.3 | 01 section 7.3 |
| edit.cutSpecial.timeline | object | {"overwriteDrawings": true, "createDeleteExposure": true, "extendExposures": false, "updateColours": true, "keyframeSubstitution": false, "motionKeys": true, "rotationKeys": true, "scaleKeys": true, "pivotKeys": true, "velocityKeys": true, "skewKeys": true, "boundaryKeys": false} | {} | object / defaults of §6.3 / option set of §6.3 | 01 section 7.3 |
| edit.dragDrop.timeline | object | {"overwriteDrawings": true, "createDeleteExposure": true, "extendExposures": false, "updateColours": true, "keyframeSubstitution": false, "motionKeys": true, "rotationKeys": true, "scaleKeys": true, "pivotKeys": true, "velocityKeys": true, "skewKeys": true, "boundaryKeys": false} | {} | object / defaults of §6.3 / option set of §6.3 | 01 section 7.3 |
| edit.pasteSpecial.motionPoint | object | {"x": true, "y": true, "z": true, "tension": true, "continuity": true, "bias": true, "angle": true, "rotationPivot": true, "scaleX": true, "scaleY": true, "scalePivot": true} | {} | object / all on / option set of §6.3 | 01 section 7.3 |
| agent.server.enabled | bool | false | {} | (no row) | 11 section 15; spelling/defaults chosen by implementation |
| agent.server.port | int | 47215 | {"min": 1024, "max": 65535} | (no row) | 11 section 15; spelling/defaults chosen by implementation |
| agent.server.allowRemote | bool | false | {} | (no row) | 11 section 15; spelling/defaults chosen by implementation |
| agent.policy.defaults | object | {"policy": "assist", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048, "maxInlineBytes": 52428800, "askInApp": true, "letAgentMoveView": true, "highlightAgentChanges": true} | {} | (no row) | 11 section 15; spelling/defaults chosen by implementation |

Missing 01 keys: []

Catalogue-only non-agent keys: ['drawing.inspectorPreview', 'paint.gapClosing', 'staging.depthNudgeStep', 'staging.pegOnlyMode']

Agent catalogue: ['agent.server.enabled', 'agent.server.port', 'agent.server.allowRemote', 'agent.policy.defaults']

Agent comparison with spec 11 section 15:

| Key | Spec constraint/default | Comparison |
|---|---|---|
| agent.server.enabled | bool, opt-in in section 4 | false matches |
| agent.server.port | int 1024..65535; no numeric default specified | range matches; 47215 is implementation choice |
| agent.server.allowRemote | bool, false | matches |
| agent.policy.defaults.policy | observe/assist/autonomous; policy presets in section 5 | assist chosen; generic object validator does not enforce enumeration |
| agent.policy.defaults.maxJobs | int 1..8, default 2 | default matches; nested range absent |
| agent.policy.defaults.maxBatchSteps | int 1..100000, default 1000 | default matches; nested range absent |
| agent.policy.defaults.maxInlineImagePx | int 256..8192, default 2048 | default matches; nested range absent |
| agent.policy.defaults.maxInlineBytes | int, default 52428800 | default matches |
| agent.policy.defaults.askInApp | bool, true | matches |
| agent.policy.defaults.letAgentMoveView | bool, true | matches |
| agent.policy.defaults.highlightAgentChanges | bool, true | matches |

All 71 literal keys from 01 section 7.3 are present. Scalar numeric defaults, bool defaults, colours and enumerations match the table. Context-dependent defaults use empty strings for platform path and theme colour; language is system/en. Object defaults follow section 6.3 and member types; they have no nested range metadata. All four additional non-agent keys are explicitly recorded in SI-117. Agent object validation discrepancy is C41-R1-11.
