# Implementation plan — 2-D animation application (spec v2, 2026-10-17)

Status: proposal for approval. Author: orchestrator (Claude Fable 5.1). Builder: Claude Opus 5.5 (effort high).
Adversarial reviewer: Codex GPT-6-Astra. Spec: `../spec/00_overview.md` … `11_agent_interface_mcp.md`, `A_command_registry.md`.

---

## 1. What the spec asks for (assessment)

| Measure | Value |
|---|---|
| Spec size | 13 files, 14 403 lines |
| Registered commands and events | 796 across 35 namespaces (`anim` 101, `fx` 61, `draw` 51, `colour` 51, `library` 46 …) |
| Element types | 17 (drawing, image, sound, media, annotation, peg, camera, skeleton, bone, 5 effects, 2 particle effects, effect parameter) |
| Panels | 21 dockable panels + 5 tool palettes, 4 built-in workspaces |
| Output paths | interactive playback, rendered preview, export (MP4/MOV/GIF/sequences/PDF/print/publish), deterministic agent render |
| Conformance gates | parity (every GUI action bound to a command id), schema, round-trip, gesture equivalence, determinism (byte-identical renders), never-block, MCP protocol, 7 end-to-end agent scenarios |

This is a complete mid-range animation suite (vector drawing with planar-map painting, X-sheet and timeline, 3-D
staging with multiplane cameras, cut-out rigs with bones, particles, lip-sync, library, storyline, export) plus a
full MCP control surface. Four requirements shape the architecture more than any feature:

1. **One command layer (11 P2).** GUI and agents are both clients of one registry + one serialized executor. The GUI
   may not touch the document by any other path, and a build-time parity test enforces it.
2. **Headless mode without display or GPU (11 §3)** with a deterministic software rasteriser for every offscreen
   render. The whole engine must therefore live in a native library with no dependency on the UI framework.
3. **One JSON object model = file format = agent API (10 DM-1).** One serializer, one set of JSON Schemas, canonical
   JSON, byte-identical saves, revisions and change sets on every object.
4. **Determinism everywhere (11 P7, 06 §4.6, 02 §3.3).** Curve fitting independent of timing, in-house PRNG for
   particles, fixed floating-point operation order, byte-identical renders across runs and restarts.

Consequence: roughly 85 % of the work (and all of the hard, risky work) is engine code that has nothing to do with the
window toolkit. The UI framework choice matters for tablet input, canvas performance and docking, and should be
isolated so it can be swapped.

Scope notes from the spec that reduce v1 work: SWF import/export is MAY (omit); publishing services ship with none
configured (framework only); scanner and tethered-camera capture are platform-specific and can follow webcam capture;
the starter library (hundreds of authored templates) is a content workstream, not code.

---

## 2. Stack recommendation

### 2.1 Recommendation: Rust engine + Tauri 2 shell + TypeScript front end

Agree with the Tauri / Rust suggestion, with the architecture split so the recommendation is really "Rust engine
first, Tauri as the current shell":

| Layer | Technology | Why |
|---|---|---|
| Engine (document, commands, persistence, vector, animation, effects, final renderer, audio, MCP, headless CLI) | Rust workspace of library crates + two binaries (`headless`, desktop host) | Deterministic f64 maths, no GC pauses in brush/particle paths, single static binary for headless/CI, mature crates for every subsystem (below), compiles unchanged for Windows and macOS. |
| Desktop shell | Tauri 2 (WebView2 on Windows, WKWebView on macOS) | Native menus, dialogs, single-instance, open-document events, sidecars, small installers; the Rust engine runs in-process so IPC is a function call plus serialization. |
| Front end | TypeScript + React, `dockview` for docking/tabs/floating, virtualised grids for Timeline and X-sheet | 21 dockable panels, a key-binding editor, rich-text tool options and a preferences UI are web-UI strengths; dockview (MIT) already implements the docking/tabbing/floating/serialise behaviour of 01 §5.4–5.5. |
| Interactive canvas | Render list produced in Rust → WebGPU (WebGL2 fallback) in the webview; in-progress gestures drawn as a local overlay, committed as one structured command on pointer-up | Matches 06 §6.1 (both renderers consume the same render list) and 10 REV-06 (a gesture is one transaction when it ends). Keeps the document single-sourced in Rust. |
| Final / agent renderer | CPU rasteriser in Rust: evaluate `vello_cpu` first, own exact-area coverage rasteriser as fallback | Byte-identical output, no GPU, runs in headless mode and CI. |

