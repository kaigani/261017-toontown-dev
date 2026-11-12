# ADR-0020 — Compositor and render list

Status: accepted (M3 C3.3; amended in C3.3 rounds 1-3 and C3.4 rounds 1-3).

## Context

06 §6 makes one render list per frame the input of every renderer back end, so that final renders, hit-testing and
interactive views differ in quality only. 09 §11 requires deterministic `render.frame.run` (identical pixels and PNG
bytes for equal inputs), inline/file rules and image-sequence jobs; 11 §7.5 requires `describe_frame`, `hit_test` and
`convert_coordinates` that agree with what is rendered; 11 §14 item 5 requires golden images that match byte for byte
across runs and between attached and headless modes. ADR-0006 fixes the rasteriser (vello_cpu, f32, scalar, single
thread); ADR-0019 fixes the evaluation order of transforms.

## Decision

1. **One render list** (`commands/render_list.rs`). For (scene, frame, camera, output size) the compositor walks the
   scene tree, evaluates every node through the C3.2 evaluator (charged, ADR-0019) and produces the content entries:
   drawing, image and annotation elements with their exposed drawing, world matrix, world Z, Timeline rank,
   drawing → pixel map, bounds and a visibility verdict. Renders, `inspect.frame.describe`, `inspect.hitTest`,
   `inspect.coordinates.convert`, the contact sheet and the range job read only this list.
2. **Order** (SI-106): role tier (background, normal and envelope, foreground; 04 §6.1), then the world Z of the origin
   (05 §2.8, smaller first), then the Timeline order (back to front = displayed rows bottom to top). Depths are not
   rounded (ruling C33-R1-01): within a tier the entries are sorted by Z and grouped from the smallest; a group takes
   each following entry whose Z is less than 1e-6 field from the group's first (smallest-Z) member, its anchor, and is
   ordered by the Timeline alone. **Chain rule:** a long run of near-equal depths splits where the distance from the
   anchor reaches 1e-6, although each step is smaller (0, 0.6e-6 and 1.2e-6 make two groups). Sorting and grouping use
   only comparisons of exact f64 values, so the order is platform-independent.
3. **Visibility** as `anim.evaluate` (SI-100): hidden (itself or an ancestor), annotation (view-only), no drawing at the
   frame, empty drawing, plane at or behind the near plane, out of frame. A request may carry a render-time override
   (ruling C33-R1-05): `include` draws an element although hidden or an annotation, `exclude` leaves an element and its
   descendants out (verdict `excluded`; exclusion wins). The override is part of the request, never of the document, so
   `exclude: [e]` renders the bytes of hiding `e`. Background elements extend their last exposure
   to the scene length. Effects pass the transform through (M7); bones are absent (SI-097).
4. **Projection per plane.** Elements rotate about Z only, so each lies in a plane parallel to the table plane and the
   rigid camera's perspective maps that plane to output pixels affinely. Three projected points (origin, +1 field in x
   and y) fix the map exactly; drawing units are 1/100 field.
5. **Rasteriser items.** `tt_render::Item::Layer { transform, items }` (additive): each drawn element's drawing is built
   in drawing units by the drawing builder of `draw.drawing.render` (shared, colours through the scene's active palette
   styles) and wrapped in a layer holding its drawing → pixel map, so vello strokes and paints anisotropic and skewed
   elements exactly. `draw.drawing.render` keeps its own path; the M2 goldens do not move.
6. **Bitmaps** (SI-105): a bitmap node's pixel space (origin top-left, y down) maps to drawing space through its
   `transform` (10 §4.9); the image is drawn as an image paint over its quad, clipped to it, bilinear (`ImageQuality::
   Medium`). Native size is the scale `1200 / cameraWidth` du per pixel with a y flip (`[s, 0, 0, −s, tx, ty]`), so one
   image pixel covers one output pixel of the default camera on the table plane; hit-testing reads the pixel's alpha
   (fully transparent pixels are not hit, 06 §6.8).
7. **Encoding.** PNG through `tt_render::encode_png` (no ancillary chunks: equal buffers give equal files); JPEG, TGA and
   BMP through the pinned `image` crate (`tt_io::image::encode_still`; JPEG keeps the premultiplied colour, i.e. over
   black).
