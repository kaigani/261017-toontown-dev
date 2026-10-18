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
| R | **RealTimeStylus on the main window** (`octotablet`, the spike's Windows native listener) | Windows | measured in T-W1 directly; nothing new to build | received 0 synthetic frames over the webview; real-pen behaviour unknown |
| H | **Raw HID listener in the host**: `WM_INPUT` digitizer collection, decoded with `HidP_*` from the device's report descriptor; samples streamed over a Tauri channel and merged with DOM positions | Windows | received every synthetic frame | per-device report descriptors; vendor-specific usages; mapping to canvas coordinates; driver modes (Wacom WinTab vs Windows Ink) |
| D | **Dedicated same-process input window** (WM_POINTER or RTS) over the canvas, forwarding samples to the page | Windows | WM_POINTER delivered 1024/1024; RTS positive control | it sits above the webview: focus, IME, cursor, hit-testing and resize synchronisation must be solved; composition hosting is not available in wry/Tauri today |
| A | **AppKit `NSEvent` monitor in the host** (the spike's `native_macos.rs`), streamed over a channel | macOS | independent of WKWebView's pointer events | unmeasured: coordinates (bottom-left origin, backing scale, multiple displays), tilt calibration, eraser and button mapping |
| E1 | *Diagnostic, not an architecture:* a WebView2 DevTools-protocol trace (CDP `Tracing`, `input` / `latencyInfo`) aligned with injection times, to locate where updates are lost | Windows | tells W vs H/D apart on evidence; may show a WebView2 setting or bug to report upstream | trace coverage of the browser input path is itself unestablished |

`RegisterPointerInputTarget` (needs UIAccess, redirects rather than duplicates input) and wintab are noted
comparisons, not candidates.

## Decision (conditional; nothing is chosen yet)

The front end gets one **input-source abstraction** in M2/M4. The brush engine consumes timestamped samples (position,
pressure, tilt, twist, tool) and does not know whether they came from the DOM or from a native channel. That is
needed under every outcome below.

The architecture is chosen **only from physical-pen evidence**, by the rule set below. Every analyzable log maps to a
decision, a named next experiment, or an escalation, with no new analysis. The macOS log comes from T-M1
(`docs/manual-tests/M0-C0.2-tablet-macos.md`, six strokes). The Windows log comes from T-W1
(`docs/manual-tests/M0-C0.4-tablet-windows.md`, the same six strokes, with E1 alongside). The tables are the same
for both platforms; only the native candidates differ. The go/no-go report carries an identical copy, and
`desktop/src/test/decisionTables.test.ts` fails if the two copies differ.

<!-- tablet-rules:begin -->
**Step 0: is the log evidence?**

| Condition | Action |
|---|---|
| `scripts/analyse-tablet-log.py` exits 2; or `source` is not `manual`; or the log has run/simulation records | not evidence: repeat T-M1 / T-W1; no decision |
| A required stroke is missing, or the user marks it as a mistake | repeat that stroke; the rows that depend on it stay undecided |
| The pen or tablet lacks a capability by its specification (tilt; barrel rotation, which only pens such as the Wacom Art Pen have), as recorded in "About this run" | that row is **n/a** for every path and does not count |
| Synthetic input (`--spike-auto`, toolbar simulation) | never evidence for these tables |

**Step 1: classify each capability, separately for each path P** (W = web pointer events; N = the native candidate
being evaluated). Values are the analyzer's per-stroke figures for that path.

| # | Capability | Stroke | Pass | Partial | Fail |
|---|---|---|---|---|---|
| 1 | Pen identity: `pointer` column pen (web) / tool pen or eraser (native) | 1–3 | all three strokes | one or two of them | none |
| 2 | Pressure ≥ 256 levels | 1 | ≥ 256 | 16–255 | < 16, constant or binary |
| 3 | Pressure ≥ 1024 levels | 1 | ≥ 1024 | 256–1023 | < 256 (row 2 decides) |
| 4 | Tilt (`tilt x / y` extrema) | 3 | spans ≥ ±30° on both axes | non-zero range, but < ±30° on an axis | zero range on both axes while the pen tilts |
| 5 | Twist (`twist range (nonzero)`) | 4 | range wider than 180° | non-zero range of at most 180° (e.g. 0..179, modulo 180) | no non-zero sample, or zero range, while the pen rolls |
| 6 | Coalesced events (web only; n/a for N; the analyzer's coalescing line) | 2 | API available and batching observed | API available, batching not observed | API not available |
| 7 | Sustained rate ≥ 120 Hz (`Hz`) | 2 | ≥ 120 Hz | 60–119 Hz | < 60 Hz |
| 8 | Rate ≥ 200 Hz | 2 | ≥ 200 Hz | 120–199 Hz | < 120 Hz (row 7 decides) |
| 9 | Stationary pressure-only updates (`samples`, `Hz`, pressure distinct) | 6 | ≥ 60 Hz with more than one pressure value | more than one sample with more than one pressure value, but < 60 Hz | one sample (the down), or a single pressure value |

Pressure resolution is the analyzer's implied grid (from the smallest step) when stroke 1 has ≥ 64 distinct values.
Otherwise it is the distinct count.

**Step 2: what each W result requires.** "Native" means: take this attribute from the first native candidate, in
priority order, that **passes** the row. If no measured candidate passes, use the row's "otherwise".

| # | W pass / n/a | W partial | W fail |
|---|---|---|---|
| 1 | W | native tool identity, merged with W positions; otherwise W, with pen recognised by pressure > 0 | native for every pen attribute; otherwise **platform no-go** |
| 2 | W | native pressure; otherwise W with reduced pressure resolution (documented limitation) | native pressure; otherwise **platform no-go** |
| 3 | W | W (256–1023 levels are accepted; no native path for this row alone) | row 2 decides |
| 4 | W | W with quantised tilt; native tilt only if a native path is already used for another row | native tilt; otherwise tilt-driven brush options disabled on that platform (documented limitation) |
| 5 | W | native twist; otherwise twist treated modulo 180 (documented limitation) | native twist; otherwise barrel rotation unsupported on that platform (documented limitation) |
| 6 | W | W if row 7 passes; otherwise row 7 decides | W if row 7 passes; otherwise native (macOS: A). If A also fails row 7, raising the floor to a WebKit with coalesced events is a **user decision** (ADR-0005), not taken here |
| 7 | W | native if it passes; otherwise W with reduced fidelity (curve fitting smooths; documented limitation) | native if it reaches at least partial; otherwise **platform no-go** |
| 8 | W | W (below the 200 Hz target, recorded) | row 7 decides |
| 9 | W | W (pressure interpolated between updates) | native pressure updates merged with W; otherwise pressure changes without movement are ignored (documented limitation) |

**Native candidates, in priority order.** Each is evaluated with the same Step 1 rows.
- **Windows:**
  - **R:** RealTimeStylus on the main window. This is the spike's Windows native listener, measured in T-W1 itself.
  - **H:** decoded raw HID. It needs experiment **E-H1** (an `HidP_*` decoding prototype) before it can pass any row.
  - **D:** a dedicated input window. It needs **E-D1** (an overlay prototype).

  Candidates are evaluated in that order. When a row needs native and R does not pass it, the next experiment is E-H1,
  then E-D1. The decision waits only for that experiment.
- **macOS: A** (AppKit monitor), measured in T-M1 itself.

**Step 3: the platform decision.**

| Result of Step 2 | Platform path |
|---|---|
| Every row resolves to W | **W** |
| One or more rows resolve to native, and one candidate passes all of them | **W + that candidate** for those attributes (hybrid: the DOM keeps positions and hover) |
| A row needs native, and its pending experiment (E-H1, E-D1) has not run | run that experiment; then apply this table again |
| A row resolves to a documented limitation | that path, with the limitation listed in the platform notes and reported to the user |
| Any row ends in **platform no-go** | no-go for tablet quality on that platform in this shell: escalate |
| **Both** platforms no-go | decision 7: Qt 6 is the only alternative |
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
