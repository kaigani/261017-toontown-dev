# ADR-0005 — Platform floor

Status: accepted (C0.1 review round 3, orchestrator ruling 2026-10-17); re-confirmed in C0.4 (2026-10-18) with the
C0.2/C0.3 evidence below as inputs. Open question for the user below.

## Context

The spec sets no minimum OS or webview version. Earlier C0.1 work declared one implicitly in three places: Vite
`build.target` `safari15`/`chrome110`, Tauri `bundle.macOS.minimumSystemVersion` `12.0`, and the WebView2 evergreen
runtime on Windows. Review round 3 (C01-R3-05) found the IPC validator using `Object.hasOwn` and `structuredClone`,
both new in Safari 15.4 (macOS 12.3), so the declared floor and the code disagreed. A syntax target does not polyfill
missing runtime functions.

## Decision

1. **Floor:**
   - **macOS 12.0 (Monterey) with its WKWebView, Safari 15.0.**
   - **Windows 10 version 1809 or later with the evergreen WebView2 runtime.** WebView2 updates itself;
     **Chrome 110** is used as a conservative API floor.
   - The CI builds for macOS are Apple Silicon only (see the C0.1 handoff); the floor is about OS and webview
     versions, not CPU.
2. **Single declaration, checked for agreement.** The floor is declared as:
   - `browserslist` in `desktop/package.json` (`safari 15`, `chrome 110`);
   - the Vite `build.target` (`es2022`, `chrome110`, `safari15`);
   - Tauri's `minimumSystemVersion` (`12.0`).

   `desktop/src/test/platformFloor.test.ts` fails if they disagree.
3. **Enforced in the lint gate.** `eslint-plugin-compat` (`compat/compat`) checks application source (`src/**`,
   excluding tests, which run in Node) against the browserslist targets. Examples it rejects: `Object.hasOwn`,
   `structuredClone`, `Array.prototype.at`. Syntax newer than the target is down-levelled by Vite. Runtime APIs are
   not, so code must use floor-safe equivalents: `Object.prototype.hasOwnProperty.call`, the own plain-data copy in
   `src/ipc/schema.ts`, and so on.
4. **Regression tests.** A unit test removes `Object.hasOwn` and `structuredClone` together and certifies a real host
   response through `createHostApi(...).ping()`. The reviewer's tests cover each API on its own.

## Consequences

- Newer web APIs need either a floor-safe implementation or an ADR amendment raising the floor.
- No macOS 12.0 machine runs in CI (`macos-latest` is newer). The floor is enforced statically and by simulation,
  not by execution on the oldest supported system.
- A transparent `Proxy` cannot be told apart from plain data without `structuredClone`. The validator copies such a
  value into validated plain data and never throws. Rejecting proxies outright is ticketed to M1 C1.1 (C01-R3-04).

## Open question for the user

**Raise the floor to macOS 13 (Ventura, Safari 16)?** That would allow `structuredClone`, `Object.hasOwn`, `Array.at`
and other Safari 15.4–16 APIs. It would drop Monterey 12.0–12.x users. The recommended default is to keep macOS 12.0
until there is a concrete need.

## C0.4 evidence for the open question (inputs, not a decision)

**The floor stays at macOS 12.0 / Safari 15.0 and Windows 10 1809 + evergreen WebView2 (Chrome 110 API floor).**
The M0 spikes add these inputs to the user's pending decision:

1. **Coalesced pointer events need WebKit 18.2.** `getCoalescedEvents`, `getPredictedEvents`, `altitudeAngle` and
   `azimuthAngle` arrive in Safari/WebKit 18.2 (MDN browser-compat data, WebKit release notes); pressure, tilt and
   twist properties exist since WebKit 13 (`reviews/M0-C0.2-handoff.md`, finding 3).
   - At the macOS 12 floor, a web-only tablet path gets one sample per dispatch, not the batched high-rate stream the
     Windows runner measured (p50 4 samples per dispatch).
   - The C0.2 page feature-detects the API, so this lowers sample density; it does not break the page.
   - Raising the floor for this alone would mean requiring WebKit 18.2, not macOS 13. Which macOS versions receive
     that WebKit (WKWebView uses the system WebKit, which Safari updates also replace) has to be checked before any
     such change; it is not established here.
2. **WKWebView pressure provenance is unknown.** Wacom reports no pen pressure in Safari 15 on macOS 12.1. Current Mac
   WebKit derives mouse-event force from `NSEventTypePressure` (Force Touch), not from tablet pressure. Whether a
   tablet's pressure reaches WKWebView at any version is what the user's Mac run measures
   (`docs/manual-tests/M0-C0.2-tablet-macos.md`). If it does not, ADR-0009 candidate A (the AppKit monitor in the
   host) applies, and that does not depend on the WebKit version.
3. **The macOS CI runner has no WebGPU adapter.** It runs macOS 26.6.2, WebKit 21624.5.1.11.3, in a VM: `navigator.gpu`
   is present but returns no adapter (`reviews/M0-C0.3-handoff.md`). WebGL2 on its paravirtual GPU reached 22.1 /
   60 / 60 fps on the three C0.3 scenes. WebGL2 is mandatory (ADR-0007), so the floor does not depend on WebGPU.
4. **No oldest-floor runtime evidence exists.** CI runs only `macos-latest`, which is far newer than 12.0. The floor
   is enforced statically (`eslint-plugin-compat`, `platformFloor.test.ts`), not by execution (Consequences above).
   The C0.2/C0.3 closures list "macOS/WebKit floor" among the unmeasured items.

What would change the recommendation:
- If the user's Mac run shows that tablet input needs WebKit ≥ 18.2 on the **web** path **and** the AppKit native
  path also fails, the floor question becomes a tablet-quality question (ADR-0009).
- Otherwise the recommended default stays: keep macOS 12.0 until a concrete need appears.
