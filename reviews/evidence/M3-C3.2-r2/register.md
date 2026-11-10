# C3.2 round-2 register

Reviewed head: `65cf4d88bdf530fe7970dfe37ac230198864c64c`.

{'divergent': 5, 'implemented': 31}; {'implemented': 291, 'deferred-with-owner': 90, 'divergent': 2}

## registry

| ID | State | Note |
|---|---|---|
| anim.camera.create | divergent | C32-R2-01: SI-104 explicit 41.1121 aliases to exact default during projection; default itself is fixed. |
| anim.camera.current.get | implemented | Release SDK: repeat bytes, read purity |
| anim.camera.current.set | implemented | Release SDK: dry document, execution, undo, redo |
| anim.camera.delete | implemented | Release SDK: dry document, execution, undo, redo |
| anim.camera.get | divergent | C32-R2-01: SI-104 explicit 41.1121 aliases to exact default during projection; default itself is fixed. |
| anim.camera.list | implemented | Release SDK: repeat bytes, read purity |
| anim.camera.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.evaluate | divergent | C32-R2-01: SI-104 explicit 41.1121 aliases to exact default during projection; default itself is fixed. |
| anim.function.curveType.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.function.get | implemented | Release SDK: repeat bytes, read purity |
| anim.function.key.add | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.function.key.duplicate | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.function.key.remove | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.function.key.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.function.list | implemented | Release SDK: repeat bytes, read purity |
| anim.function.preset.apply | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.function.sample | implemented | Release SDK: repeat bytes, read purity |
| anim.function.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.key.add | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.key.list | implemented | Release SDK: repeat bytes, read purity |
| anim.key.move | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.key.remove | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.loop.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.path.get | implemented | Round-1 divergence re-adjudicated against response source, inherited suites, and round-2 probes; see report closure table. |
| anim.path.lockDepth.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.path.point.add | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.path.point.remove | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.path.point.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.path.sample | implemented | Release SDK: repeat bytes, read purity |
| anim.path.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.projection.get | divergent | C32-R2-01: SI-104 explicit 41.1121 aliases to exact default during projection; default itself is fixed. |
| anim.segment.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.segment.uniform.apply | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.transform.set | implemented | Release SDK: dry document/session/history, stale guard, failing second step rollback, one revision, execution, undo, redo, save/open |
| anim.changed | divergent | C32-R2-02: direct scopes fixed, but undo and redo emit no anim.changed event. |
| anim.camera.currentChanged | implemented | Executed current-camera event coverage; coalescing is scoped to one apply_effects call. |

## source_units

