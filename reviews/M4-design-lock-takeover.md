# M4 design note: lock takeover (ruling D-78 item 1; finding C44B-R1S-07)

The project lock (`<package>.lock`, 10 §8.6, LOCK-01 to LOCK-03) and its arbitration file (`<lock>.takeover`), in
`crates/persist/src/lock.rs`. Written under the GUI hold, before the code; round 2 reviews both.

## 1. What went wrong

Two defects, both pre-existing since M1, both from removing a file **by name** on the strength of a decision made
earlier, after the protection that justified it was gone or was never re-checked:

- **Zero owners** (the C4.4b handoff §6). `Arbiter::drop` closed its arbitration file (releasing its protection) and
  then removed it by name. A reaper that judged the now-unprotected file abandoned removed it; a third contender
  created its own; the first's delayed removal deleted the third's. The third published its lock, found its arbiter
  gone (`still_ours` false), returned `HeldByOther`, and nobody owned the lock.
- **Two owners** (the reviewer's Unix schedule). Two reapers probed one abandoned arbitration file as free; one removed
  it; contender D created a fresh one, decided the project lock stale and paused; the other reaper's delayed removal
  unlinked D's fresh arbiter; W took a new arbiter and acquired the lock properly; D resumed and **unlinked W's
  protected lock by name** (Unix lets a locked file be unlinked: `flock` protects an inode, not a pathname), published a
  replacement, failed its read-back, closed it; C took that unprotected replacement over while W still held its handle.

## 2. Invariants

- **I1. Never two owners**, in any interleaving of contenders, reapers, heartbeats and releases: at most one
  `Acquired` handle whose file is the lock's current file.
- **I2. A protected live lock is never removed by a taker**, and a protected live arbitration file never by a reaper:
  only its owner removes its own file, while still protecting it.
- **I3. A stale lock is taken by exactly one contender** when any contender stays alive (no "everyone told held").

## 3. Primitives

- **Protection** of a file by its owner: on Windows an open handle with write access (shared for reading, writing and
  deletion); elsewhere an exclusive `flock` on the file's inode. The kernel drops it when the owner's process ends.
- **Proof of removal** by a non-owner, `remove_proven`: the contender takes the file's own protection, re-validates
  under it that the file is the very one its decision was about, and removes exactly that file:
  - **Unix.** Open the path; take an exclusive, non-blocking `flock` on the opened file (refused while the owner, a
    probe or another prover holds a lock on it: the file is protected, nothing is removed). For a stale lock (A4) a
    probe's momentary shared lock is waited out for up to 200 ms, so another contender's `state()` cannot make a dead
    lock look protected; for an arbitration file (A2) the contender's own retry loop does the same. Then, **after** the pause
    point and immediately before removing: the opened file's device and inode equal the path's (identity, not
    pathname), and the bytes read **through the opened file** satisfy the decision (the nonce it was made about: the
    lock's `lockId`; for an arbitration file, a written token or an age past `TAKEOVER_STALE`). Unlink by name while the
    `flock` is held; then close. Nobody can replace the path in between: replacing it needs its removal, and every
    removal needs this same `flock` or is the owner's own (and the owner holds the `flock` until after its unlink).
  - **Windows.** Open the path with read and delete access, sharing reading and deletion but **not writing**, with
    `FILE_FLAG_DELETE_ON_CLOSE`. The open fails (`ERROR_SHARING_VIOLATION`) while any handle can write the file, so
    it succeeds only when no live owner protects it; a delete-pending file (another prover closing) refuses the open
    too, which counts as contention. The deletion is bound to the opened handle: closing it deletes **that file**, never
    a file created at the path later (none can be created while it exists). The bytes are read through the handle and
    the decision re-validated after the pause point; a file that fails it is still a file no live owner holds, so it is
    dead either way, and the contender re-decides from the start.
  - **A file system that cannot lock** (Unix `flock` refused with an error; Windows refusing the sharing mode with
    `ERROR_INVALID_FUNCTION` or `ERROR_NOT_SUPPORTED`): protection cannot be proven; the age rule
    of M1 stays for arbitration files, and a stale lock is removed under the arbiter as before. Stated as the limit of
    these guarantees (network file systems without lock support).
- **Own removal** by the owner, `remove_own`: the owner verifies its nonce (the arbiter's token, the lock's `lockId`)
  and, on Unix, that the path still names its inode; it removes the file **while still protecting it** (Unix: unlink
  while holding the `flock`; Windows: delete while its handle, which shares deletion, is open; a legacy delete-pending
  file then refuses new creations until the handle closes, which contenders treat as contention), and only then closes.

## 4. The protocol, step by step

Every step names what the contender has proved and under which protection.

| Step | Who | Proved, under which protection |
|---|---|---|
| A1 take the arbiter | contender | `create_new` of `<lock>.takeover` succeeded: the file is new and its own; protection taken (Unix: `flock`, retried while a probe's momentary shared lock holds; given up after 200 ms with an **own removal**, identity checked). Token written after the protection. |
| A2 reap an abandoned arbiter | contender whose `create_new` failed | **proof of removal**: no live arbiter protects the file; under its protection, the token is written or the file is older than `TAKEOVER_STALE`; identity re-checked after the pause point (`ArbiterAbandonedProven`). Never removes a protected arbiter (I2). |
| A3 read the lock's state | arbiter owner | `state()` as before (probe, marker, heartbeat): a decision, not yet an act. |
| A4 take over a stale lock | arbiter owner | the decision (`TakeoverDecided`, the pause point of the reviewer's schedule), then **proof of removal** of the lock file: refused if its owner protects it (`HeldByOther`, I2: a live lock published meanwhile by anyone is protected by its owner's handle); else, after the second pause point (`TakeoverProven`), identity and the decided `lockId` (or still unreadable, for an abandoned unreadable lock) re-checked through the opened file before removal. A file that changed: re-decide from A3. |
| A5 publish | arbiter owner | `create_new` of the lock file: no lock exists; protection taken; content written in place through the protected handle. |
| A6 validate and announce | arbiter owner | the lock read back holds its `lockId` **and** its arbiter is still its own (`still_ours`, the token nonce). With A2 and A7, an arbiter can no longer be removed under its owner, so this holds unless the file system broke the primitives. If it fails, `HeldByOther` as before and the publication is not removed: a removal needs a valid arbiter (the review's constraint (d)), and the closed publication is then an ordinary stale lock for A4. |
| A7 release the arbiter | arbiter owner | **own removal**: token verified, Unix identity verified, removed while protected (`ArbiterReleasing`), then closed. |
| H heartbeat | lock owner | as before: under the arbiter, fails closed if the lock is not its own; writes in place through its handle. Never removes. |
| R release the lock | lock owner | under the arbiter: `lockId` verified (`ReleaseChecked`), **own removal** of the lock file while its handle is open, then the handle closed. |

## 5. Why the two schedules cannot happen

- **Zero owners:** A7 removes the arbiter while protected, so no reaper ever sees a live arbiter unprotected (A2 refuses
  a protected file), and no delayed removal by name exists anywhere: every removal is A2's or A4's proof (bound to the
  opened file's identity) or the owner's own removal (protected, nonce and identity checked).
- **Two owners:** D's arbiter cannot be removed under it (A2 refuses it, A7 is D's own), so W cannot take an arbiter
  while D holds one and cannot publish. Even if the file system removed D's arbiter behind its back, D's delayed act
  is A4's proof on the lock file: W's published lock is protected by W's handle, so D's proof is refused and D returns
  `HeldByOther`; and on Unix D's identity check would fail anyway (W's file is a different inode with a different
  `lockId`). D never removes a file it has not just proved, after its pause, to be the very one it decided about.

## 6. Tests (round 2 runs them after the hold)

- **Deterministic, paused** (`crates/executor/tests/c44b_lock_takeover.rs`, pause points only under the existing
  `test-hooks` feature):
  1. a releasing arbiter paused at `ArbiterReleasing` keeps its file protected; a contender removes nothing and then
     owns the lock;
  2. an abandoned arbiter proven by one reaper and paused at `ArbiterAbandonedProven`: no lock is published meanwhile;
     exactly one owner after;
  3. **the zero-owner schedule:** reaper R paused at `ArbiterAbandonedProven` holding its proof; the file is replaced
     behind it (the other reaper's removal, done by the test) and contender D takes a fresh arbiter and pauses at
     `TakeoverDecided`; R resumes: its identity check refuses (Unix) or its handle deletes only the old file
     (Windows), so D's arbiter survives; exactly one owner;
  4. **the two-owner schedule:** D paused at `TakeoverDecided`; D's arbiter removed behind it (by the test: the
     reviewer's delayed removal); W acquires; D resumes: its proof of the lock file is refused while W protects it; at
     every moment at most one `Acquired`, and W's handle stays valid (`held()`).
- **Randomised:** many contenders with jitter against a stale lock, repeated trials: exactly one `Acquired` per trial,
  and that owner's handle stays valid after every other contender returned.
- **The reviewer's** `c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner`, unchanged, in a loop (at
  least 200 runs of its 64 trials on Windows; CI's macOS `check` runs it once per run), and the existing lock tests
  (`lock.rs` unit tests, `c13_r4_lock.rs`, `c14_lock_races.rs`).

## 7. Constraints of the review, and where each is met

- Deleted while its protection is still held: A7, R and A5's failed write (own removal). A1's give-up (Unix only: the
  `flock` was never obtained) removes its own just-created file after checking that the path still names its inode;
  the file is empty and younger than `TAKEOVER_STALE`, which no reaper removes.
- Every removal re-validates, under protection, the very file the decision was made about (identity or nonce): A2, A4
  (proof of removal); own removals check the nonce and, on Unix, the inode.
- A decision made before a pause is never acted on after it without re-validation: A4's decision (`TakeoverDecided`)
  is followed by the proof and, after `TakeoverProven`, the identity and nonce check; A2 likewise.
- Retries are not the fix: the loop that re-decides on a changed file is the protocol's normal path, not a cure for a
  removal that already happened; no removal can be wrong in the first place.