8. **Never block.** Every frame charges a work account (path elements and pixels / 1024, 200 M units) and observes the
   caller's cancellation inside the rasteriser, an empty frame included (the check runs before each item and before
   and after the flush). Known sizes are refused before any evaluation, allocation or encoding (ruling
   C33-R1-04): sizes over 8192 and frames over 100 000 (`limit.exceeded`; per frame at a range's preflight), and an
   inline frame or contact sheet whose long side or pixel count is over the session's inline limit
   (`render.inlineTooLarge`). The encoded-byte limit is checked after encoding, which is bounded and cancellable.
   `render.range.run` runs as a job on a document snapshot, one frame at a time, checking cancellation. With
   `wait: true` it answers the manifest with the `jobId`, or `job.waitTimeout` with the `jobId` while the job runs on
   (ruling C33-R1-02). Where the wait runs:
   - A direct call's wait is on the caller's request, never the document thread, and is bounded by the call's
     watchdog budget.
   - **Exception (C34-R2-03): a sequential batch's waited job step and its `job.wait` step wait on the document
     thread.** They are bounded by the batch's remaining watchdog budget minus its margin, and they end on the
     batch's request cancellation. Measured in C3.4 round 2: a concurrent read waited 740 ms of a 1000 ms budget while
     an uncancelled batched waited step ran, and about 33 ms after the batch's cancellation.
   - Off-thread batch continuations are not planned; M11 C11.1 re-measures this bound with the attached UI.
   **Request cancellation** (M3 C3.4 rounds 1 and 2: O-1, rulings C34-R2-01, C34-R2-02, C34-R2-04; code SI-111). A
   caller can cancel its request: an MCP `notifications/cancelled`, or the executor's `invoke_served_until`,
   `batch_until` and `batch_audited_until` with a cancellation probe. The contract:
   1. *Acceptance.* A request that may change something (an action, a job, a batch) and whose cancellation is
      observable when the call is made never begins: the probe is checked before enqueue. There is no document
      change, no undo step, no event and no job, and nothing is remembered under its `requestId`, so a retry executes
      once. A query changes nothing: it is not refused before it begins, and a cancellation stops it at its next
      work-account check.
   2. *Boundaries.* After acceptance, the cancellation is arbitrated at the executor's boundaries: dequeue, the start
      of every invocation and batch step (before its handler runs, so a job step whose request is cancelled never
      starts its job), every acceptance check, immediately before commit, and every work-account check. At dequeue,
      at each start, at acceptance and before commit, the worker asks the caller **directly**: a link carried by the
      request, which the waiting caller answers by evaluating its probe at once. A worker never passes one of these
      boundaries once the cancellation is observable. A call made without a probe (`invoke`, `invoke_served`,
      `batch`, `batch_audited`) carries no link and costs no question. A probed query carries a notice-only link: it
      is never asked, but is told when it is discarded. A probed action asks once at dequeue and three times per
      committing step (start, acceptance, pre-commit); a step that changes no document asks twice. The caller also marks the call's watchdog token when it sees the
      cancellation, which stops work-account checks; a refusal for cancellation is not remembered under the
      `requestId`.
   3. *A commit that got there first stands.* A commit that passed its pre-commit check before the cancellation was
      observable stands whole: one undo step, one event set, its envelope remembered under its `requestId`. This is
      not a defect.
   4. *Jobs.* Request cancellation never cancels a job. A wait on a job (a direct `wait: true`, a batch's waited step,
      a batch's `job.wait` step, a direct `job.wait` and its attached retry) ends with `command.timeout`,
      `details.reason: "cancelled"` and `details.jobId`. The job stays in `job.list` / `job_status`, runs on to its own
      terminal state or stops at `job.cancel`, and leaves no staging once terminal (decision 11).
   5. *Timing.* Waits and the caller's token mark poll the probe every 20 ms (`CANCEL_POLL`). That is a wake-up
      interval, not an end-to-end bound. C3.4 round 2 measured 100 direct waited-job cancellations at min 0.67 ms,
      median 3.6 ms, p95 4.8 ms, p99 20.2 ms and max 20.3 ms.

   **Probes must not block** (C34-R3-01). The document thread never depends on caller code for longer than one bound:
   each question waits at most `ASK_BOUND_MAX` (250 ms) or a quarter of the watchdog budget, whichever is smaller.
   Silence is not cancellation. A caller that does not answer in time is taken as not cancelled at that boundary, and
   its link is latched unresponsive, so the worker asks it nothing more for that request. One request therefore stalls
   the document thread by at most one bound, whatever its step count. From then on its cancellation is seen only
   through its marked token; seen late, it is like a commit that got there first (rule 3). A caller that is gone
   (disconnected link) counts as cancelled. A caller held past its own watchdog budget (inside its probe, say) answers
   the timeout even if the reply has arrived; a commit that happened stands.

   **Causes** (C34-R3-02). A token remembers why it was marked: by its caller's watchdog timeout, or by its caller's
   cancellation. Every refusal whose cause is the caller's cancellation carries `details.reason: "cancelled"`
   (SI-111): before enqueue, at dequeue, at an invocation's or step's start, at acceptance, before commit, and at a
   work-account check. A watchdog timeout is never relabelled.

   A queued request discarded because it was cancelled answers that cancellation, never `internal`. Only the worker's
   positive notice that it discarded the request makes a dropped reply a cancellation; a marked token alone does not,
   so a worker that really stops answers `internal`, also for a request cancelled while queued (C34-R2-04,
   C34-R3-03). An MCP client receives no answer to a cancelled request (11 §4).
   Watchdog tokens are bounded to outstanding calls: marking a token the worker already forgot does nothing.
10. **Publication is a transaction** (ruling C33-R1-03; amended in round 2). Frames and the companion manifest are
   staged beside their targets as `.<target name>.tt-partial-<token>`. At the commit point every target is
   preflighted (an existing target must be a writable regular file, else `export.targetInvalid`; the conflict policy
   was applied at submission); then each existing original is moved aside to `.<target name>.tt-backup-<token>` and the
   staged file renamed onto the target. On any failure the publication is undone: published targets without an
   original are removed and the originals restored. If the file system refuses part of that undo, the job is `failed`
   with `export.rollbackIncomplete` (`details.cause`: the failure that started it; `details.unrestored`: the targets
   not restored), never silently; the backups and the sidecar are kept, so the next range job into the directory
   retries (decision 11). On success the sidecar is marked `published`, the backups are deleted and the sidecar is
   removed last; files that cannot be removed are reported (`render.stagingLeft`). The manifest exists only after full
   success.
11. **Recovery contract** (rulings C33-R1-08, C33-R2-01, C33-R2-02). Recovery acts only on files the product can prove
   it owns.
   - **Exact names.** A job's token is exactly 12 lowercase hexadecimal characters (six random bytes; the generator's
     no-randomness fallback is hashed into the same format). Its files are `.<target>.tt-partial-<token>` and
     `.<target>.tt-backup-<token>`, where each `<target>` is a file name the job's own pattern and range produce for
     the directory (the frames and the manifest).
   - **Owned sidecar.** Before its first staged write a job writes `.tt-partial-<token>.job` in the directory: JSON with
     `kind` `toontown.render.range.staging`, `version` 1, `token`, `jobId`, `head` (the CI commit the product was built
     from, `unknown` in local builds), `productVersion`, `host`, `pid`, `published`, `targets`, and the `staged` and
     `backups` names derived from them. It is removed last.
   - **Who recovers what.** A recovery reads only sidecars whose name has a well-formed token, which are regular files,
     whose `kind`, `version` and inner `token` match, whose targets are plain file names (never paths) and whose
     staged and backup names are exactly the derived ones; it acts only on the files such a sidecar lists. Files not
     listed, malformed tokens, marker-like names and invalid sidecars are never touched and never reported. The next
     range job into a directory recovers the sidecars whose job is not alive: a job of this process whose token is no
     longer live, or a job of another process on this machine that is known to have exited; a job of another machine,
     or of a process whose state is unknown, is left alone. A cancelled or failed job recovers its own sidecar only.
   - **What recovery does.** Listed staged files are removed. A listed backup is deleted when the sidecar is
     `published` (the publication completed); otherwise its original is put back onto the target, replacing the file
     the interrupted publication left there (the backup proves the job moved the original aside). The sidecar is
     removed only when nothing failed, so a refused step is retried by a later job.
   - **Receipts.** The next job lists what it recovered in its warnings (`render.staleStagingRemoved`: `files` removed
     or moved back, `restored` targets, `unrestored` names left). A cancelled or failed job records its own recovery
     (`render.stagingDiscarded`, same fields) in its job record's `warnings`, kept with the state and the typed
     error (a cancelled record keeps `job.cancelled`, or what met the cancellation, such as
     `export.rollbackIncomplete`; C33-R3-01), readable through `job.list`, `job.wait` and MCP `job_status` after
     completion. Neither state becomes success. Dead jobs are recovered by the entry recovery of the next job into the
     directory; a cancellation recovers only the cancelled job's own token.
   - Process death may leave a job's staged files, backups and sidecar; they are never published, and the next job
     into the directory recovers them as above.
12. **Volatile fields** (ruling C33-R1-06). `renderMs` (in `render.frame.run`, the generic `render_frame` result and
   per run), job IDs and job timestamps are volatile and excluded from deterministic byte comparisons; the PNG bytes
   and their `sha256` are the determinism contract. The retained example results record `renderMs` as varying.
13. **Invalidation.** `anim.changed` is conservative: every frame whose render can change is covered by the announced
   range (all-frame edits announce every frame); a frame inside the range may render unchanged.
9. **Determinism gates.** Golden PNGs (`crates/executor/tests/golden/c33/`) are compared byte for byte on both OSes in
   debug (`check`) and release (`c33 render release`); the render gate (`scripts/c33-render-gate.py`) replays
   `tests/fixtures/c33/gate-scene.json` on the release headless binary and checks the in-process hashes
   (`gate-index.json`), and `c33 cross-OS` compares both OSes' gate images, sequence frames, manifest and the passage
   frames of a non-horizontal path byte for byte.

## Consequences

- A change of `vello_cpu`, `png`, `image` or the compiler that alters a byte fails the goldens and the cross-OS gate; a
  legitimate change re-records the goldens and is declared with the old and new hashes.
- Effects (M7), bone deformation (M6 C6.3), the interactive renderer and views (M4 C4.2) add to the render list and its
  back ends without changing the order, visibility or projection rules above.
- Text pixels come from the machine's fonts (ADR-0016), so goldens contain no text.
