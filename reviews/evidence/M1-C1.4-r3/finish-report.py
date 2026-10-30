"""Assemble the review's complete SI/ticket inventory and final merge brief."""
from pathlib import Path
HERE=Path(__file__).resolve().parent
report=HERE.parents[1]/'M1-C1.4-r3.md'
text=report.read_text(encoding='utf-8')
text=text.replace('replay has 1 passing control and 2 failing cases','replay has 1 passing control and 4 failing cases')
text=text.replace('decide a replay representation that preserves commit and history boundaries, then test atomic batch → undo/redo and checkpoints as well as final metadata.', 'decide a replay representation that preserves commit and history boundaries. The additional `undo_after_an_atomic_batch_replays_the_same_document` and `checkpoint_restore_after_an_atomic_batch_replays_the_same_document` regressions also fail; redo remains a required closure variant.')
if '\n## M1 merge readiness\n' in text:
    text=text.split('\n## M1 merge readiness\n')[0]
text+='''
## M1 merge readiness

**Do not merge `m1-core-kernel` into `main` or tag `m1` yet.** Under plan §5 this third full round ends in a scoped closure/ruling phase: close R3-01/02/03 with the retained tests, dispose of P2 R3-04/05 explicitly, and record the accepted SI-049 protocol target. Merge the reviewer tests/evidence into the milestone branch before rerunning the gates; a green pre-review CI run is not the final merge receipt.

The orchestrator's merge checklist is:

1. Builder fixes preserve the full integrity gate, one revision per atomic commit, unchanged successful cached replies, truthful error audit status and every actual committed edit in replay. Scoped replay closure must cover publication failure, successful multi-edit atomic batches, undo/redo and checkpoint references on the same initial saved document.
2. Scoped cancellation closure covers before sending, waiting without an answer, cancellation before a late answer, and answer delivery before confirmed invocation; the worker releases, no dropped-request response is sent, and a cancelled confirmation authorises no action.
3. Record an explicit result for the remaining crash/staging/late-root/ACL matrix limitations above, rather than calling them executed. Keep no new product unsafe/panic/unwrap/expect/allow under the standing rule unless the orchestrator explicitly rules on R3-05.
4. Keep both C14-R1-16 tickets and their real deadlines. Executed kernel examples are due **before M2 C2.1**, including results and a schema gate; effective parameters belong to **M4 C4.1**. SI-044, SI-047 and SI-049 need the owner dispositions below; do not silently change the frozen spec.
5. Rerun the affected reviewer targets, frontend/generated-contract gates when touched, release-feature proof and the complete workspace after fixes; obtain green Windows/macOS check jobs and the Linux SDK job on the final milestone head. Restore the two self-rewriting C1.3 evidence files. Only then merge to main and tag m1 under the existing release process. This reviewer did not merge, tag or open issues.

'''
inventory=(HERE/'readiness.md').read_text(encoding='utf-8').replace('## Spec issues','### Spec issues').replace('## Ignored tests','### Ignored tests')
text+=inventory
text+='''
There are **two defect tickets carried by ignored tests introduced in C1.1–C1.4**, both C14-R1-16. No C1.1–C1.3 defect ticket remains hidden in an ignored body. The ZIP64 scenario and crash child are operational ignores, and the SIMD defect ticket predates M1. All five function bodies match their introduction commits; the whole C14 ticket file also matches its introduction despite its later rename. The crash child runs via its passing parent. The 4 GiB release scenario was not rerun in this round; its ignore does not mean it failed. The C0.4 SIMD ticket stays due M3 C3.3.

### Known gaps with owners

| Gap / accepted limit | Owner and completion boundary |
|---|---|
| R3 replay and cancellation blockers | builder/orchestrator C1.4 scoped closure, before M1 merge |
| Malformed summary typing and new lint allow | builder/orchestrator C1.4 P2 disposition before M2 C2.1 |
| Protocol revision conflict SI-049 | spec owner; orchestrator must record accepted M1 baseline, builder M4 C4.1 for an authorised protocol/attached-host change |
| Save-time hashing cost and size+mtime watch limit SI-047 | spec owner; interim content-at-save rule binding now, builder M4 C4.1 policy/UX follow-up |
| Repair base cannot undo into invalid model SI-044 | spec owner; builder M1 C1.3 decision stands and integrity must remain enforced |
| Effective parameters; worked examples | builder M4 C4.1; builder before M2 C2.1 respectively, retained ignored tests |
| Attached HTTP, Origin/bearer authentication, multi-client sessions, discovery, indicator, in-app confirmations | builder M4 C4.1, with coexistence/parity in C4.3; do not advertise attached support until exercised |
| Non-returning document handler leaves its worker occupied after caller timeout | builder M4 C4.1 restart policy; timeout remains indeterminate and is not thread cancellation |
| Virtual views, pointer/key/text, inspect UI, screenshots | builder M4 C4.2; drawing structured/gesture relationships M2 C2.3 |
| Renderer/frame/hit/coordinate/contact-sheet resources and computed summaries | builder M3 C3.3; drawing rendering M2 C2.3; no fake successful empty outputs |
| Structural commands before derived geometry is ready; strict placeholder schemas | each domain checkpoint; M2 C2.1/C2.2/C2.3 for strokes/text/zones/drawing operations, M3 C3.1/C3.2 for element/animation work; replace the relevant SI placeholders before enabling a handler |
| Multi-second 100k-stroke transaction cost and coarse whole-entry staging | performance/executor builder M2 C2.1; retain full validation/inverses while measuring any node-granular improvement |
| Unchecked model assembly confinement | model/persistence builder M2 C2.1 must retain the accepted candidate-load/repair-only restriction; new drawing constructors must pass the model integrity gate |
| Colour recovery, zones and unresolved colour reporting | builder M2 C2.2; keep colour.unresolved until recovery is implemented |
| missingFonts lint computation | builder M2 C2.1; replace unavailable with actual font results when text is implemented |
| Other unavailable lint computations | builder M8 C8.2 staleLipSync, M9 C9.2 transitionPadding, M9 C9.1 outdatedTemplateLinks |
| Derived thumbnails | builder M3 C3.3 generation; continue preserving existing bytes now |
| Global library persistence and locks | builder M9 C9.1; reuse the single arbiter ownership contract |
| First actual network permission/confirmation test | builder M8 C8.2 for model download, or whichever real network command ships first; Assist must deny before outbound I/O |
| Windows atomic rename without write-through; power-loss guarantees | persistence/hardening builder M11 C11.1; document existing ADR-0014 limit, test any stronger safe implementation before claiming durability |
| SMB/NFS, multi-host file ownership and remote lock semantics | persistence/hardening builder M11 C11.1; local NTFS/APFS and kernel-held-lock evidence are not network certification |
| Arbitrary export crash between staging/rename, late root changes, ACL publication variants | builder C1.4 scoped publisher closure before M1 merge or explicit orchestrator test-coverage disposition; distinguish these from project-save journal recovery |
| Temp/cache cleanup after failing tests and local build disk pressure | builder M11 C11.1 test hygiene; use isolated paths and clean only this worktree when needed |
| Final renderer scalar/SIMD and u8 opacity limitation | builder M3 C3.3; preserve the fixed-configuration golden corpus and verify final pixels before enabling dispatch |
| Real tablet/device and macOS manual QA; media source-duration and encoder quality decisions | manual-QA/domain owners M4 C4.2, M8 C8.1/C8.3, M10 C10.1/C10.3, M11 C11.1; CI runners are not a substitute for device evidence |

### Testable obligations for the M2 C2.1 brief

1. **Before starting C2.1**, execute and retain worked request/result examples for project, doc, edit, checkpoint, job, agent and app; make the retained namespace-doc ticket pass without weakening its body. Schema-check the successful results on both OSes.
2. Add strokes/text/bitmap constructors only through the registry and serialized executor. With a saved/reopened fixture, prove atomic rollback after a later invalid geometry/reference step, one revision and one undo step for a successful atomic batch, exact inverse/redo, dry-run pre-state preservation and revision-exhaustion refusal.
3. Preserve opaque meta/ext data, concrete-index inverse behavior, stable object IDs and drawing-content revision propagation. New constructors must pass the full model gate; unchecked assembly stays confined to candidate load/repair. Keep the retained old structural-command readiness split until the derived computation exists.
4. Measure the 100k-stroke edit/undo/dry-run path from observations, reporting wall time and allocation/storage growth. Improve coarse staging where needed without narrowing the integrity gate or excluding references/semantic validation. Re-run both seeded history oracles through MCP on saved/reopened geometry fixtures.
5. For every new file-producing drawing/media command, use the common staged effect, exercise later-step rollback, failed publication and its audit/commit accounting, and preserve targets before commit. Include generic, typed, batch and script entry points, bounded scope checks and no outbound effect before permission/confirmation.
6. After C1.4 closure, retain the new replay tests unchanged and extend them with real drawing edits: atomic multi-edit transaction, undo/redo, checkpoint restore, partial sequential failure, cached requestId retry, dry run, discovery and publication failure. Replay exported activity through actual script mode into the same initial document and compare canonical entries byte for byte.
7. New confirmations must not authorise a cancelled call, consume a late answer, or respond to a dropped request. Prove both worker responsiveness and unchanged target bytes around the answered/cancelled boundary; keep Assist overwrite and malformed-answer regressions.
8. Replace each draw/text command's SI-tagged placeholder shape before its handler is enabled; reject malformed/unknown parameters and validate success outputs/examples. Query results must satisfy the model schema and explicit target contract, including wide integer revisions through IPC.
9. Seed and record any stroke/brush randomness; compare command-created geometry and canonical state on Windows/macOS and across headless runs. Rendering and gesture equivalence keep their actual M2 C2.3/M3/M4 owners rather than being inferred from metadata determinism.
10. Implement the C2.1 font-lint obligation for text and preserve missing/unknown colour reporting until C2.2 recovery. Do not install derived fields that their owning computation has not validated; preserve imported unknown extension content through save/reopen.
11. Maintain bounded frame/payload/cache/history behavior and autosave responsiveness while geometry work runs. A caller timeout does not terminate a non-returning worker; retain the M4 C4.1 restart obligation and report indeterminate outcomes honestly.
12. Keep release graphs free of test-hooks, product code free of the prohibited constructs/suppressions, and all prior reviewer tests unchanged unless an orchestrator ruling explicitly authorises and declares an edit. Finish C2.1 with current generated declarations/IPC types and Windows/macOS CI evidence.
'''
report.write_text(text,encoding='utf-8')
