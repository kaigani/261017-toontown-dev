# M4 design note: lock takeover (ruling D-78 item 1; finding C44B-R1S-07; rulings D-83 and D-84, findings C44B-R2S-01 and C44B-R2L-01)

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

- **Zero owners again, by creation (round 2, C44B-R2S-01; Unix).** A's `create_new` of the arbitration file succeeded
  and A was suspended before its `flock`. Past `TAKEOVER_STALE`, R opened that empty file, locked it, proved it by
  age and unlinked it. A resumed, locked its now **unlinked** inode, wrote its token, took the arbiter role, published
  the lock; R saw the protected lock and returned `HeldByOther`; A's `still_ours` found no pathname for its arbiter
  and returned `HeldByOther` too. The flaw: A trusted a handle the path no longer named. Ruling D-83 item 1: **a
  handle is trusted only while the path still names it** (C1, C2 below).

- **No owner on Windows (round 2, C44B-R2L-01).** An opener proved the stale lock (delete-on-close) while another
  program held the file open for reading, sharing deletion (another opener's probe, a scanner, an indexer, a backup):
  Windows deferred the deletion, the name stayed delete pending, the opener's `create_new` got access denied and it
  returned `NotWritable`; a second opener returned `HeldByOther`. Zero owners, and a false "not writable". Ruling D-84.

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
| C1 create a protected file (A1's and A5's creation) | contender | `create_new` succeeded (`CreatedUnprotected`); Windows: the creating handle can write, so the file is protected from the moment it exists. Unix: `flock` taken after, retried while a probe's momentary shared lock holds; **given up after 200 ms with nothing removed** (another opener, a prover, holds a lock on it: a removal by name without the protection could land on a successor's file once the prover removed this one; the empty, unprotected file is removed later by a proof, once stale). On a file system that cannot lock, the M1 rule: removed while the path still names it. |
| C2 confirm the identity under the protection (D-83 item 1) | contender | Unix: with the `flock` held, the path's device and inode equal the opened file's. If not, a prover removed the file between C1's creation and the lock: the handle protects a file no path names and confers nothing; it is closed and the attempt is **contention** (`AlreadyExists`): the caller decides again from the start. From C2 on nobody can remove the file (a proof needs this `flock`; an own removal is the owner's). Windows: nothing to confirm (C1). |
| A1 take the arbiter | contender | C1 and C2 for `<lock>.takeover`: the file is new, its own and named by its path; token written after the protection. An attempt that fails is observable (`ArbiterNotTaken`). |
| A2 reap an abandoned arbiter | contender whose `create_new` failed | **proof of removal**: no live arbiter protects the file; under its protection, the token is written or the file is older than `TAKEOVER_STALE`; identity re-checked after the pause point (`ArbiterAbandonedProven`). Never removes a protected arbiter (I2). |
| A3 read the lock's state | arbiter owner | `state()` as before (probe, marker, heartbeat): a decision, not yet an act. |
| A4' free the name (D-84, Windows) | arbiter owner, inside A4's proof | after the re-validation, the proven lock file is **renamed aside** (`<lock>.removed-<token>`) while the proof's handle protects it; the delete-on-close removes it under that name when the last handle closes. So the lock's name is free at once whatever other programs hold the old file open (sharing deletion). Safe by name because A4 runs under the arbiter: one prover of the lock file at a time, and nobody else creates at that name meanwhile. A failed rename leaves the name pending: contention (below). Not used for A2: several contenders can hold proofs of one arbitration file, and a late rename could move a successor's file; there a pending deletion is contention, re-decided. |
| A4 take over a stale lock | arbiter owner | the decision (`TakeoverDecided`, the pause point of the reviewer's schedule), then **proof of removal** of the lock file: refused if its owner protects it (`HeldByOther`, I2: a live lock published meanwhile by anyone is protected by its owner's handle); else, after the second pause point (`TakeoverProven`), identity and the decided `lockId` (or still unreadable, for an abandoned unreadable lock) re-checked through the opened file before removal. A file that changed: re-decide from A3. |
| A5 publish | arbiter owner | C1 and C2 for the lock file: no lock exists; protection taken; identity confirmed; content written in place through the protected handle. |
| A6 validate and announce | arbiter owner | the path still names the protected lock file (identity), the lock read back holds its `lockId` **and** its arbiter is still its own (`still_ours`: the path names the arbiter's protected file, and it holds the token nonce). With A2 and A7, an arbiter can no longer be removed under its owner, so this holds unless the file system broke the primitives. If it fails, `HeldByOther` as before and the publication is not removed: a removal needs a valid arbiter (the review's constraint (d)), and the closed publication is then an ordinary stale lock for A4. |
| A7 release the arbiter | arbiter owner | **own removal**: token verified, Unix identity verified, removed while protected (`ArbiterReleasing`), then closed. |
| H heartbeat | lock owner | as before: under the arbiter, fails closed if the lock is not its own; writes in place through its handle. Never removes. |
| R release the lock | lock owner | under the arbiter: `lockId` verified (`ReleaseChecked`), **own removal** of the lock file while its handle is open, then the handle closed. |

