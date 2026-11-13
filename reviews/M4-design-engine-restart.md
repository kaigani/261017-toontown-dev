# M4 design note — the engine stops answering (C1.4 gap 3)

Status: decided before implementation (M4 C4.1); amended before its code in C4.1 round 1 (ruling O-14). Owner: builder. Spec: 11 P5 (never block), 11 §14.6 (never-block
test), 01 §3.7 (recovery). Carried from M1 C1.2 / C1.4 ("restarting a document thread that never returns is the host's,
M4").

## 1. Problem

A handler that never returns cannot be preempted in-process (no safe thread termination, no `unsafe`). The executor
already answers every caller with `command.timeout` within its budget and serves again when the handler returns
(`c12_never_block`). If the handler never returns, every later call times out: the window stays responsive (calls run
off the main thread) but nothing works, and the open project's unsaved work is only in the stuck thread's memory and
in the last autosave.

## 2. Contract

- **Detection.** The host keeps a heartbeat: a cheap query (`app.state.get`) sent off the main thread on a fixed 5 s
  cadence measured with a monotonic clock (each tick is scheduled from the previous tick's due time, not from when the
  previous wait ended). A heartbeat still unanswered at a tick is one miss; while it stays unanswered no new one is
  sent (a stuck engine is not flooded) and every further tick is another miss. Three misses mark the engine *not
  responding*: about 15 s after the engine stopped answering (between 15 and 20 s, depending on where in the cadence
  it stopped). Any answer clears the mark at once, including one that arrives after its tick (a late reply). The
  heartbeat runs whether or not a user request is outstanding: the clause "while no user call is outstanding" of the
  first version is removed, because the hang this guards against is most likely a user's own command, which the
  clause would have hidden; a long legitimate command shows the mark and the user chooses Wait. The mark is shown as
  a banner offering **Wait** (keep watching) and **Restart the engine**.
- **Restart is a generation change (ruling O-14).** Restarts are serialised: a second request while one runs waits
  for it and answers with the new generation; it never starts another one or interleaves. The engine generation (0 at
  start, +1 per restart) travels with everything that crosses between the host and the window:
  - a request answers from the generation it started on; a request in flight when the engine is replaced is answered
    at once with `command.timeout`, `details.reason: "engineRestarted"` and the old and new generations (the code the
    spec defines for a call whose outcome is unknown, as for a caller-cancelled call, SI-111; SI-120 asks the spec
    owner to confirm); its late result, if the old engine ever returns, is never shown as applied;
  - cancellation flags of the old generation are dropped at the restart;
  - sensor reports carry the generation the window last saw; a report of another generation is ignored, and the
    window's modal, busy and text-focus facts start empty in the new engine (ruling O-9), so a window re-reports
    them after the restart;
  - engine events carry their generation; the host numbers each generation's events from 1, and the window resets
    its event numbering and resyncs on a new generation, so freshness (`renderedSeq`, C41-R1-14) compares numbers
    of one engine only.
- **Restart.** A new executor is spawned over the same user folders; then the stuck one is fenced and named
  (C41-R1-15: it writes no autosave, recovery data or heartbeat if it wakes up) and only its recovery data is
  released. The new executor offers that data on the start screen; the user chooses **Recover**
  (`project.recovery.open`) or **Discard**. The confirmation before the restart states the time of the last autosave
  the window saw (or that there is none), because what was not autosaved is lost. Agent sessions of the old executor
  end (their HTTP sessions are closed; clients reconnect). The front end resyncs from the new executor.
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
- Host logic (the three-strikes mark, the clear on any answer including a late one, the fixed cadence) is a pure
  state machine unit-tested in `tt-desktop` with an explicit clock; host tests cover a request in flight during a
  restart, two concurrent restarts, an old-generation report and old-generation cancellation flags.
