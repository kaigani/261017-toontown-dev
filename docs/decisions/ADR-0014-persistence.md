# ADR-0014 — Persistence: packages, folders, saves, recovery and the `project.*`/`doc.*` commands

Status: accepted (M1 C1.3, 2026-10-26). Scope: plan §4 row M1, checkpoint C1.3; the C1.3 tickets of the C1.2
closure (`reviews/M1-C1.2-r5-closure.md`).

## Context

C1.2 delivered the serialized executor with a guarded load path (ADR-0013 §16) but no files. Spec 10 §6–§8 fixes:
- the package (`.animpkg`, zip with ZIP64) and its folder form;
- the manifest and versions, migrations;
- the load sequence with repairs and a load report;
- atomic saves, backups, autosave, crash recovery, read-only and locking.

Spec 01 §3 and §11 add new-project defaults, recent projects, presets and persisted user settings. Spec 10 §10 and
spec A list the `project.*` and `doc.*` commands. Spec 11 §14.3 requires a round trip: objects created by commands,
saved, reopened, read back identically.

## Decision

1. **Two crates, one owner of installation.**
   - `tt-persist` reads and writes files. Loading produces a *candidate* `Document` plus its load report, preserved
     data and binary entries. It never touches an open document.
   - Only the executor installs a candidate. It does so through `install_into`, which needs a `LoadGrant`, after
     its integrity gate (`Document::issues()` errors: schemas, references, REV-01…06, §5 limits). Every path runs
     that gate first: open, recovery, backup, repair and migration.
   - Unchecked assembly (`Document::from_parts`) is used only by the candidate load and repair code (the C1.1
     confinement ticket).

2. **Folder form.** A folder project holds the same entries with the same bytes as the package (FMT-01). There is
   one difference: the folder manifest omits the entry list (10 §6.4).
   - **Control files** are files whose first path segment starts with `.`: `.lock`, `.save-journal.json`, temporary
     files, and the `.git/`, `.gitignore` and `.gitattributes` of a version-controlled project folder. They belong to
     the folder, not to the project: they are never read as entries, preserved, listed or copied into a package.
   - **A folder save** writes each changed entry to a temporary sibling. It then records the pending renames and
     deletions in `.save-journal.json`, applies them, writes `manifest.json` last and deletes the journal (SAVE-02).
     Unchanged entries are not rewritten (LAY-04).
   - **On open, an interrupted save is completed or rolled back.** A temporary whose hash matches the journal, or a
     target that already holds the recorded content, completes the save. Anything else rolls it back
     (`project.saveInterrupted`).

3. **Atomic replacement and durability.**
   - Every file replacement writes a temporary sibling, flushes it (`sync_all`) and renames it over the target.
   - On POSIX the parent directory is then flushed.
   - On Windows, `std::fs::rename` is `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`. It replaces atomically, but without
     `MOVEFILE_WRITE_THROUGH`, which safe Rust cannot reach. The file content is durable (flushed before the rename);
     the rename itself becomes durable when the OS flushes metadata. We accept this: product code has no unsafe or
     native calls (plan §3.2).
   - **Package saves:**
     1. write `<target>.tmp-*`;
     2. verify it by reopening it and checking every entry against the manifest;
     3. stage the backup of the previous version (hard link, falling back to a copy);
     4. rename over the target;
     5. commit the backup.

     A failure at any step deletes the temporary and leaves the target untouched (SAVE-01).

4. **Repairs on open (REP-01/REP-02, LOAD-03).**
   - Repairs work on the entries' JSON, in a fixed order and in entry-path order. New IDs come from a generator
     seeded by the input, so the same files always give the same document.
   - Every removed or replaced value is listed in the load report with its old JSON (REP-02). An unreadable entry
     keeps its raw text under the parent's `ext["app.unreadable"]`.
   - **The repaired project opens modified.** Its history starts with the step "Repair on Open" (origin `load`),
     whose `doc.changed` reports the repairs. **That step cannot be undone.** Undoing it would install the unrepaired
     document, which fails the integrity gate (ticket "validate every candidate"). So:
     - to keep the repaired version, save it;
     - to drop it, close without saving.

     REP-01 says "undoable"; see SI-044.

