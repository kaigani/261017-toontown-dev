# ADR-0005 — Platform floor

Status: accepted (C0.1 review round 3, orchestrator ruling 2026-10-17). Open question for the user below.

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
