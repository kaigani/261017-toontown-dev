# ADR-0009 — Tablet input architecture

Status: **proposed; not decided.** C0.4, 2026-10-18. The decision is conditional on two measurements nobody has yet:
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

**Windows**, after one real-tablet session (Wacom or Windows Ink pen; E1 run alongside):

| Outcome with real hardware | Windows path |
|---|---|
| The DOM delivers stationary and pressure-only updates, early frames and full twist | **W** |
| The DOM loses them, and raw HID decodes to usable pressure/tilt | **H** (DOM keeps positions and hover; HID supplies pressure/tilt/twist) |
| The DOM loses them, and raw HID is unusable, but WM_POINTER/RTS on an overlay window receives them | **D** |
| None of W, H or D delivers usable pressure with a real pen | no-go for tablet quality on Windows in this shell; escalate (decision 7: Qt 6 only if Tauri fails on **both** OSes) |

**macOS**, after the user's run:

| Outcome of the manual test | macOS path |
|---|---|
| WKWebView pointer events carry pen type, pressure and tilt from the tablet (not just Force Touch force) at ≥ 200 Hz with coalesced events | **W** |
| WKWebView loses pressure, tilt or pen type, and the AppKit monitor receives them | **A** |
| Neither path delivers tablet pressure | no-go for tablet quality on macOS in this shell; escalate as above |

Combined: if both platforms end in no-go, plan §2.2 / decision 7 applies (Qt 6 is the only alternative). Any other
combination keeps the Tauri shell.

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