5. **Newer minor versions (VER-02).**
   - Unknown properties are kept per entry as *overlays* and merged back on save.
   - An entry holding an enum value this version does not know is *frozen*. In memory it holds a known substitute;
     on save it is written back exactly as read.
   - **Frozen objects are read-only.** The executor refuses any transaction, dry runs included, that changes a frozen
     entry, with `doc.unknownValue` `{entry, unknownValues}`.
   - The `modifiedAt` stamp of a save is exempt, because a frozen `project.json` is written as read anyway.
   - The manifest keeps the newer `formatVersion` and its unknown members.

6. **Libraries, templates and palette files.**
   - `library.json` is the model's library object, with its `children` written as `root` and `format: "animlib"`.
     So the project-local library (`library/`) and a library folder on disk share one format (10 §6.11).
   - Templates (`.animtpl`) and palette archives (`.animpal`) use the same hostile-safe zip reader and writer as
     packages, with their own `mimetype` and manifest `format`.
   - The global library, with its locking (LOCK-03), is M9's. This checkpoint provides the file formats and the
     project-local library.

7. **Interchange import (`doc.import.json`, 10 §6.8).**
   - One transaction, validated as a whole before anything changes, one undo step "Import".
   - `idPolicy: "new"` gives every object a fresh ID, and every reference inside the bundle follows, including entry
     names that embed IDs. Colour IDs are kept: they are the identity that colour recovery works with.
   - `"preserve"` keeps IDs and makes an existing ID a conflict.
   - Name conflicts are handled by `rename` (`-2`, `-3`, …), `replace` or `fail` (`doc.importConflict`).
   - Palettes merge by palette ID, with swatches handled by `keepExisting` or `updateExisting`.
   - A scene imported into a project also gets a style entry for each palette of that project (10 §4.3). This is why
     the round-trip test compares the source's own style keys.

8. **Saves inside the executor.**
   - A save first stamps `project.modifiedAt` (10 §4.1, "set by each save") as a *system transaction*. It has its
     own revision and a `doc.changed` with `undoStepId: null` and origin `system`, but no undo step.
   - It then snapshots the document (a copy-on-write clone, taken between transactions, SAVE-04) and writes it on a
     job thread.
   - The result is applied on the document thread. The clean marker moves to the snapshot's undo position only if
     the history still leads there.
   - A save cannot be cancelled once writing starts (SAVE-03). `app.quit {onUnsaved: "save"}` and the load path's
     `onUnsaved: "save"` save synchronously on the document thread.

9. **Backups (BAK-01…04).**
   - Backups live in `<user>/Backups/<projectId>/<savedAt>.animpkg`, indexed by `backup.json`.
   - `files.backups.count` versions are kept, oldest removed first. A folder project is backed up as a package
     snapshot (BAK-02).
   - `project.backup.open` opens a backup as an untitled copy.

10. **Autosave and recovery (AUTO-01…04, REC-01/02).**
    - Maintenance runs on the document thread. Hosts tick it every second; tests call
      `ExecutorHandle::run_maintenance`, which delivers its events before returning.
    - It writes a folder-form copy of a snapshot to `<user>/Recovery/<recoveryId>/project/`, with `recovery.json`.
      Only changed entries are written. The project file, its clean marker, history and location are never touched.
    - The owner refreshes `heartbeatAt` every 60 s. Recovery data is offered when its owner is gone: the heartbeat
      is older than 5 minutes, or the process is known dead on the same host. Liveness is checked with `kill -0` on
      POSIX and `tasklist /FI "PID eq …"` on Windows; unknown means alive.
    - A recovered project is modified and keeps its original path. Saving it deletes the recovery data (AUTO-04).

