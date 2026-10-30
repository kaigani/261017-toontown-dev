# Inventory register, round-2 response

Every row the reviewer classed `divergent` in round 2 (621), re-checked with the reviewer's own criteria (`build-register.py`). Now: {'deferred-with-owner': 727, 'implemented': 69}.

| Command | R2 | Now | R2 schema issues | Closed by | Strict placeholders (spec issue) |
|---|---|---|---:|---|---|
| `anim.bone.create` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.bone.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.bone.influence.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.bone.parent.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.bone.pose.set` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `anim.bone.reach.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.bone.rest.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.camera.create` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.camera.current.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.camera.current.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.camera.currentChanged` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.camera.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.camera.get` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `anim.camera.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.camera.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.camera.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.changed` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `anim.channel.copy` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.channel.cut` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.channel.paste` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.depth.move` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.allPegs.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.antialiasToolOverlays.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.arms.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.display.envelopes.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.frameMarkers.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.get` | divergent | deferred-with-owner | 9 | C12-R2-02 (generator) | — |
| `anim.display.pegGhosts.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.pivotPath.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.display.smoothPaths.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.evaluate` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.flip.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.function.curveType.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.function.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-023) |
| `anim.function.key.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.function.key.duplicate` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.function.key.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.function.key.set` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `anim.function.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `anim.function.preset.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-023) |
| `anim.function.sample` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.function.set` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `anim.functionEditor.get` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `anim.functionEditor.open` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.functionEditor.syncFrame.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.functionEditor.view.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.hierarchy.attach` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.hierarchy.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.hierarchy.detach` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.hierarchy.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `anim.hierarchy.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.key.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.key.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `anim.key.move` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.key.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.loop.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.node.get` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | 1 (SI-023) |
| `anim.node.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.node.pathDisplay.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.path.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.path.lockDepth.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.path.point.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.path.point.remove` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.path.point.select` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.path.point.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.path.sample` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.path.set` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `anim.peg.create` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.pegOnlyMode.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.point.copy` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.point.paste` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.prefs.get` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `anim.prefs.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-023) |
| `anim.projection.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | 1 (SI-023) |
| `anim.record.completed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.record.options.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.record.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.record.run` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `anim.reset.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.segment.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.segment.uniform.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `anim.skeleton.create` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.skeleton.deform.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.restPose.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.root.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.skin.add` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.skeleton.skin.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.snapLastKey.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.stretch.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.tool.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.tool.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.tool.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `anim.transform.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.view.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `anim.view.slot.select` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `anim.view.type.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `capture.auto.start` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.device.added` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.device.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-024) |
| `capture.device.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.device.removed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.device.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.device.settings.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 2 (SI-024) |
| `capture.display.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `capture.frame.add` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.frame.addFromFile` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `capture.frame.captured` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `capture.inputLock.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.keyColour.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.keying.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `capture.lightTable.references.list` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `capture.lightTable.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `capture.preview.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-024) |
| `capture.session.error` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `capture.session.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.session.open` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `capture.source.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `capture.target.set` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `cell.blank.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.blank.remove` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.clipboard.apply` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `cell.clipboard.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.columnate.apply` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `cell.cycle.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `cell.drawing.duplicate` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `cell.exposure.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.exposure.remove` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.exposure.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.exposures.changed` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `cell.fill.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.label.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.note.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `cell.range.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.range.delete` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.range.move` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.select` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `cell.selection.changed` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `cell.selection.get` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `checkpoint.create` | divergent | implemented | 0 | C12-R2-03 | — |
| `checkpoint.delete` | divergent | implemented | 0 | C12-R2-03 | — |
| `checkpoint.list` | divergent | implemented | 0 | C12-R2-03 | — |
| `colour.autoColourUpdate.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.current.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.current.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `colour.current.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.fillMapping.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `colour.fillMapping.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 1 (SI-025) |
| `colour.gap.close` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.gapClosing.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `colour.gapClosing.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.missing.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.paint.apply` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `colour.paint.remove` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `colour.paintUnpainted.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.palette.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.palette.create` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 1 (SI-025) |
| `colour.palette.duplicate` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-025) |
| `colour.palette.export` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.palette.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.palette.import` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.palette.list` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `colour.palette.rename` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.palette.select` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.palette.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-025) |
| `colour.pick` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `colour.recolour.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.recovery.performed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.recovery.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.sceneStyle.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.sceneStyles.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.selection.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.style.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.style.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.style.duplicate` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.style.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `colour.style.rename` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.style.select` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.swatch.changed` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `colour.swatch.create` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 2 (SI-025) |
| `colour.swatch.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `colour.swatch.list` | divergent | deferred-with-owner | 9 | C12-R2-02 (generator) | — |
| `colour.swatch.move` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `colour.swatch.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-025) |
| `colour.swatchNames.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `colour.tintBlend.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.tintOffset.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `colour.usage.list` | divergent | deferred-with-owner | 12 | C12-R2-02 (generator) | — |
| `doc.diff` | divergent | implemented | 0 | C12-R2-12 | — |
| `doc.object.patch` | divergent | implemented | 0 | C12-R2-11 | — |
| `draw.arrange.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.breakApart.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.centreLine.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.content.add` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 2 (SI-026) |
| `draw.current.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | 1 (SI-026) |
| `draw.cut.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.drawing.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.drawing.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.drawing.rename` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.drawing.render` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.ellipse.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.envelope.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.erase.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-026) |
| `draw.feather.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.flatten.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.flip.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.group.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.group.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.hitTest.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-026) |
| `draw.inspectorPreview.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.line.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.linesToBrush.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.lock.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.object.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.optimize.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.pivot.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.point.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.point.delete` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.point.move` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.point.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.pointSelection.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.polyline.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.rectangle.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.rotate.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.segment.move` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.selection.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-026) |
| `draw.selection.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.selection.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.smooth.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.stroke.add` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `draw.stroke.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.text.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 1 (SI-026) |
| `draw.text.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `draw.textFormat.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.transform.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.vectorFile.import` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `draw.vectorize.preview` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.vectorize.run` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `draw.viewMapping.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `drawAid.autoLightTable.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `drawAid.background.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `drawAid.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-027) |
| `drawAid.currentOnTop.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `drawAid.grid.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `drawAid.lightTableShade.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 2 (SI-027) |
| `drawAid.onionSkin.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `drawAid.onionSkinCycle.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `drawAid.onionSkinStyle.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `drawAid.selectionDisplay.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `drawAid.state.get` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `drawAid.staticLightTable.add` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `drawAid.staticLightTable.list` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `drawAid.staticLightTablePanel.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `drawAid.strokes.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `edit.clipboard.set` | divergent | implemented | 0 | C12-R2-10 | — |
| `edit.copy` | divergent | implemented | 0 | C12-R2-05/08/13 | — |
| `edit.cut` | divergent | implemented | 0 | C12-R2-05/08/13 | — |
| `edit.cutSpecial` | divergent | implemented | 0 | C12-R2-05/08 | — |
| `edit.delete` | divergent | implemented | 0 | C12-R2-05/08 | — |
| `edit.paste` | divergent | implemented | 0 | C12-R2-04/05/08/09/13/14 | — |
| `edit.pasteAsNewElement` | divergent | implemented | 0 | C12-R2-04/08 | — |
| `edit.pasteSpecial` | divergent | implemented | 0 | C12-R2-04/05/08/09/13/14 | — |
| `element.annotation.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.childExposure.apply` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.clone` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.collapsed.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.colour.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.create` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.current.move` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.current.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.delete` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.drawing.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.drawing.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.drawing.delete` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `element.drawing.duplicate` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.drawing.list` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `element.drawing.rename` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.drawingContent.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.drawingLock.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.duplicate` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-028) |
| `element.hierarchy.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `element.hierarchy.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.list` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `element.lock.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.mediaLength.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.note.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.parent.remove` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.parent.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.parentPeg.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.properties.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.rename` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `element.solo.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.state.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.timing.set` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `element.typeColour.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `element.unusedDrawing.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `element.visibility.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `export.drawingPdf.run` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `export.formats.list` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | 3 (SI-029) |
| `export.job.finished` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `export.movie.defaults.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-029) |
| `export.movie.defaults.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 2 (SI-029) |
| `export.movie.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-029) |
| `export.movie.validate` | divergent | deferred-with-owner | 9 | C12-R2-02 (generator) | 1 (SI-029) |
| `export.publish.account.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `export.publish.categories.list` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `export.publish.run` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 1 (SI-029) |
| `export.publish.service.list` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `export.publish.validate` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 2 (SI-029) |
| `export.report.get` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | 1 (SI-029) |
| `export.settings.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `export.settings.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-029) |
| `export.snapshot.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.allEffects.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.allEffects.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.allEffects.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.block.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `fx.block.defaults.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.block.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.block.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.block.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.block.seed.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.block.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.block.type.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.create` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.curve.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.dropShadow.transform.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.hitTest.run` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `fx.keyframe.add` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.keyframe.list` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.list` | divergent | deferred-with-owner | 9 | C12-R2-02 (generator) | — |
| `fx.paramElement.list` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.paramRows.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.particles.get` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `fx.preset.apply` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.preset.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preset.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 2 (SI-030) |
| `fx.preset.export` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preset.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 2 (SI-030) |
| `fx.preset.list` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `fx.preset.rename` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preview.cache.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preview.frame.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preview.frameRendered` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `fx.preview.open` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preview.options.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `fx.preview.options.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `fx.preview.render` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.preview.status.get` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `fx.renderList.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.renderer.capabilities.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `fx.renderer.options.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.shake.displacement.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `fx.simulation.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `fx.spriteSet.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `fx.spriteSet.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `fx.timing.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `fx.types.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `fx.types.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-030) |
| `import.completed` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `import.file.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-031) |
| `import.file.run` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-031) |
| `import.formats.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `import.keyedImage.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `import.keyedImages.run` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 1 (SI-031) |
| `import.movie.run` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-031) |
| `import.options.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `import.options.set` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `import.vectorize.settings.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `import.vectorize.settings.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `job.wait` | divergent | implemented | 0 | C12-R2-06/07 | — |
| `keymap.export` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `keymap.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `keymap.import` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `keymap.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `keymap.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `keymap.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-032) |
| `library.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.close` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.closed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.drawing.insert` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `library.folder.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.folder.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.folder.rename` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.item.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.item.duplicate` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.item.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-033) |
| `library.item.import` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `library.item.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.item.move` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.item.rename` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.item.thumbnail.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.list` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.node.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.open` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `library.opened` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `library.preview.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `library.preview.set` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `library.project.get` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `library.save` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `library.selection.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.selection.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.starter.open` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.template.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.template.create` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `library.template.edit.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `library.template.edit.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `library.template.edit.start` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.template.edit.stop` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.template.export` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.template.frame.render` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.template.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.template.insert` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.template.search` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.templateUpdate.list` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `library.tree.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `library.view.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `library.view.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `library.writeAccess.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `lipsync.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 3 (SI-034) |
| `lipsync.detect` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-034) |
| `lipsync.display.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `lipsync.mapping.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `lipsync.mapping.mouth.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `lipsync.mapping.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-034) |
| `lipsync.mouth.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `lipsync.panel.state.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `lipsync.settings.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | 1 (SI-034) |
| `lipsync.settings.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 2 (SI-034) |
| `lipsync.shapes.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | 1 (SI-034) |
| `lipsync.template.import` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `lipsync.template.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `lipsync.track.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `lipsync.track.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `lipsync.track.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `lipsync.track.stale` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `media.movie.import` | divergent | deferred-with-owner | 7 | C12-R2-02 (generator) | — |
| `media.movie.probe` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `media.object.list` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | — |
| `media.object.relink` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `media.sequence.add` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `media.sequence.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `media.sequence.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `media.sequence.move` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `media.sequence.naturalTiming.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `media.sequence.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `media.sound.play` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `media.source.missing` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `playback.forceRate.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `playback.loop.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `playback.player.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-035) |
| `playback.player.open` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `playback.player.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-035) |
| `playback.preview.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `playback.range.changed` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `playback.range.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `playback.range.mode.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `playback.range.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `playback.rate.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `playback.start` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `playback.started` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `playback.state.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `playback.stop` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `playback.stopped` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `prefs.list` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | 1 (SI-036) |
| `print.pageSetup.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `print.pageSetup.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `print.preview.get` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `print.printer.list` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `print.run` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `print.settings.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `print.settings.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-037) |
| `render.frame.run` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `render.limits.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `render.range.run` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.asset.get` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `sound.asset.import` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.asset.list` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | — |
| `sound.asset.missing` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.asset.probe` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | — |
| `sound.asset.relink` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.editor.close` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.editor.open` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.editor.state.get` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | — |
| `sound.editor.state.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.element.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.element.streamed.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.envelope.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.envelope.point.add` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.envelope.point.move` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.envelope.point.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.envelope.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.fade.apply` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.import` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | — |
| `sound.placement.add` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.placement.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.placement.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.placement.move` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.placement.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.placement.split` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.placement.trim` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `sound.play` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.playback.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.range.delete` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `sound.range.select` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.recent.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.scrubbing.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-038) |
| `sound.settings.get` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | — |
| `sound.silence.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `sound.waveform.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.waveform.ready` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `sound.waveforms.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.currentScene.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.currentScene.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `storyline.currentScene.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.frame.get` | divergent | deferred-with-owner | 9 | C12-R2-02 (generator) | — |
| `storyline.frame.render` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.play.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `storyline.play.start` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.play.stop` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.playhead.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `storyline.playhead.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.scene.create` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `storyline.scene.delete` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.scene.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.scene.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.scene.rename` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.scene.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.scene.visible.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.selection.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `storyline.selection.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `storyline.selection.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-039) |
| `storyline.transition.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.transition.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.transition.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.transition.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.transition.types.list` | divergent | deferred-with-owner | 8 | C12-R2-02 (generator) | 1 (SI-039) |
| `storyline.view.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `storyline.view.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `storyline.warning.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `timeline.backgroundColour.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.currentFrame.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `timeline.currentFrame.move` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.currentFrame.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.effectParameterRows.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.playbackRange.changed` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `timeline.playbackRange.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `timeline.playbackRange.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `timeline.rows.list` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.singleCellSelection.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.soundWaveforms.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.timeDisplay.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.transferOptions.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `timeline.transferOptions.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `timeline.view.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `tool.active.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.active.get` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `tool.active.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.brush.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.drawTopLayer.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.eraser.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.group.select` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 2 (SI-040) |
| `tool.options.changed` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `tool.options.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-040) |
| `tool.options.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-040) |
| `tool.pen.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.pen.create` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | 1 (SI-040) |
| `tool.pen.delete` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `tool.pen.list` | divergent | deferred-with-owner | 10 | C12-R2-02 (generator) | — |
| `tool.pen.rename` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `tool.pen.select` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `tool.pen.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-040) |
| `tool.pencil.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.placeBehind.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.polyline.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.select.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.settings.get` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `tool.snapToContour.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.symmetry.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `tool.tablet.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `tool.text.options.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `ui.contextHelpMode.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `ui.help.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `ui.message.shown` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | 1 (SI-041) |
| `ui.startScreen.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `ui.statusBar.get` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | 1 (SI-041) |
| `ui.statusBar.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `ui.theme.list` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `ui.tips.list` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `ui.tips.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `view.active.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `view.coordinates.convert` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `view.list` | divergent | deferred-with-owner | 5 | C12-R2-02 (generator) | 1 (SI-042) |
| `view.pan.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `view.rotation.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `view.transform.get` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `view.transform.reset` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `view.type.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `view.zoom.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | — |
| `xsheet.columnWidth.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `xsheet.currentCell.set` | divergent | deferred-with-owner | 2 | C12-R2-02 (generator) | — |
| `xsheet.display.set` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | — |
| `xsheet.exposures.get` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `xsheet.exposures.set` | divergent | deferred-with-owner | 4 | C12-R2-02 (generator) | 2 (SI-043) |
| `xsheet.lightTable.add` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | 1 (SI-043) |
| `xsheet.lightTable.changed` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `xsheet.lightTable.list` | divergent | deferred-with-owner | 6 | C12-R2-02 (generator) | — |
| `xsheet.lightTable.remove` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `xsheet.lightTable.set` | divergent | deferred-with-owner | 1 | C12-R2-02 (generator) | — |
| `xsheet.panel.set` | divergent | deferred-with-owner | 3 | C12-R2-02 (generator) | 2 (SI-043) |