## 4.1 Contention, not "not writable"; "held" only when true (ruling D-84)

- **Contention** (decided again from the start, A3): an existing file; on Windows a sharing violation or a name whose
  deletion is pending (reported as access denied, or `ERROR_DELETE_PENDING`); any permission error while the folder
  can be written (a uniquely named file is created there and removed). `NotWritable` only when the folder cannot be
  written, or the lock cannot be written through its own protected handle.
- **A refused proof** (A4) answers `HeldByOther` only when a live owner protects the lock at that moment (`probe`);
  a foreign handle that refuses sharing deletion is contention.
- **An unconfirmed publication** (A6 fails) is closed, not removed (a removal needs a valid arbiter) and the opener
  decides again: a protected publication of this host that nobody protects is stale at once, so the next round takes
  it over (and does not report the opener's own publication as a stale lock it took over).
- **The bound.** `acquire` decides in rounds of 2 s (`DECIDE_BOUND`). At the end of a round it answers what is true at
  that moment: the lock held (a live owner, or a lock the protocol counts as held) or the arbiter held by another live
  opener: `HeldByOther`; the folder not writable: `NotWritable`. When nothing is held (a removed file's name kept by
  another program's handle; an arbitration file left empty by a creator that gave up, which becomes removable once
  older than `TAKEOVER_STALE`), it decides for another round. I3 therefore holds on Windows with other handles open on
  both files: the lock file's name is freed at once (A4'), and the arbitration file's pending deletion ends when the
  other handle closes, after which the next attempt takes it.

## 5. Why the schedules cannot happen

- **Zero owners:** A7 removes the arbiter while protected, so no reaper ever sees a live arbiter unprotected (A2 refuses
  a protected file), and no delayed removal by name exists anywhere: every removal is A2's or A4's proof (bound to the
  opened file's identity) or the owner's own removal (protected, nonce and identity checked).
- **Two owners:** D's arbiter cannot be removed under it (A2 refuses it, A7 is D's own), so W cannot take an arbiter
  while D holds one and cannot publish. Even if the file system removed D's arbiter behind its back, D's delayed act
  is A4's proof on the lock file: W's published lock is protected by W's handle, so D's proof is refused and D returns
  `HeldByOther`; and on Unix D's identity check would fail anyway (W's file is a different inode with a different
  `lockId`). D never removes a file it has not just proved, after its pause, to be the very one it decided about.
- **Zero owners by creation (C44B-R2S-01):** after R unlinks A's empty, unprotected file, A's C2 finds that the path no
  longer names the inode it locked: A closes it and its attempt is contention (`ArbiterNotTaken`); A never writes a
  token into it, never takes the arbiter role and never publishes. R (or anyone) creates a fresh arbitration file and
  publishes; A's next attempt sees the live lock. If R had not yet unlinked when A locked, R's proof needed A's `flock`
  and is refused; if R held the `flock` when A tried, A gives up without removing anything (C1), and R's proof then
  removes exactly the file it opened (identity re-checked under its `flock`). On Windows A's handle protects the file
  from its creation, so R's proof is refused. In every order, the file that confers the arbiter role (and the
  published lock, A6) is one the path names while its owner protects it, and nobody else can remove it.