Key crates (all permissively licensed, offline): `kurbo` (Béziers, fitting, offsets), `lyon` (tessellation),
`i_overlay` (boolean ops for brush outlines), `serde_json` + `schemars` + `jsonschema` (object model and schemas),
`ulid`, `zip` (ZIP64), `symphonia` (WAV/AIFF/MP3/AAC/FLAC/Vorbis decode), `cpal` (audio out), `rmcp` (official MCP
SDK, stdio + Streamable HTTP), `wgpu` (optional native interactive renderer), `parley`/`swash` + `fontdb` (text
shaping and outlines), `image` + `psd` (bitmaps, layered PSD), `pdfium-render` (PDF/AI import), `usvg` (SVG import),
`pdf-writer` (PDF export), `nokhwa` (webcams), `ort` (ONNX runtime for lip-sync), `notify` (file watching),
`octotablet` (native tablet input fallback).

Video encode/decode: ship an **LGPL FFmpeg build as a Tauri sidecar** using platform hardware encoders
(`h264_mf` on Windows, `h264_videotoolbox` on macOS) so no GPL or patent-encumbered software encoder is bundled;
decode movie imports with the same sidecar. This is a licensing decision for the user (§6).

Lip-sync detector (07 D6): `ort` + a CTC phoneme model that emits IPA directly (e.g. `wav2vec2-lv-60-espeak-cv-ft`,
Apache-2.0, ~80 MB int8). Transcript-assisted mode = CTC forced alignment over the same emissions. Deterministic on
CPU. Fallback when the model is absent: the acoustic-cue classifier the spec describes (loudness → B/C/D, F2 →
E/F, frication → G/B), which also serves as the "always plausible" floor.

### 2.2 Honest comparison of alternatives

| Option | Strengths | Why not first choice |
|---|---|---|
| **Qt 6 (C++, or Rust via cxx-qt)** | Best native docking (KDDockWidgets), first-class tablet events with pressure/tilt, printing, menus; what shipping animation tools use | Second language in the critical path, LGPL dynamic-linking constraints or commercial licence, slower iteration for agent-driven development. Strongest technical alternative if the Tauri input spike fails on both platforms. |
| **Electron + Rust** | Same architecture as Tauri with Chromium on both OSes | **Excluded by user decision (2026-10-17). Not a fallback.** If the Tauri shell fails on both platforms, Qt 6 is the only alternative. |
| **Pure Rust GUI (egui, iced, Slint, Dioxus)** | One language, no IPC | Docking, rich text, accessibility and complex forms are immature; 21 panels + key-binding editor + preferences is a lot of bespoke widget work. |
| **Flutter + Rust (flutter_rust_bridge)** | Good canvas, decent tablet input | Dart as a third language, weak desktop docking ecosystem. |

### 2.3 Risks specific to Tauri and how the plan retires them (M0)

| Risk | Mitigation |
|---|---|
| Pressure/tilt/coalesced pointer events in WKWebView with Wacom/Apple tablets | M0 spike on a Mac. Fallback: `octotablet` native tablet input in the Rust host, streamed to the front end over a Tauri channel. |
| Canvas throughput for 100 k-stroke drawings and 1 000-element scenes | M0 spike: render-list → WebGPU/WebGL2 at 60 fps; binary IPC (`tauri::ipc::Response`) bandwidth test. Fallback: native `wgpu` child surface. |
| WebGPU availability in WKWebView on older macOS | WebGL2 path mandatory from day one. |
| Determinism of the CPU rasteriser across machines | M0: render fixture set on Windows and macOS, compare SHA-256. Pick `vello_cpu` or own rasteriser on evidence. |
| Printing and scanners have no cross-platform crate | Print = our page layout → PDF → OS print API (`windows` crate GDI/XPS, `objc2` NSPrintOperation). Scanners/tethered cameras deferred to the end of M10. |