| ID | State | Note |
|---|---|---|
| 05_scene_planning.md:1.1:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.1:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.1:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.2:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.2:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.2:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.2:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.2:6 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:1.3:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.3:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.3:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.3:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.3:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.3:6 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:1.4:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.4:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.4:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.4:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.4:5 | deferred-with-owner | SI-097: skeleton transform only; bone kinematics excluded. |
| 05_scene_planning.md:1.4:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.5:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.6:10 | deferred-with-owner | SI-097: skeleton transform only; bone kinematics excluded. |
| 05_scene_planning.md:1.6:11 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.7:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.7:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.7:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.7:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.7:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:1.8:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.1:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.2:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.2:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.3:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.3:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.3:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.3:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.4:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.4:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.5:1 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:2.5:2 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:2.5:3 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:2.6:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.6:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.6:3 | divergent | C32-R2-01 / SI-104: tan(FOV/2) is replaced by the exact-default tangent only for explicit 41.1121. |
| 05_scene_planning.md:2.6:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.6:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.6:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.7:1 | implemented | SI-102 exact default is stored and projects to twelve fields; inherited premise corrected by ruling. |
| 05_scene_planning.md:2.8:1 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:2.8:2 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:2.8:3 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:2.8:4 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:2.9:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.9:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.9:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.9:4 | deferred-with-owner | Render-list exposure/hidden filtering; numeric hidden-node queries follow SI-100. |
| 05_scene_planning.md:2.9:5 | deferred-with-owner | Bone kinematics and skin deformation deferred under SI-097. |
| 05_scene_planning.md:2.9:6 | deferred-with-owner | Projection is numeric C3.2; effect execution and compositing retain downstream owners. |
| 05_scene_planning.md:2.9:7 | deferred-with-owner | Scene-view overlays are future UI, not numeric evaluation. |
| 05_scene_planning.md:2.10:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.10:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:2.10:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:10 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:11 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:12 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.1:13 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:10 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:11 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:12 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.2:13 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.3:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.3:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.3:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.3:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.4:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.4:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.4:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.4:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:8.5:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.1:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.3:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.3:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.4:10 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.5:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.5:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.5:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.6:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.6:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.6:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:9.6:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.1:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.2:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.2:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.3:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.3:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.3:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.3:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.4:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.5:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.5:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.5:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.5:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.5:5 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:10.5:6 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:10.5:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:10.5:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.1:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.1:4 | deferred-with-owner | Template editing is not implemented in this checkpoint. |
| 05_scene_planning.md:12.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.2:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.2:3 | deferred-with-owner | Camera list/current commands execute; scene-view reachability is the allocated future UI. |
| 05_scene_planning.md:12.2:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.3:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.3:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.3:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.3:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.3:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.3:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:12.4:1 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:12.4:2 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:12.4:3 | deferred-with-owner | Render list, native bitmap mapping and display integration are outside numeric checkpoint acceptance. |
| 05_scene_planning.md:18:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:4 | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |
| 05_scene_planning.md:18:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:10 | deferred-with-owner | Error belongs to the named future operation, not one of the C3.2 commands. |
| 05_scene_planning.md:18:11 | deferred-with-owner | Error belongs to the named future operation, not one of the C3.2 commands. |
| 05_scene_planning.md:18:12 | deferred-with-owner | SI-097: skeleton transform only; bone kinematics excluded. |
| 05_scene_planning.md:18:13 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:18:14 | deferred-with-owner | Template editing is not implemented in this checkpoint. |
| 05_scene_planning.md:18:15 | deferred-with-owner | Error belongs to the named future operation, not one of the C3.2 commands. |
| 05_scene_planning.md:18:16 | deferred-with-owner | Error belongs to the named future operation, not one of the C3.2 commands. |
| 05_scene_planning.md:18:17 | deferred-with-owner | Error belongs to the named future operation, not one of the C3.2 commands. |
| 05_scene_planning.md:18:18 | deferred-with-owner | Error belongs to the named future operation, not one of the C3.2 commands. |
| 05_scene_planning.md:18:19 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:1 | deferred-with-owner | SI-097: skeleton transform only; bone kinematics excluded. |
| 05_scene_planning.md:19.1:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.1:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.2:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.2:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.2:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.2:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.2:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.2:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:1 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:2 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:3 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:4 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:5 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:6 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:7 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:8 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:9 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:10 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:11 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:12 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:13 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:14 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:15 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:16 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:17 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:18 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:19 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:20 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:21 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:22 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:23 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:24 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:25 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:26 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:27 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:28 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:29 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:30 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:31 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:32 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:33 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:34 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:35 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:36 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:37 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:38 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:39 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:40 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:41 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:42 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:43 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:44 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:45 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:46 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:47 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:48 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:49 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:50 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:51 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:52 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:53 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:54 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:55 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:56 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:57 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:58 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:59 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:60 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:61 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:62 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:63 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:64 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:65 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:66 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:67 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:68 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:69 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.3:70 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:71 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:72 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:73 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:74 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:75 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:76 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:77 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:78 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:79 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:80 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:81 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:82 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:83 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:84 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:85 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:86 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:87 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:88 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:89 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:90 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:91 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:92 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:93 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:94 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:95 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:96 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:97 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:98 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:99 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.3:100 | deferred-with-owner | Outside the 34-command C3.2 allocation; registry owner retained. |
| 05_scene_planning.md:19.4:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:10 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 05_scene_planning.md:19.4:11 | divergent | C32-R2-02: direct per-step scope union is correct; history replay omits anim.changed. |
| 10_data_model_project_format.md:4.6:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:7 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:8 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:9 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:10 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:11 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:12 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:13 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:14 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:15 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:16 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:17 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:18 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:19 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:20 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:21 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:22 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:23 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:24 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:25 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:26 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:27 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.6:28 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.12:1 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.12:2 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.12:3 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.12:4 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.12:5 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:4.12:6 | implemented | Numeric/command implementation supported by the executed SDK matrix, independent maths/query probes, and c32_semantics/contract/readiness. |
| 10_data_model_project_format.md:AG-02:extra | implemented | Round-1 divergence closed by response implementation; see round-2 finding closure table. |