- **A creator's cleanup racing a prover:** C1 never removes a file whose protection someone else holds; the only
  remaining removal by name without a `flock` is on a file system that cannot lock, where no prover can hold one.
- **Windows: two kinds of removal (corrected in round 3, after the hold).** An owner's own removal is
  `std::fs::remove_file`, which on NTFS uses POSIX delete semantics: the name is freed at once even while the owner's
  handle (which shares deletion) is open. A proof's removal is its delete-on-close handle: when that handle closes while
  another handle is open, the file stays under its name, delete pending, and opens and `create_new` there fail with
  access denied until the last handle closes (probed with a small script before the test was written). The zero-owner
  and two-owner schedules therefore run on Windows as on Unix (a removal by name frees the name), and the
  delete-pending case is tested on its own (§6): a contender's attempts end in `ArbiterNotTaken` until the old handle
  closes; the paused holder, whose arbiter is no longer its own, closes its first publication, decides again (D-84) and
  owns the lock.

## 6. Tests (round 2 runs them after the hold; rewritten for ruling D-83 item 2)

- **Deterministic, by participant** (`crates/executor/tests/c44b_lock_takeover.rs`, pause points only under the
  existing `test-hooks` feature). Each participant is a named thread; the hook records every boundary each one reaches
  and suspends exactly the participant the test armed at that boundary, which the test then resumes by name. No step
  is ordered by time: the test waits for states (a participant held at a boundary, a thread returned). Round 2's
  tests, as round 2 had them, are below; their D-83 forms are in the test file's header:
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
- **Added by D-84:** the reviewer's reproduction (an opener held at `TakeoverProven`, a foreign reader sharing
  deletion, resumed: the opener owns the lock, a second opener is told it is held, nobody is told "not writable"); on
  Windows a reader refusing deletion (the opener decides again rather than answering "held"); a folder that cannot be
  written is `NotWritable` (Unix); the randomised test prints its seed, and its loop of at least 2,000 runs on Windows
  sets a seed per run (`TT_LOCK_SEED`).
- **Added by D-83 item 2:** the creation before the protection (A held at `CreatedUnprotected`; R's proof removes the
  aged file on Unix and is refused on Windows; A's attempt then ends in `ArbiterNotTaken` on Unix; exactly one owner);
  a creator's give-up racing a prover (Unix: the prover holds the `flock`, the creator gives up and removes nothing);
  the Windows delete-pending case on its own (a contender's attempt ends in `ArbiterNotTaken` while the old handle
  lives; exactly one owner after); and a participant **ending** at each pause point (a child process exits at the
  boundary, so the system closes its handles and nothing of its own cleanup runs; the files it leaves are aged past
  `TAKEOVER_STALE`, standing for the wait, and the next open owns the lock with nothing left behind after release).
- **Randomised:** many contenders with jitter against a stale lock, repeated trials: exactly one `Acquired` per trial,
  and that owner's handle stays valid after every other contender returned.
- **The reviewer's** `c13_r1_persistence::concurrent_stale_lock_takeover_has_only_one_owner`, unchanged, in a loop (at
  least 200 runs of its 64 trials on Windows; CI's macOS `check` runs it once per run), and the existing lock tests
  (`lock.rs` unit tests, `c13_r4_lock.rs`, `c14_lock_races.rs`).

## 7. Constraints of the review, and where each is met

- Deleted while its protection is still held: A7, R and A5's failed write (own removal). C1's give-up (Unix only: the
  `flock` was never obtained because another opener holds it) removes nothing (D-83 item 1).
- A handle is trusted only while the path still names it (D-83 item 1): C2 after every creation, A6 before announcing,
  `still_ours` and every own removal (identity on Unix; on Windows the handle protects the file from its creation).
- Every removal re-validates, under protection, the very file the decision was made about (identity or nonce): A2, A4
  (proof of removal); own removals check the nonce and, on Unix, the inode.
- A decision made before a pause is never acted on after it without re-validation: A4's decision (`TakeoverDecided`)
  is followed by the proof and, after `TakeoverProven`, the identity and nonce check; A2 likewise.
- Retries are not the fix: the loop that re-decides on a changed file is the protocol's normal path, not a cure for a
  removal that already happened; no removal can be wrong in the first place.