### 2.4 This machine

Rust 1.98 stable (MSVC), VS Build Tools 2022, WebView2 runtime, Node 22, pnpm 10, Python 3.13, gh CLI are
installed, so Windows builds work immediately. A Tauri macOS app cannot be built or tested from Windows. Decided
2026-10-17: GitHub Actions `macos-latest` runners build macOS artifacts, and the user pulls the repo onto their Mac to
run the tablet spike and any macOS-only manual checks. The builder writes those as step-by-step scripts under
`docs/manual-tests/` and never claims a macOS result it has not received.

---

## 3. Architecture

### 3.1 Repository layout (Cargo workspace + pnpm workspace)

```
<repo>/
  Cargo.toml                 workspace
  crates/
    model/        object model types, envelope, ULIDs, revs, change sets, canonical JSON, schemas (schemars), validation
    registry/     command declarations (attribute macro), enablement, scopes, examples; registry version
    executor/     document thread, transaction queue, undo/redo, checkpoints (copy-on-write), batch, dryRun, expectRev, requestId
    vector/       fitting, brush outline sweep, planar map (noding, zones), gap closing, flatten/optimize/smooth, centre line, text outlines, tracing
    colour/       palettes, styles, swatches, recovery, gradient/texture mapping
    anim/         transform maths (05 §2), TCB paths, arc length, velocity, channels, easing, bones + LBS skinning, cameras, per-frame evaluation
    fx/           effect scopes, particle engine with in-house PRNG, presets
    render/       render-list compiler; render-soft (deterministic final renderer); mesh export for the web renderer; hit-test ids
    audio/        decode (symphonia), peaks cache, mixer + limiter, output (cpal), scrubbing
    lipsync/      ONNX phoneme CTC, forced alignment, acoustic fallback, phoneme map, smoothing, quantisation
    persist/      .animpkg zip/folder, manifest, atomic save, backups, autosave, recovery, locks, migrations; .animtpl/.animpal/.animlib
    io/           image/PSD/PDF/SVG import, PNG/JPEG/TGA/BMP/TIFF/GIF sequence export, PDF export, FFmpeg sidecar, print, capture
    mcp/          rmcp server: generic tools, resources, prompts, sessions, permissions, confirmation, audit log, discovery file
    engine/       facade: app state, settings, virtual views, jobs runner, events bus, areas 01–09 command implementations
  bins/
    headless/     app --headless --mcp-stdio | --run batch.json
  desktop/        Tauri 2 app (Rust host embeds engine; TS front end)
    src-tauri/
    src/          React + dockview; views; panels; ui-bindings manifest (command ids)
  content/        default palette, pens, presets, mouth templates, starter library (authored separately)
  tests/
    fixtures/     projects, golden images, migration fixtures
    conformance/  parity, schema, round-trip, gesture-equivalence, determinism, never-block, protocol, E1–E7
    adversarial/  tests written by the reviewer (kept)
  docs/decisions/ ADR-NNNN-*.md (one per design decision taken from the spec's lists)
  reviews/        M<N>-r<k>.md review reports and responses
  plan/           this file (copied), traceability matrix
```

Repository location (decided 2026-10-17): `~/Documents/PROJECTS/261017-toontown-dev/`
(`C:\Users\kaiga\Documents\PROJECTS\261017-toontown-dev` on Windows), outside the Syncthing folder, with a private
GitHub remote that the user also pulls onto a Mac. The authoritative spec stays in the Syncthing folder; the repo
carries a read-only copy of `spec/` that only the orchestrator refreshes.

### 3.2 Non-negotiable engineering rules (enforced by CI from M1)

- Every document mutation goes through the executor; `model` types have no public mutators outside `executor`
  (compile-time enforcement via crate visibility). A lint fails CI on direct mutation from `engine` or `desktop`.
- Every command: declared with id, kind, param + result schema, enablement, scopes, undo label, ≥ 1 example; the
  example runs against a fixture in CI (11 §14.2).
