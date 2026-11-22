# M4 C4.1 evidence

| Folder | What | Produced by |
|---|---|---|
| `local-windows/` | The real application (product front end embedded, debug profile) on a Windows 11 workstation, driven over attached mode (Streamable HTTP, bearer token) | `python scripts/c41-gui-e2e.py <toontown.exe> <out>` |
| `ci-gui-windows/` | The same script and the HTTP conformance run on the `bundle` product build, Windows runner | CI job `desktop gui (windows-latest)`, artifact `desktop-gui-Windows` |
| `ci-gui-macos/` | The same on the macOS runner | CI job `desktop gui (macos-latest)`, artifact `desktop-gui-macOS` |

Files of one GUI run:

- `manifest.json`: OS, duration, number of checks, failed checks, screenshots, file list.
- `transcript.json`: every step in order: the command or tool call, its answer, each check with its detail.
- `tree-01-start.json`, `tree-02-project.json`, `tree-04-restart.json`: the UI tree the front end rendered and reported
  (`inspect_ui`), at the start screen, after a project was created and after a restart over the same user folder.
- `01-start-screen.png` … `04-restart.png`: captures of the application window only (Windows: the window's visible
  frame; macOS runners: best effort). Not gated; the UI trees are the gated receipt.
- `activity.json`: the agent activity log of the first session (`agent.activity.list`).
- `discovery.json`: the attached-mode discovery file, token redacted; `discovery.log`: the endpoints of the three
  launches.
- `mcp-conformance-http.json` (CI only): the official Python SDK client's protocol checks over Streamable HTTP.
