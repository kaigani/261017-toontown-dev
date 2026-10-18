# ADR-0009 — Tablet input architecture

Status: **proposed; not decided.** C0.4, 2026-10-18; decision rules made exhaustive after review round 1. The decision is conditional on two measurements nobody has yet:
the user's macOS run (`docs/manual-tests/M0-C0.2-tablet-macos.md`) and one real-tablet session on Windows. This ADR
records the evidence, the candidates, and which outcome selects which candidate.

## Context

Spec 02 §6 needs pressure, tilt, barrel rotation, eraser/tool identity and high-rate (coalesced) samples for brush
strokes. Plan §2.3 names the risk ("pressure/tilt/coalesced pointer events in WKWebView with Wacom/Apple tablets")
and a fallback: native tablet input in the Rust host, streamed over a Tauri channel.

C0.2 measured with **synthetic pen input only**, on the GitHub `windows-latest` runner VM; this machine has no pen
hardware. Sources: `reviews/M0-C0.2-handoff.md`, matrix 5 (CI 39116097955,
`reviews/evidence/M0-C0.2/tablet-spike-windows-runner-r2-matrix5.json.gz`, cross-checked by the reviewers'
`r2-independent-audit.txt`), and the closure report `reviews/M0-C0.2-r4-closure.md`.

### Established (synthetic pen, Windows runner, 1024 contact frames per run)

1. **The WebView2 DOM drops pen updates that do not move the pointer.**
   - Stationary and pressure-only strokes deliver only the `pointerdown` (1 of 1024).
   - The four repeated-pixel frames of the sine path are missing at 240 and 500 Hz; the zigzag path, with no
     repeats, misses none of them.
   - At least one of the first three frames after the down is missing in all 15 moving webview strokes of matrices
     1–5.
2. **Same-process listeners receive every frame.**
   - WM_POINTER on a plain same-process window: 1024/1024 for sine, stationary and pressure-only, including
     repeated pixels.
   - A hidden page-wide raw-input sink: 1046 HID digitizer reports per run, equal to the injected frames. The
     reports are **not decoded**.
3. **RealTimeStylus on the app's main (webview) window received 0 frames** in every run. RTS on a plain window is a
   working positive control: RTS and WM_POINTER together account for 1024/1024.
4. **Twist arrives modulo 180°** on the web path; values with rotation ≥ 180° arrive 180° lower. WM_POINTER returns
   the full rotation.
5. **What the web path does deliver:** pen type, exact tilt, a 1024-level pressure grid, and `getCoalescedEvents`
   batches (p50 4 samples per dispatch). Event timestamps fall within about 0.3–0.9 ms of injection (p50).
6. **`octotablet` 0.1.0 has no macOS backend.** The spike's macOS native path is an AppKit `NSEvent` local monitor
   with type-gated accessors. **It has never received input.**

### Hypothesis (not established)

- *Which layer drops the updates.* The handoff says "non-position-changing pen updates are lost on the way into, or
  inside, the WebView2 input path". Whether WebView2's input window receives the WM_POINTER updates is not measured
  (experiment E1). The r4 closure says: "the webview's receiving WM_POINTER boundary is unmeasured. CDP is a feasible
  experiment whose actual trace coverage must be established."
- *Real hardware.* Whether a real pen and driver behave like synthetic injection, in the webview, in main-window RTS
  or in raw input. Also whether raw HID reports carry usable pressure and tilt once decoded.
- *macOS.* Whether a tablet's pressure, tilt, twist, or even `pointerType = pen` reach WKWebView. The API surface
  (coalesced events from WebKit 18.2; pressure/tilt properties from WebKit 13) "dates the API; it does not establish
  that a Mac tablet's pressure ... reach[es] WKWebView". Two indications point the other way: Wacom reports no pen
  pressure in Safari 15 on macOS 12.1, and Mac WebKit derives mouse-event force from `NSEventTypePressure` (Force
  Touch) (`reviews/M0-C0.2-handoff.md`, finding 3).

## Candidates