- UI bindings are a manifest of command ids; the parity test diffs it against the registry (11 §14.1).
- Golden-image and golden-JSON tests for every renderer and serializer change; determinism tests run twice per CI job.
- Never-block watchdog: the headless binary executes every command example under a timeout.
- Spec traceability matrix (`plan/traceability.csv`): command id → milestone → status → test. Updated per milestone.
- `spec/` is frozen for the builder. Spec defects or ambiguities go to `spec-issues.md` for the user to rule on.

---

## 4. Milestones and checkpoints

Ordering is chosen to retire risk first and to make the application testable headlessly through MCP before any GUI
exists. Sizes: S ≈ a day of agent time, M ≈ several days, L ≈ one to two weeks, XL ≈ more. The critical path is
M0 → M1 → M2 → M3 → M4.

| # | Milestone | Spec | Size | Checkpoints (each gets a Codex review) |
|---|---|---|---|---|
| M0 | Foundations and spikes | 00, 11 §3 | M | C0.1 repo, CI (Windows + macOS), Tauri hello, dockview shell · C0.2 tablet input spike (pressure, tilt, coalesced events) on WebView2 and WKWebView with real tablets · C0.3 canvas spike (render list → WebGPU/WebGL2, 100 k paths, binary IPC bandwidth) and rasteriser determinism across both OSes · C0.4 go/no-go report: Tauri shell confirmed (Electron is excluded; Qt 6 only if Tauri fails on both OSes), `vello_cpu` vs own rasteriser, FFmpeg/encoder licensing |
| M1 | Core kernel | 10, 11, 01 §3 | XL | C1.1 object model, IDs, revs, change sets, canonical JSON, schemas, path addressing · C1.2 registry, executor, undo/redo/checkpoints, batch, dryRun, expectRev, requestId, enablement, `edit.*` · C1.3 persistence: package + folder form, manifest, atomic save, backups, autosave, recovery, locks, migrations skeleton, `project.*`, `doc.*` · C1.4 MCP server: generic tools, resources, subscriptions, events, permissions, confirmation protocol, audit log, headless binary, batch-script mode, docs resources |
| M2 | Vector engine and drawing commands | 02, 03 (model and paint) | XL | C2.1 strokes, curve fitting, brush outline, pencil, invisible, text and bitmap nodes · C2.2 planar map: noding, zones, sides, gap closing, paint by id/point, colour IDs, recovery · C2.3 flatten/optimize/smooth/centre line/convert/feather/arrange/group, `draw.*` and `colour.paint.*` commands, `render_drawing` · C2.4 vectorize (trace + fit), topology property tests, E2 headless |
| M3 | Elements, timing, staging maths, final renderer | 04 (model), 05 §1–2, §8–10, §12, 06 §6, 09 §11 | XL | C3.1 element types, exposure runs, cells, clone/duplicate, hierarchy, `element.*`, `cell.*`, `xsheet.*` JSON · C3.2 transforms, pivots, TCB paths, velocity, channels, easing, cameras, projection, `anim.evaluate`, E4 numeric · C3.3 compositor, render list, deterministic `render_frame`, contact sheet, `describe_frame`, `hit_test`, `convert_coordinates`, PNG sequence export, golden images · C3.4 E1 headless (minus MP4) |
| M4 | Desktop shell and first interactive views | 01, 02 §4–§10, 11 §6.4 | XL | C4.1 start screen, project wizard, menus, prefs, keymap editor, themes, workspaces, docking, history panel, status bar, agent indicator, messages, settings persistence · C4.2 interactive renderer in the webview, Drawing view with brush/pencil/select/eraser/paint/shape tools, pens, onion skin, grids, light tables; gesture equivalence; `pointer`/`key`/`inspect_ui`/`screenshot` · C4.3 Camera view with select/transform; parity test gate; E6 coexistence |
| M5 | Timeline and Exposure Sheet | 04 (views), 01 §6.2–6.4 | L | C5.1 Timeline (rows, bars, keyframes, ruler, zoom, selection, drag/drop, paste special) · C5.2 Exposure Sheet (thumbnails, display modes, all cell commands, cycles, columnate, drawing management, static light table), Properties/Cells panels |
| M6 | Staging and animation complete | 05 | XL | C6.1 Top/Side views, display options, all animation tools, peg-only mode, flips, hierarchy commands · C6.2 motion path UI, keyframes, function editor, velocity, easing, cameras UI, motion recording · C6.3 bones: tools, kinematics, IK, skinning, zones of influence, skeleton properties; E4 interactive |
| M7 | Colour and effects complete | 03 (UI), 06 | L+L | C7.1 palette panel, colour editor, styles, tint offset/blend, gradients, textures, edit-texture tool, `.animpal` · C7.2 effect elements and parameter rows, colour transform, clipping, drop shadow, blur, shake, presets, AA clipping/flatten/blur in final renderer · C7.3 particle engine, all particle types and presets, effect preview panel, determinism goldens across restarts |
| M8 | Sound, lip-sync, media | 07 | L | C8.1 import/decode/storage, waveform cache, mixer, playback sync, scrubbing, Timeline/X-sheet sound display, sound editor · C8.2 lip-sync detector (model + fallback + transcript alignment), track model, mapping, apply, panel, mouth templates; accuracy corpus; E3 · C8.3 media elements, movie import as sequence |
| M9 | Library and storyline | 08 | L | C9.1 libraries, folders, items, templates (.animtpl), create/insert copy-link, edit mode, locking · C9.2 scene manager, storyline panel, transitions, timing model, storyline playback, transition rendering; E5 · C9.3 starter-content pipeline (loader, thumbnails, validation; content authored separately) |
| M10 | Output and input complete | 09 | L | C10.1 FFmpeg sidecar: MP4/MOV/GIF export, audio mux, rendered preview + player, snapshot, export reports; E1 full · C10.2 PDF export, print layout/preview/PDF/OS print, vector import (PDF/AI/SVG), PSD layers, keyed image import, publishing framework (descriptors, OAuth in system browser, OS credential store; no service shipped) · C10.3 capture: webcams via nokhwa, keying, light table, auto-capture; scanners and tethered cameras as stretch |
| M11 | Hardening, conformance, packaging | 11 §14, 10 §5 | L | C11.1 full conformance suite green on both OSes, performance targets (1 000 elements, 100 k strokes, 10 k drawings, 10 k swatches), fuzzing of the command surface and package reader · C11.2 installers (MSI/NSIS, DMG, signing, notarisation), auto-update channel, crash reporting opt-in, migrations fixtures, docs, prompts, typed-tools option, localisation scaffolding |

