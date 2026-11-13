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
