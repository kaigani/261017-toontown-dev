# M4 manual checks (user-pending)

Short scripts for things CI runners cannot show: real menus and keyboards, OS dialogs, multiple monitors, the OS
appearance switch, a real agent client. Each takes a few minutes. None is an acceptance condition of the spec; they are
listed in each handoff as user-pending. Record the outcome in the "Result" section of each file and commit it, or tell
the orchestrator.

Build to use: the `desktop-<OS>` artifact of the CI run named in the handoff (`out/default-build/`), or
`pnpm --dir desktop tauri build` locally.

| Script | Platform | Checkpoint |
|---|---|---|
| [C4.1-macos-menu-and-shortcuts.md](C4.1-macos-menu-and-shortcuts.md) | macOS | C4.1 |
| [C4.1-windows-menu-and-shortcuts.md](C4.1-windows-menu-and-shortcuts.md) | Windows | C4.1 |
| [C4.1-single-instance-and-open-document.md](C4.1-single-instance-and-open-document.md) | both | C4.1 |
| [C4.1-native-dialogs.md](C4.1-native-dialogs.md) | both | C4.1 |
| [C4.1-window-geometry-and-full-screen.md](C4.1-window-geometry-and-full-screen.md) | both | C4.1 |
| [C4.1-theme-follow-system.md](C4.1-theme-follow-system.md) | both | C4.1 |
| [C4.1-attached-agent.md](C4.1-attached-agent.md) | both | C4.1 |
| [C4.1-keyboard-and-focus.md](C4.1-keyboard-and-focus.md) | both | C4.1 (round 1) |
| [C4.1-display-scaling.md](C4.1-display-scaling.md) | both | C4.1 (round 1) |
| [C4.1-dialog-then-shortcut.md](C4.1-dialog-then-shortcut.md) | both | C4.1 (round 1) |
| [C4.1-floating-windows.md](C4.1-floating-windows.md) | both | C4.1 (round 1, ruling O-2) |
| [C4.2-tablet.md](C4.2-tablet.md) | both | C4.2b |
| [C4.2-visual-fidelity.md](C4.2-visual-fidelity.md) | both | C4.2b |
| [C4.2-feel.md](C4.2-feel.md) | both | C4.2b |
| [C4.2-navigation.md](C4.2-navigation.md) | both | C4.2b |
| [C4.2-display-scaling.md](C4.2-display-scaling.md) | both | C4.2b |
| [C4.2-gpu-less.md](C4.2-gpu-less.md) | both | C4.2b |
| [C4.3-camera-view.md](C4.3-camera-view.md) | both | C4.3b |
| [C4.3-scene-tools.md](C4.3-scene-tools.md) | both | C4.3b |
| [C4.3-coexistence.md](C4.3-coexistence.md) | both | C4.3b |
| [C4.4a-keyboard.md](C4.4a-keyboard.md) | both | C4.4a |
| [C4.4a-agent.md](C4.4a-agent.md) | both | C4.4a |
| [C4.4a-open-recent-full-path.md](C4.4a-open-recent-full-path.md) | both | C4.4a |
| [C4.4b-text-input-method.md](C4.4b-text-input-method.md) | both | C4.4b |
| [C4.4b-tools-pen-and-macos.md](C4.4b-tools-pen-and-macos.md) | both (pen or tablet; macOS) | C4.4b |

**Keystroke to glyph on a display without a frame cap** (C4.4b, ruling D-76 item 2): on a display faster than 60 Hz
or with the cap off, run `python scripts/c44b-measure.py <toontown binary> <out dir> --backend webgl2 --text-only`,
then with `--backend webgpu`. The bound is two display frames of the run's own control median; `measure.json`'s
`keyToGlyphBound` gives the control median, the bound and the margin. Record the figures and the display.