Each milestone ends with: CI green on Windows and macOS, its conformance subset passing, traceability matrix
updated, ADRs written for every design decision taken, no open P0/P1 review findings, a git tag `m<N>`.

---

## 5. Build and review protocol (Opus 5.5 builds, Codex GPT-6-Astra reviews)

Verified against the T3 catalog on this machine: builder = provider `claudeAgent`, model `claude-opus-5-5`,
effort `high`; reviewer = provider `codex`, model `gpt-6-astra`, reasoning `high` (raise to `xhigh` for M1 and M3,
which carry the determinism and command-layer guarantees everything else relies on).

### 5.1 Mechanics

1. The orchestrator thread launches **one Opus thread per milestone** in its own worktree
   (`t3_thread_launch`, `workspaceStrategy: worktree`, branch `m<N>-<slug>` off `main`). The message contains: the
   milestone row above, the exact spec sections, the engineering rules of §3.2, the checkpoint list, and the
   definition of done. Opus commits per checkpoint and writes `reviews/M<N>-C<k>-handoff.md` (what was built, how to
   run it, known gaps).
2. At each checkpoint the orchestrator runs a **Codex review round** as a delegated task (`delegate_task`, provider
   `codex`, model `gpt-6-astra`) in a worktree on the same branch. Codex gets the review charter (§5.2), the handoff
   note, the diff range and the spec sections. It may run anything, write tests, and drive the headless MCP server,
   but it does not fix product code.
