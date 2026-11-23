# M5 design note: a handler that panics (ruling E-26 item 4)

Builder: Claude Opus 5.5. Written after rulings E-21 and E-26 (C5.1a round 1), before any code for containment. This
note answers two questions: what happens today when a handler panics, and whether a panic should be contained at the
dispatch boundary. **No containment is built**; the orchestrator rules on §3 first.

Workspace facts: the panic strategy is the default, `unwind` (no `panic = "abort"` in any `Cargo.toml`); no crate sets a
panic hook; product code may not call `panic!`, `unwrap` or `expect` (the `clippy::panic`, `unwrap_used` and
`expect_used` lints deny them, `Cargo.toml` line 80), so a panic comes from a standard-library precondition (E-21's
`BTreeSet::range`, E-26's `BTreeMap::range`), an arithmetic overflow in a debug build, or a dependency.

## 1. What happens today

### 1.1 The executor (every host)

The document thread (`tt-document`, `crates/executor/src/lib.rs`) owns the session: the open project, its staged and
committed document, history, checkpoints, the application context, the clipboard, the Timeline state. Handlers run on
it inside `Core::invoke` (`run.rs`, `(command.handler)(cx, params)` at line 454). Prechecks are already contained
(`catch_unwind` at line 449, `internal: the precheck of ... failed unexpectedly`); handlers are not.

When a handler panics:

1. The panic unwinds the document thread to its top; the thread ends. The default hook prints
   `thread 'tt-document' panicked at <file>:<line>` to the process's stderr.
2. The call in flight: its reply channel is dropped, so the caller's `recv_timeout` sees `Disconnected` and answers
   **`internal`, "the document thread has stopped"** at once (`lib.rs` `stopped()`), not after the watchdog budget.
3. Every later call (any command, query, availability read, canvas read, gesture) answers the same `internal` error
   at once: the request channel's receiver is gone.
4. **State lost:** everything the session held in memory: the open project's unsaved changes, undo history,
   checkpoints, selection, clipboard, Timeline view and playback state, agent sessions. The staged commit of the
   panicking call never commits (the panic happens before acceptance), and nothing it staged reaches disk.
5. **State kept:** what is on disk: the project as last saved; the recovery data written by the last autosave
   (maintenance tick, 01 §3.7); user preferences already applied (the store writes on apply); the job table's
   finished outputs. The playback clock thread and the maintenance ticker end at their next send (their channel is
   closed).
6. **Nothing restarts inside the executor.** ADR-0013 decision 3 makes recovery an engine restart by the host.

`crates/executor/tests/c51_handler_panic.rs::e21_a_handler_panic_today_stops_the_document_thread` shows items 2, 3 and
6: a panic on the document thread inside `element.create` (the persist boundary hook stands for any handler panic, the
same device as `c34_round2::a_worker_that_really_dies_answers_internal`), the call answers `internal`, then
`timeline.rows.list` and `doc.state.get` answer `internal` "the document thread has stopped"; a new executor (what the
host's restart creates) has no project until one is opened or recovered.

### 1.2 The headless engine and the MCP server (`bins/headless`, `crates/mcp`)

The process keeps running: a thread's panic does not end a process under `unwind`. The MCP server stays connected and
answers every `tools/call` with the `internal` envelope; resources that read the session (`app://events` is the
server's own log and keeps serving) fail the same way. There is no host above the headless engine to restart it: the
client (or the person running it) must restart the process. The recovery data of the last autosave stays on disk and
is offered by `project.recovery.list` in the next process (the recovery design of `reviews/M4-design-engine-restart.md`
§2; this path is the same as after a crash, and is not re-tested here).

### 1.3 The desktop host (`desktop/src-tauri`)

ADR-0013 decision 3 and `reviews/M4-design-engine-restart.md` give the recovery: the watchdog marks the engine *not
responding* after three missed heartbeats, the window shows a banner with **Wait** and **Restart the engine**, the
restart spawns a new executor over the same user folders, fences the old one, and offers its recovery data
(**Recover** / **Discard**) with the time of the last autosave.

**That path does not start after a panic.** The watchdog (`watchdog.rs`) counts a *miss* only for a heartbeat still
unanswered at the next tick, and records an *answer* only for a heartbeat that answered `ok`. After a panic the
heartbeat (`app.state.get`) answers at once with `internal`: the in-flight slot is cleared, so the next tick sends a
new heartbeat instead of counting a miss, and no answer is recorded. The mark is never set, so **no banner appears and
the restart is not offered.** What a person sees: every action fails with an `internal` message on the status line
(D-87's one refusal path), menus and controls go disabled or stale (availability reads fail), the canvases keep their
last frame. The only way out is to quit the application; whether the next launch offers the recovery data is the
crash path of the recovery design (the old executor never removed its recovery data, because removal happens on the
document thread at close), not verified by a test in this round.

**Work lost** in the desktop host: everything since the last autosave (the recovery data) for the open project; and,
because no restart is offered, the person cannot save even work that a restart could have recovered from memory (none
can be: the session died with the thread).

## 2. What the E-21 and E-26 sweeps did instead

The two panics found in round 1 are fixed at their cause, and the sweep listed in `reviews/M5-C5.1a-r1-response.md`
(E-21) covers every site of the classes the ruling names in the files C5.1a added or changed. The argument sweep
(`crates/executor/tests/c51_argument_sweep.rs`) calls every M5 command with boundary arguments on three fixtures and
requires that the engine still answers afterwards. Those make a panic less likely; they do not make one harmless.

## 3. Containing a panic at the dispatch boundary: assessment

### 3.1 The design

Wrap the handler and its settle in `catch_unwind` inside `Core::invoke`, as the precheck already is:

```text
match catch_unwind(AssertUnwindSafe(|| { let r = (command.handler)(cx, params)?; cx.settle()?; Ok(r) })) {
    Ok(r) => r,
    Err(_) => Err(CommandError::internal("`<id>` failed unexpectedly; nothing was changed")),
}
```

A caught panic becomes an ordinary failed invocation: the staged document and the deferred effects are dropped, and the
`Rollback` taken before the call (`run.rs`, struct `Rollback`) is restored, as for any refusal. The caller gets
`internal` with the command id; nothing commits, nothing is written, the thread lives, and the next call is served.

A second, outer `catch_unwind` around each request the document thread serves (`document_thread`'s loop) would catch
panics outside an invocation: the timing-gesture interpreter's press and release (`timing_gesture::run`, which plans
before it invokes), `text_input`, `pointer`, the canvas reads, maintenance. Those run with no rollback of their own.

### 3.2 What a half-run handler could leave wrong

The `Rollback` restores what a handler may change in the session besides the staged document and its effects: the
application context, the UI state, checkpoints, the clipboard, the edit options, the agent hub's sessions and policy,
the Timeline state, and the event list (truncated to its length before the call). A panic caught at the inner boundary
leaves these right. What it does not cover:

- **Jobs started inside the handler.** A job command starts its worker in its handler (`jobs.rs`); a panic after the
  start leaves a running job whose command failed. Detection: the job table lists a job whose request answered
  `internal`; the containment would cancel jobs started by the failed call (the call's job ids are known to the job
  table).
- **Derived caches** (`crate::memo`, the font database `OnceLock`): a cache entry is written after its value is
  complete, so a panic leaves no entry rather than a wrong one; a `OnceLock` initialiser that panics stays
  uninitialised and runs again. No wrong state, by construction; a test can show it for the memo.
- **Shared locks.** Locks taken on the document thread are released by unwinding; every product lock is read with
  `unwrap_or_else(PoisonError::into_inner)`, so a poisoned lock does not cascade.
- **Agent records kept on purpose** (confirmation requests, the activity log): a failed call is recorded as failed,
  as for any refusal.
- **Panics caught at the outer boundary** (outside an invocation): the session may be half-changed (for example a
  grab inserted, or a text session moved), with no rollback. Detection: none in general; the outer catch must
  therefore treat the session as **degraded**: answer `internal`, emit a host-only notice, and have the host offer the
  restart (the recovery of §1.3).

### 3.3 The watchdog gap, independent of containment

Whatever the ruling on containment, the desktop host should treat a heartbeat answered with `internal` "the document
thread has stopped" as a dead engine: mark it *not responding* at once (or count it as a miss) so the banner offers the
restart and the recovery data. This is a host change of a few lines plus a unit test of the state machine
(`watchdog.rs` tests); it is not containment and does not touch the executor.

### 3.4 Costs and risks

- `catch_unwind` costs nothing on the normal path. `AssertUnwindSafe` is already the precheck's pattern.
- Containment hides a defect from the person but not from the reviewer: the `internal` message names the command, and
  the panic message still goes to stderr. A count of caught panics in `doc.state.get` (or a log line in the agent
  activity) keeps them visible in tests and in the field.
- A handler with an invariant broken mid-way in state the `Rollback` does not cover (§3.2) would continue with that
  state; the list above is complete for the session as it is today, and a new session field would have to join the
  `Rollback` (a test can enumerate the `Session` fields against it).

### 3.5 Recommendation

1. **Contain at the invocation boundary** (§3.1, inner): a caught panic is a failed call with `internal`, the staged
   commit discarded, the `Rollback` restored, jobs started by the call cancelled; a counter of caught panics reported
   in `doc.state.get`. Tests: a dev-only registry with a handler that panics before and after staging, and after
   starting a job (the device of `c12_never_block`), then the session and the document equal their state before the
   call and the next call is served.
2. **At the request boundary** (outer), catch, mark the engine degraded, answer `internal`, and have the host offer the
   restart: no attempt to continue a session whose state is unknown.
3. **Fix the watchdog gap** (§3.3) in any case, so that a dead or degraded engine leads to the restart banner and the
   recovery data, as ADR-0013 decision 3 intends.
4. Keep the sweeps (§2): containment is the last line, not the fix.

Until the ruling, nothing in this section is built.

## 4. Ruling E-29: what was built

The recommendation was approved with conditions (`reviews/M5-C5.1-plan.md` §15, E-29) and is built in C5.1a round 2.

### 4.1 The invocation boundary (item 1)

`run::run_step` runs the handler and its settle inside `catch_unwind`. A caught panic is the call's failure: `internal`,
"`<id>` failed unexpectedly; nothing was changed", `details: {commandId, reason: "handlerPanicked", panic}`, with a hint.
The caller's failure path (`Core::run_staged`, `Core::batch_atomic`) drops the staged transaction and the deferred
effects, restores the `Rollback`, and then `Core::contained` counts the panic, cancels the jobs the call started (job ids
are numbered: `JobTable::watermark` before the call, `cancel_since` after), writes a line to stderr and raises the
host-only `shell.handlerPanicked` notice (after the rollback, so it is the one event the call leaves). The count is read
with `ExecutorHandle::caught_panics`; it is not a member of any spec A result (D-57; the spec A addition is on the
user's list, `plan/spec-issues.md`). The person sees the failure once, on the status line (D-87's path,
`desktop/src/shell/refusals.ts`): "<command>: failed unexpectedly; nothing was changed"; no message box.

Tests (`crates/executor/src/panic_tests.rs`, a dev-only registry with a handler that panics where the call says):
`every_session_field_is_classified_against_the_rollback` (an exhaustive pattern over `Session`: a new field stops the
build with its name until classified); `a_panicking_handler_fails_its_call_and_changes_nothing` (before staging, after
staging, after starting a job, after changing each kind of state the `Rollback` covers: the document, history,
checkpoints, application context, UI, clipboard, edit options, agent sessions and policy, Timeline state and event list
equal their state before the call; the started job is cancelled; the next call is served);
`a_panicking_step_fails_its_atomic_batch_and_changes_nothing`. The claims of §3.2: `memo::tests::a_computation_that_panics_leaves_no_entry`,
`jobs::tests::a_poisoned_table_lock_is_still_served`.

### 4.2 The request boundary (item 2)

`document_thread` serves each request inside `catch_unwind`. A panic there marks the engine degraded
(`ExecutorHandle::degraded`): stderr, the host-only `shell.engineDegraded` notice once, and the thread leaves its loop
without another tick, so it writes no snapshot of a session it cannot vouch for (its last autosave stays on disk). Every
later call answers `internal`, "the document thread has stopped", `details.reason: "engineStopped"`, with the hint that
names the way out (restart the engine; then `project.recovery.list`), the same answer as a thread that died outright.

**What runs outside an invocation, and why it cannot use the inner boundary.** The inner boundary can contain a panic
because a call's effects are staged in its `Cx` and the `Rollback` restores the session; these requests change state
with no such staging, or after it:

| Request | What it does | Why not the inner boundary |
|---|---|---|
| `Invoke`, `Batch`: the request cache, `expectRev`, context repair, acceptance (`Core::accept`: commit, history, effects, revision log), settle and observation | the parts of a call around its handler | acceptance applies the transaction and its effects to the session and the project in place; a panic half-way leaves a half-committed state no `Rollback` describes |
| `Open` (`Core::load`) | installs a loaded project into the session | it replaces the session's project and context wholesale, with no prior state to roll back to |
| `WaitBegin`, `WaitFinish` (`job.wait`'s two halves) | the request cache and the waited job's envelope | bookkeeping of the request cache, outside any handler |
| `Snapshot`, `List` | a copy of the document; `list_commands` (enablement read on a lent session) | reads, but `List` takes the session out of the core while it runs |
| `Run` | work posted by job threads (installing a loaded project, finishing a save) and by the host: the timing-gesture interpreter (`timing_gesture::run` hit-tests and plans before it invokes), the Text tool's input path, the pointer gesture drivers, canvas reads, availability, key resolution, recovery release | each changes or reads session state directly (a grab inserted, a text session moved, a save's result installed) without a `Cx` of its own |
| `Tick`, `Maintenance` | autosave, lock and recovery heartbeats, external-change detection | writes to disk and to the session's persistence state |
| `PlaybackTick` | advances the playback clock and the current frame | changes the session's frame and playback state in place |

### 4.3 The watchdog (item 3) and headless engines (item 4)

The desktop watchdog marks the engine not responding at once when a heartbeat is answered with the stopped `internal`
(`Watchdog::stopped`; `watchdog::is_stopped`), so the banner offers Restart and the recovery data; a contained panic
(`handlerPanicked`) is one failed call and does not mark it. Unit tests: `a_stopped_engine_is_marked_at_once_and_a_real_answer_clears_it`,
`only_the_stopped_internal_counts_as_a_stopped_engine`. The live check (a dev-only hook kills the document thread, the
banner appears, Restart, Recover) joins the C5.1b rows and its manual script.

Headless and MCP: the stopped answer's hint tells an agent to restart the engine process, then call
`project.recovery.list`. `panic_tests::after_the_document_thread_died_a_new_executor_lists_the_last_autosave`: an
autosave is written, the document thread dies, a new executor over the same folders lists that recovery data (in one
process the dead engine is named first, as the desktop's restart does; a new process finds the old one gone). This was
the unverified path of §1.2.

### 4.4 The sweeps stay (item 5)

The argument sweep (`c51_argument_sweep`) still fails on a contained panic: no call may answer `internal` (nor
`handlerPanicked`), and each fixture's `caught_panics` must be 0 at the end.

### 4.5 What "nothing was changed" covers, and its limits (ruling E-33)

- **Jobs.** A contained panic cancels the jobs its call started that have not finished. A job that already ran to its end
  and published its result (its outputs written, `job.finished` raised) is **not reversed**: cancellation cannot undo a
  finished job, and a job past its commit point completes (`JobTable::cancel`). The caller then sees the call answered
  `internal` (`handlerPanicked`) and the job listed as finished with its result; the notice's `cancelledJobIds` names
  only the jobs whose cancellation was requested. Test: `panic_tests::a_job_that_finished_before_the_panic_is_not_reversed`.
- **The field tests.** `every_session_field_is_classified_against_the_rollback` names the session's top-level fields;
  `every_project_and_agent_hub_field_is_classified_against_the_rollback` names the fields of the open project and the
  agent hub, the two structures inside the session whose parts a handler reaches and the `Rollback` treats field by
  field. The other session values (application context, UI state, Timeline state, clipboard, edit options) are
  restored whole, as cloned values, so a field added inside them is covered by construction.
- **A retry with the same `requestId`.** The request cache keeps the first envelope of an id, whatever it was (C12-R3-03),
  so a retry after a contained panic is answered with the same `internal` failure, not run again. That is the cache's
  rule for every failure; a caller that wants the command run again sends a new `requestId`.