11. **Locks (LOCK-01/02, D13).**
    - The lock file holds a random `lockId`, so two executors in one process are still "someone else" to each other.
    - A fresh lock is created exclusively.
    - A live lock opens the project read-only (`project.lockedByOther` in the report, holder in `doc.state.get`).
    - A stale lock (5 minutes, or a dead process) is taken over with a warning.

12. **External changes (RO-02).**
    - Maintenance compares the project file's size and modification time (for folders, those of `manifest.json`).
    - A change emits `doc.package.changedOnDisk` and marks the project.
    - The next plain `project.save` fails with `project.changedOnDisk`, carrying a hint naming both choices:
      - *reload*: `project.open` with `onUnsaved`;
      - *keep mine*: `project.saveAs {path, onExists: "overwrite"}`.
    - RO-02 asks for a `needs_confirmation` result. The confirmation protocol is C1.4's (plan row M1), and this
      error is the kernel's pre-confirmation form; see SI-045.

13. **User folders and settings (01 §3.7, §11).**
    - `PersistOptions.user_dirs` names the settings, recovery and backups folders. Hosts pass
      `UserDirs::platform()`.
    - Library embedding and tests get an isolated temporary folder, removed when the executor stops.
    - Settings are four versioned JSON files (`preferences`, `recent`, `state`, `workspaces`), written atomically.
    - A corrupt file is kept aside, the defaults apply to that file only, and it is reported once (`prefs.corrupt`).
    - Workspaces are written whenever the window/workspace state changes and are restored at start.

14. **File-system roots (11 §6.2).**
    - Path parameters must be absolute (01 §12.1 rule 2).
    - An agent may use paths inside the open project's folder and inside the roots its host grants
      (`ExecutorHandle::grant_roots(session, roots)`); anything else is `fs.outsideRoots`.
    - User origins are not restricted.

15. **Error details and codes.**
    - `project.invalidFormat` always carries `details.reason`. Spec 10 names `unsafeEntry` and `sizeLimit`; this
      checkpoint adds `corrupt`, `encrypted`, `notZip`, `noMimetype`, `wrongMimetype`, `duplicateEntry`, `noManifest`,
      `manifest`, `project`, `library`, `palette`, `unrepairable`, `migration` and `wrongFormat`.
    - Codes the spec does not list are recorded in SI-046.

16. **Lint checks owned by later milestones (`doc.validate`, 10 §10.3).** Checks that need computations owned by
    later milestones are returned in `unavailable` with their owner, not guessed:
    - `unpaintedZones`: M2 C2.2;
    - `missingFonts`: M2 C2.1;
    - `staleLipSync`: M8 C8.2;
    - `transitionPadding`: M9 C9.2;
    - `outdatedTemplateLinks`: M9 C9.1.

17. **Deferred load steps.**
    - Colour recovery for unresolved colour IDs (§7.1 step 6, 03 §11.2) is M2 C2.2's. Until then, unresolved IDs
      stay unresolved and are reported (`colour.unresolved`).
    - Derived data and thumbnails (§7.1 step 7) belong to their owners: zones M2, thumbnails M3.
    - `thumbnail.png` is preserved when present, and not generated.

18. **Migrations (VER-03/04).**
    - A step is a pure `fn(Entries) -> Result<Entries, String>`; the product chain `migrate::CHAIN` is empty, since
      1.0 is the first format.
    - Every product step needs a fixture folder `crates/persist/tests/fixtures/migrations/<from>-<to>/`, enforced by
      `tests/migrations.rs`.
    - A migrated project opens unmodified. Its first save needs `allowFormatUpgrade` (`project.formatUpgrade`).
    - Hosts and tests may supply their own chain (`PersistOptions.migrations`).

## Consequences

- One reader serves packages, folders, templates and palette archives: hostile-input safety is in one place
  (`tt_persist::zip`). The hostile cases include unsafe names, size lies, decompression bombs, duplicates, encryption,
  truncation, path escapes and symbolic links. They run in CI on both platforms.
- Windows saves are atomic but not write-through (§3). Revisit if a safe API becomes available.
- REP-01 and RO-02 diverge from the letter of the spec until the spec owner rules (SI-044, SI-045).