3. Opus receives the findings and answers each one in `reviews/M<N>-C<k>-r<j>-response.md`: fixed (commit), disputed
   (with spec citation), or deferred (ticket with milestone). Codex re-reviews; rounds continue until no P0/P1 remain
   or three rounds have run, after which the orchestrator rules or escalates disputes to the user.
4. The milestone branch merges to `main` after the last checkpoint passes; `main` is always buildable on both OSes.
5. Every review round is a new `delegate_task` with the full history (brief, prior findings, responses) in its
   prompt and a distinct `clientRequestId`.

### 5.2 Reviewer charter (adversarial)

Codex is asked to break the checkpoint, not to approve it. Required activities and the report format:

- **Spec conformance sweep.** For each command in the checkpoint's namespaces: registered? schema complete? example
  runs? undo label and behaviour correct? enablement rule matches the section? explicit targets accepted (P4)?
  Missing or divergent items are findings with a spec citation.
- **Functional attack through MCP.** Drive the headless binary: never-block (every command under a watchdog),
  determinism (render twice, hash), concurrency (`expectRev`, interleaved batches), malformed input (unknown params,
  out-of-range, bad refs, cyclic hierarchy), hostile packages (unsafe zip entries, size lies, duplicate ids, dangling
  refs, newer versions), file-system boundary escapes, confirmation and permission bypass attempts.
- **Architecture attack.** Search for document mutation outside the executor, UI state that `get_state` cannot read,
  modal paths reachable by agents, non-seeded randomness, timing-dependent geometry, platform-dependent maths in the
  final renderer.
- **Code review.** Correctness, error handling, unsafe code, dependency licences, performance against the §5 limits.
- **Output.** `reviews/M<N>-C<k>-r<j>.md` with findings ranked P0 (blocks merge) / P1 (fix this milestone) / P2
  (ticket), each with repro steps, spec citation, and where possible a failing test committed under
  `tests/adversarial/`. End with a verdict: `block` or `pass`.

### 5.3 Builder rules

- Work only from `spec/` (read-only) and this plan; never edit the spec. Record each design decision as an ADR with the
  spec's recommended default unless there is a documented reason.
- Tests before merge for every command, renderer and serializer path; keep the traceability matrix current.
- Report blocked items explicitly in the handoff; never narrow a checkpoint silently.

---

## 6. Decisions (resolved by the user on 2026-10-17)

1. **Repository.** `~/Documents/PROJECTS/261017-toontown-dev/` with a private GitHub remote; the user also pulls it
   onto a Mac.
2. **macOS access.** The user runs macOS checks on their own Mac. CI builds macOS artifacts on GitHub Actions.
3. **Video codec licensing.** LGPL FFmpeg sidecar with platform encoders (`h264_mf`, `h264_videotoolbox`), AAC via
   the platform encoder or FFmpeg's native `aac`. No GPL component ships.
4. **Lip-sync model.** Download on first use, with the acoustic fallback always available. Works identically on
   Windows and macOS: the ONNX file is platform-independent data (not code, so no Gatekeeper or signing issue); it is
   fetched over HTTPS from a release asset listed in a signed manifest, verified by SHA-256, stored in the per-user app
   data directory (`%APPDATA%\<app>\models` / `~/Library/Application Support/<app>/models`), and the download is a
   job (`job_*`) that needs the `network` scope. Only the ONNX Runtime library itself is bundled and signed with the
   app.
5. **Scope trims.** Omit SWF; publishing as framework only; scanners and tethered cameras as stretch; starter
   library as a separate content workstream.
6. **Review budget.** Codex `gpt-6-astra` at `high` (`xhigh` for M1 and M3), at most three rounds per checkpoint.
7. **Shell.** Tauri 2. **Electron is excluded outright**; Qt 6 is the only alternative, and only if the M0 spikes
   fail on both platforms.
8. **Working name.** "Toontown" as the placeholder product name (crate prefix `tt`, bundle id `dev.toontown.app`);
   file extensions stay `.animpkg/.animtpl/.animpal/.animlib` as in the spec.
