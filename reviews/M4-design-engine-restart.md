# M4 design note — the engine stops answering (C1.4 gap 3)

Status: decided before implementation (M4 C4.1). Owner: builder. Spec: 11 P5 (never block), 11 §14.6 (never-block
test), 01 §3.7 (recovery). Carried from M1 C1.2 / C1.4 ("restarting a document thread that never returns is the host's,
M4").

## 1. Problem

A handler that never returns cannot be preempted in-process (no safe thread termination, no `unsafe`). The executor
already answers every caller with `command.timeout` within its budget and serves again when the handler returns
(`c12_never_block`). If the handler never returns, every later call times out: the window stays responsive (calls run
off the main thread) but nothing works, and the open project's unsaved work is only in the stuck thread's memory and
in the last autosave.

## 2. Contract

- **Detection.** The host keeps a heartbeat: a cheap query (`app.state.get`) with a short budget, sent off the main
  thread every 5 s while no user call is outstanding. Three consecutive heartbeat timeouts (15 s of a busy document
  thread) mark the engine *not responding*; a successful answer clears the mark at once. The mark is shown in the
  status bar and the window title, and the front end offers **Wait** (keep watching) and **Restart the engine**.
- **Restart.** A new executor is spawned over the same user folders; the stuck thread is abandoned (its handle dropped;
  it ends if its handler ever returns). The new executor finds the autosave recovery data the old one wrote (01 §3.7)
  and the start screen lists it as recoverable work; the user chooses **Recover** (`project.recovery.open`) or
  **Discard**. What was not autosaved is lost, and the message says so with the autosave time. Agent sessions of the
  old executor end (their HTTP sessions are closed; clients reconnect). The front end resyncs from the new executor.
- **Never automatic.** Restarting discards in-memory work, so it is the user's choice; agents cannot trigger it.
- **No product hook.** The product has no way to make a handler hang; the behaviour is tested in an embedding test.

## 3. Alternatives considered

1. **Run each command in a killable process.** Rejected: the document lives in one process by design (ADR-0013 §3),
   and moving it out of process is an architecture change far beyond M4.
2. **Restart automatically after N timeouts.** Rejected: a long legitimate command (a huge vectorize) looks the same
   from outside, and an automatic restart would throw away work the user is waiting for.

## 4. Deterministic tests

- An executor embedding test (`crates/executor/tests/c41/restart.rs`) uses the dev-only registry of `c12_never_block`
  (a handler that never returns, released by the test at the end): heartbeats time out within their budget; a second
  executor over the same user folder starts, lists the recovery data the first one autosaved (forced maintenance before
  the hang), and opens it with `project.recovery.open`. Assertions are on outcomes and on budgets with a 3x margin, not
  on a sleep fitting a bound.
- Host logic (the three-strikes mark, the clear on success) is a pure state machine unit-tested in `tt-desktop`.