| Id | Candidate | Platform | What it fixes | Known cost / risk |
|---|---|---|---|---|
| W | Web pointer events only (`pointerrawupdate` / coalesced events) | both | nothing to build; one code path | the losses above, if they occur with real hardware; twist mod 180 |
| R | **Raw HID listener in the host**: `WM_INPUT` digitizer collection, decoded with `HidP_*` from the device's report descriptor (experiment **E-R1** builds the decoder), samples streamed over a Tauri channel | Windows | received every synthetic frame | per-device report descriptors; vendor-specific usages; mapping to canvas coordinates; driver modes (Wacom WinTab vs Windows Ink) |
| H | **RealTimeStylus on the main window** (`octotablet`, the spike's Windows native listener) | Windows | measured in T-W1 directly; nothing new to build | received 0 synthetic frames over the webview; real-pen behaviour unknown |
| D | **Dedicated same-process input window** with direct WM_POINTER (or RTS) over the canvas, forwarding samples to the page (experiment **E-D1** builds it) | Windows | WM_POINTER delivered 1024/1024; RTS positive control | it sits above the webview: focus, IME, cursor, hit-testing and resize synchronisation must be solved; composition hosting is not available in wry/Tauri today |
| A | **AppKit `NSEvent` monitor in the host** (the spike's `native_macos.rs`), streamed over a channel | macOS | independent of WKWebView's pointer events | unmeasured: coordinates (bottom-left origin, backing scale, multiple displays), tilt calibration, eraser and button mapping |
| E1 | *Diagnostic, not an architecture:* a WebView2 DevTools-protocol trace (CDP `Tracing`, `input` / `latencyInfo`) aligned with injection times, to locate where updates are lost | Windows | tells W vs H/D apart on evidence; may show a WebView2 setting or bug to report upstream | trace coverage of the browser input path is itself unestablished |

`RegisterPointerInputTarget` (needs UIAccess, redirects rather than duplicates input) and wintab are noted
comparisons, not candidates.

## Decision (conditional; nothing is chosen yet)

The front end gets one **input-source abstraction** in M2/M4. The brush engine consumes timestamped samples (position,
pressure, tilt, twist, tool) and does not know which candidate produced them. It is needed under every outcome
below.

The architecture is chosen **only from physical-pen evidence**, by the rule set below (revised after C0.4 review
rounds 1 and 2). It is total: every log maps to exactly one outcome, either an evidence outcome from Step 0 or one
decision from Step 3. The procedures are T-M1 (`docs/manual-tests/M0-C0.2-tablet-macos.md`) and T-W1
(`docs/manual-tests/M0-C0.4-tablet-windows.md`, with E1 alongside). The go/no-go report carries an identical copy,
and `desktop/src/test/decisionTables.test.ts` fails if the two differ.

<!-- tablet-rules:begin -->
**Actions and episodes.** The procedures number the **actions** a person performs:
- A1 pressure ramp;
- A2 speed;
- A3 tilt;
- A4 barrel rotation (optional);
- A5 hover;
- A6 stationary pressure.

The analyzer numbers **contact episodes**. The page records an action marker when the person presses and releases
Space at the start and end of each action, and K to skip it.
- Markers are taken on the **keyup** of a deliberate press. Auto-repeat keydowns are ignored, a keyup needs its own
  keydown, and two transitions are at least 500 ms apart.
- Every Space/K event is logged with what it did (`keyEvents`).

Each episode belongs to the action whose span contains its first sample: web samples by the page's performance clock,
native packets by host wall time. Every row below names an action, never an episode number.

**Only samples inside an action's span are decision inputs.** Every per-action statistic is computed from the
measured episode's samples inside that span, including row 6's coalescing, which comes from A2's own samples. Samples
outside every span are excluded before anything is computed, and the analyzer prints how many per path.
- Per path, an action is "episode n", "split" (the episode with most samples is measured), "performed, no events"
  (done, but no contact on that path), "omitted" (skipped with K) or "unfinished" (saved mid-action).
- Clicks on Save, the form or the listener button fall outside every action and are ignored.
- The manual tests show the expected mapping for a clean run.

**Step 0: is the log evidence?**

| Condition | Outcome |
|---|---|
| `scripts/analyse-tablet-log.py` exits 2; or `source` is not `manual`; or the log has run or simulation records; or the input is synthetic | **not evidence**: repeat the procedure |
| Malformed action markers, including overlap on either clock, or raw key events that do not account for every marker transition (exit 2) | **not evidence**: repeat the procedure |
| The log has no action markers (it predates them) | **not evidence**: repeat the procedure with the current build |
| A row's action is omitted, unfinished or not recorded (A4 omitted is fine when "Pen has barrel rotation" is no) | **insufficient evidence: repeat that action** |
| Row 7 or 8: A2 has a single sample (rate null) | **insufficient evidence: repeat A2** |
| Row 4 or 5 fails and the pen's tilt or rotation support is not stated in "About this run" | **insufficient evidence: state the pen's capability** |
| The pen lacks tilt or barrel rotation by specification ("About this run": no) | that row is **n/a** for every candidate |

Any "insufficient evidence" outcome ends the evaluation with that outcome. Otherwise continue.

**Step 1: classify each row for each measured candidate.** Bands are half-open, `[low, high)`, and apply to the
analyzer's **unrounded** value. Row 6 is n/a for native candidates. "Performed, no events" counts as the fail
column.

| # | Capability | Action | Pass | Partial | Fail |
|---|---|---|---|---|---|
| 1 | Pen identity (`pointer` pen/eraser) | A1–A3 | all three actions | one or two of them | none |
| 2 | Pressure ≥ 256 levels | A1 | [256, ∞) levels | [16, 256) levels | [0, 16) levels |
| 3 | Pressure ≥ 1024 levels | A1 | [1024, ∞) levels | [256, 1024) levels | [0, 256) levels |
| 4 | Tilt (`tilt x / y` extrema) | A3 | min ≤ −30° and max ≥ +30° on both axes | a non-zero range, otherwise | zero range on both axes |
| 5 | Twist (`twist range (nonzero)`) | A4 | range width in (180°, ∞) | non-zero samples, width in (0°, 180°] | no non-zero sample, or width 0 |
| 6 | Coalesced events (web only) | A2 | API available, batching observed in A2's samples | API available, no batching in A2 | API not available |
| 7 | Sustained rate (`Hz`) | A2 | [120, ∞) Hz | [60, 120) Hz | [0, 60) Hz |
| 8 | Rate ≥ 200 Hz (`Hz`) | A2 | [200, ∞) Hz | [120, 200) Hz | [0, 120) Hz |
| 9 | Stationary pressure-only updates | A6 | more than one sample and pressure value, [60, ∞) Hz | more than one sample and pressure value, rate below 60 Hz or undefined | one sample (the down only), or a single pressure value |

Pressure levels are the analyzer's implied grid when A1 has ≥ 64 distinct values; otherwise they are the distinct
count. In row 9, one sample is the failure the row tests, not missing evidence.

**Candidates and priority.** Each measured candidate is evaluated against every row on its own.
- **Windows:** **W** (web pointer events) > **R** (raw HID; measurable only after experiment E-R1 decodes it) > **H**
  (RealTimeStylus on the main window, the spike's native listener) > **D** (dedicated input window with direct
  WM_POINTER; measurable only after E-D1).
- **macOS:** **W** > **A** (AppKit monitor).

W comes first because it needs no native code, and R next because raw HID was the only path that saw every
synthetic frame. A T-W1 log measures W and H; a T-M1 log measures W and A. A candidate that is not measured is not
scored, and the decision is final for that evidence. Passing a row on a native path proves only that row: native pen
identity says nothing about native pressure, tilt or rate.

**Step 2: select one candidate** (rulings C04-R3-04 and the final C0.4 ranking ruling).
1. A candidate **without any applicable measured row** (not measured, or every row n/a) is excluded.
2. A candidate at **fail on row 2 (pressure) or row 7 (rate)** is **disqualified**. It is listed with its share but
   never selected.
3. A candidate's **share** is the number of its applicable rows at pass divided by its number of applicable rows, as
   an exact fraction. An n/a row is in neither part, so a native candidate is not penalised for row 6.
4. Among the candidates that are not disqualified, the highest share is selected; equal shares go to the higher
   priority (W > R > H > D, or W > A).
5. **Two sources are never combined.** The other qualified candidates may be offered as the "Tablet input interface"
   choice of spec 02 §6.3.

**Step 3: the decision.**

| Result | Decision |
|---|---|
| No candidate has a valid measurement | **not evidence**: repeat the procedure |
| Every candidate with a valid measurement is disqualified | **no-go on this platform: escalate (plan §6 decision 7)**; the candidate with the highest share is named only as the best available evidence |
| The selected candidate passes every row that is not n/a | **go: that candidate** |
| Otherwise | **go-with-conditions: that candidate**, with each row at partial or fail and its mitigation from the table below |

| # | Mitigation when partial | Mitigation when fail |
|---|---|---|
| 1 | pen recognised by pressure > 0; eraser not distinguished | pen not identified on this path: contacts with pressure > 0 are treated as pen; no eraser |
| 2 | accept the measured resolution of 16 to 255 levels; brush response quantised | disqualifies the candidate (Step 2) |
| 3 | accept >= 256 levels | as row 2 |
| 4 | accept the limited or quantised tilt | tilt unsupported on this platform: tilt-driven brush options off |
| 5 | twist limited to the measured range (e.g. modulo 180) | barrel rotation unsupported on this platform |
| 6 | accept one sample per dispatch | accept one sample per dispatch; a floor change is a user decision (ADR-0005) |
| 7 | interpolated sampling off; the >= 200 Hz target is recorded as below target | disqualifies the candidate (Step 2) |
| 8 | recorded as below the 200 Hz target | recorded as below the 200 Hz target |
| 9 | pressure interpolated between updates | pressure changes without movement are ignored |

**Worked examples** (checked by `scripts/test_decision_rules.py`). The shares are exact fractions of applicable rows.
1. **Windows.** All four candidates are measured, and every row not named here is at pass.
   - W: pressure partial (rows 2 and 3 at partial and fail), tilt at fail.
   - R: tilt at fail.
   - H: pressure at fail.
   - D: pressure and tilt at fail.

   H and D are disqualified (row 2 at fail). Shares: W 6/9, R 7/8 (H 6/8 and D 5/8 are listed only). Decision:
   **go-with-conditions: R**, with row 4 at fail: tilt unsupported on this platform.
2. **macOS.** W runs at 50 Hz and A at 90 Hz; every other row passes on both.
   - W is at fail on row 7, so it is disqualified (share 7/9, listed only).
   - A has row 6 n/a, row 7 partial (not fail) and row 8 fail: share 3/4.

   Decision: **go-with-conditions: A**, with:
   - row 7 at partial: interpolated sampling off, and the 200 Hz target recorded as below target;
   - row 8 at fail: recorded as below the 200 Hz target.
3. **All disqualified.** On macOS, W is at fail on row 2 and A at fail on row 7. Decision: **no-go on this platform:
   escalate (plan §6 decision 7)**, with the higher share named as the best available evidence.

`scripts/analyse-tablet-log.py` implements these steps and prints the decision for every manual log. Its constants
are checked against this text by `scripts/test_decision_rules.py`.
<!-- tablet-rules:end -->

## Consequences

- Product tablet code waits for these measurements; the C0.2 spike code is not reused as is. Before any reuse, the
  closure-report risks must be resolved:
  - lifecycle;
  - per-device state;
  - simultaneous-device segmentation;
  - coordinates, Retina and multiple displays;
  - prefix/tail completeness of native streams.
- The local Windows synthetic run stays blocked while the two Windows Security prompts (`PickerHost`) cover the
  desktop. They belong to the user and are left untouched.
