from pathlib import Path
R=Path.cwd(); old=(R/'reviews/M1-C1.4-r3.md').read_text(encoding='utf-8')
ready=old[old.index('### Spec issues SI-001'):]
ready=ready.replace('SI-051â€™s representation now also needs the orchestratorâ€™s C14-R3-02 ruling.', 'SI-051 records the binding atomic-group ruling; its expectRev implementation remains open in this closure check.')
ready=ready.replace('M1 depends on replay semantics. C14-R3-01/02 block implementation closure; builder/orchestrator C1.4.', 'M1 depends on replay semantics. Commit accounting and atomic history pass; group expectRev remains C14-R3-02, builder C1.4 before merge.')
ready=ready.replace('The whole C14 ticket file also matches its introduction despite its later rename.', 'The C14 ticket bodies match their introduction despite the later rename; the effectiveParams ignore marker alone now names the real M4 C4.1 checkpoint.')
ready=ready.replace('the whole C14 ticket file also matches its introduction despite its later rename.', 'the C14 ticket bodies match their introduction despite the later rename; the effectiveParams ignore marker alone now names the real M4 C4.1 checkpoint.')
ready=ready.replace('| R3 replay and cancellation blockers | builder/orchestrator C1.4 scoped closure, before M1 merge |', '| Atomic group expectRev remains unsupported (C14-R3-02) | builder C1.4 before M1 merge; preserve passing replay/history and cancellation closures |')
ready=ready.replace('| Malformed summary typing and new lint allow | builder/orchestrator C1.4 P2 disposition before M2 C2.1 |\n','')
ready=ready.replace('builder C1.4 scoped publisher closure before M1 merge or explicit orchestrator test-coverage disposition; distinguish these from project-save journal recovery', 'builder M11 C11.1 under the recorded builder publisher-coverage disposition; not executed here; distinguish these from project-save journal recovery')
ready=ready.replace('Replay exported activity through actual script mode into the same initial document and compare canonical entries byte for byte.', 'Replay exported activity through actual script mode into the same initial document and compare canonical entries byte for byte, including revisions and undo/redo history. Preserve atomic group boundaries and test both matching and mismatching group expectRev after C14-R3-02 closes.')
ready=ready.replace('Prove both worker responsiveness and unchanged target bytes around the answered/cancelled boundary;', 'Prove both worker responsiveness and unchanged target bytes when cancellation wins before the confirmed-invocation commit point; after that commit point require the effect and its response to complete consistently;')
header='''# M1-C1.4 scoped closure and M1 merge gate

**Verdict: block.** One P1 remains in a ruled item: **C14-R3-02 does not implement atomic-group `expectRev` as ruled**. Do not merge or tag M1 yet. The other four ruled items close; no additional product regression was established. Product reviewed: `5c46aa61b55c3e75ac218306826ae7681d4da52a` (fixes `1ca13bb`, `347e34c`). This is a scoped closure check; accepted closures, deferrals and rulings are retained.

## Closure table

| Item | Result | Independent evidence |
|---|---|---|
| C14-R3-01 | closed | Internal commit facts precede publication and drive committedSteps. Unchanged replay suite passes; r4 sequential last-step publication failure after two edits audits error with all three committed steps and replays canonical entries exactly. |
| C14-R3-02 | **open, P1** | Two atomic batches plus a sequential batch replay byte for byte through four undo and four redo operations; group boundary and typed nesting refusal tests pass. However both matching and mismatching group expectRev are rejected as unknown fields. |
| C14-R3-03 | closed | Source has pre-ask and post-answer cancellation checks plus a lock-serialized begin_effect commit point. Unchanged late-accept regression passes 20/20; phase tests pass 3/3 and real-binary r3 suite passes 5/5. Supplemental ping/worker and pending-undecided token probes pass all four phase scenarios. Cancellation winning before begin_effect leaves the token unused and sends no dropped-request response; a later cancellation consistently loses to invocation. |
| C14-R3-04 | closed | Unchanged measurement reviewer suite rejects boolean/non-numeric/non-finite summaries and non-integer n; valid retained evidence remains accepted. |
| C14-R3-05 | closed | AuditCall removes the added allow. Recorder declarations gate runs the inventory checker; scratch-copy negative control adds an allow, prints NEW ALLOW and exits 1. Product tree was not modified for the control. |

## Remaining ruled finding

**C14-R4-01 / C14-R3-02 â€” P1, builder C1.4, before M1 merge.** ADR-0015 Â§13 explicitly gives group params `{label, steps, expectRev}` and says they include expectRev semantics; SI-051 records the same ruling. `group_step` deserializes params into `BatchRequest`, which has `deny_unknown_fields` and no expectRev member. Script loading accepts the generic params, then execution returns `batch.stepFailed` wrapping `arg.invalid: unknown field expectRev`. A matching current revision cannot execute; a mismatching revision never reaches revision-conflict checking. The document remains unchanged, but that safe refusal is not the ruled contract.

Reproduce: `cargo test -p tt-mcp --test c14_r4_closure --locked`. Two tests pass and two fail: matching expectRev must run; mismatching expectRev must refuse the entire group with `rev.conflict`. See [closure-final.log](evidence/M1-C1.4-r4/closure-final.log). Required closure is support and enforcement at the group transaction boundary, retaining the passing replay, nested-group, atomicity and history tests. Do not remove expectRev from the written ruling to fit the parser.

## Final register

Implemented / divergent / missing / deferred-with-owner:

- Requirements: **152 / 2 / 0 / 72** (226 units).
- Declarations: **21 / 0 / 0 / 11** (32 units).

[Full appendix](evidence/M1-C1.4-r4/conformance.md), [machine register](evidence/M1-C1.4-r4/conformance.json), [reproducer and ignored-body checks](evidence/M1-C1.4-r4/register.py). All four original closure rows and agent.activity.export promote for their passing audit, replay and cancellation contracts. S11-L81 (script mode) becomes divergent for the unfulfilled ruled group expectRev contract. S11-L96 / SI-049 remains the original spec-owner divergence. Thus the declaration proposal is confirmed, while requirements reach 153/1/0/72 only after R3-02 closes as ruled. This final unit-by-unit mapping supersedes the provisional register that retained audit/export rows merely because the broader finding remained open.

## Gates, CI and regression evidence

The builder receipt on `347e34c` has **18/18 exit zero**, all **21 SHA-256 hashes match**, and test totals **709/0/5**. This is authentic retained evidence, not a claim that it exercises expectRev. [Verification script](evidence/M1-C1.4-r4/verify.py) checks hashes, counts, protected paths, dependency policy and prohibited product constructs, then proves the allow negative control on a scratch copy.

[CI run 41070017095](https://github.com/kaigani/261017-toontown-dev/actions/runs/41070017095) is green on exact head `5c46aa6`, rechecked with gh. Retained API/log evidence verifies both Windows and macOS check jobs execute the runtime r1/r2/r3 suites (23/14/5), rather than relying on the earlier skipped Python gate. Official Python SDK reports 24 passed on Windows, macOS and headless Linux. Both bundles and all three raster jobs succeed. CI does not contain these new group expectRev probes.

Earlier reviewer-owned C0.xâ€“C1.4 tests, reports and evidence are unchanged from `b91a3c1` in the product diff. Builder test changes match the declared diff: new group tests, new cancellation phase tests, and only the effectiveParams ignore-marker owner text. Cargo.lock/deny.toml are unchanged; no new product unwrap/expect/panic/unsafe/allow is added. Regenerating 662 declarations leaves no diff. Release-feature graph proof passes for desktop and headless. IPC freshness is exercised by existing workspace tests. Targeted clippy for the new reviewer probes passes with -D warnings; reviewer code compiles and its failures are contract assertions, not compilation errors.

Local frontend: **333/333 in 40 files plus 9/9 tooling tests pass**. The first retained partial run exhausted disk; this run cleaned only this worktree's dev outputs and reran with debug symbols/incremental off. That interrupted evidence is not used as a product failure. The full local `cargo test --workspace --locked` run exits 101 solely at the retained r4 mismatch probe (2 pass / 1 fail in the probe version compiled when the sweep began). The final targeted probe version adds a matching-revision control and reports 2 pass / 2 fail. Because Cargo stops at that failing target, complementary package and MCP-target runs complete the coverage: all 104 original Cargo test targets from the builder receipt are covered, with identical pass/fail/ignore counts and no original-target failure (see workspace-coverage.json). The complementary workspace run exits 0 with 354/0/2; remaining MCP targets give 18/0/2; remaining doc tests exit 0. These overlap earlier targets and must not be summed as a new whole-workspace total. Both retained 10k MCP oracles pass. A green local merge gate is not claimed. The two C1.3 self-rewriting files are restored before commit. No product or spec files are edited.

## M1 merge readiness

The remaining merge gate is R3-02 expectRev closure and a final milestone receipt after integrating reviewer probes. Keep the supported older MCP baseline explicit under the binding SI-049 disposition; SDK success does not ratify the newer protocol target. No merge, tag or issue is created by this review.

The following final list preserves accepted owners and deferrals. It contains **51 spec issues (4 open), 5 ignored-test entries (2 M1 defect tickets, 2 operational scenarios and 1 pre-M1 ticket), 24 known-gap rows, and 12 M2 obligations**. Checkpoint references exist in plan Â§4. Publisher crash/staging/late-root/ACL cases remain explicitly not executed and assigned to M11 C11.1 under the recorded builder disposition; project-save recovery does not prove arbitrary-export recovery.

'''
footer='''
## Provenance

This check spans two delegated runs. The provider failure interrupted the first run after it committed `e64dde2` with closure probes and initial CI evidence. The resumed run inspected those artifacts, independently verified CI and receipt conclusions, reran closure probes and tests, and retained partial progress commits before finalizing. Raw initial disk-full output is retained as interrupted evidence, not a verified gate. No accepted closure or ruling is reopened.
'''
(R/'reviews/M1-C1.4-r4-closure.md').write_text(header+ready+footer,encoding='utf-8')

