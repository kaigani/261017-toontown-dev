# The window's budget receipt (plan §6.2; C5.1b round 1 observation)

- `report.json`, `manifest.json`: the receipt, from the run on the diagnostics build of `c43422a3` (23 Nov, 21:44).
  It reopened the scene a previous run of the same harness had built and saved in this folder, so its
  `buildSeconds` (4.45 s) is the reopen, not the build.
- `scene-build-run.log`: that previous run (23 Nov, 21:20 to 21:34, a diagnostics build with the work folder's fix, committed
  as `19c51c00`), which built the scene through the window's commands in 792 s (`buildSeconds` 792.49) and measured the
  first complete window rows (the four misses the handoff names, before the read discipline of `c43422a3`). Its own
  report in this folder was overwritten by the later runs; this is its log, kept for the build time. The scene package
  itself was moved out of the evidence (4.8 MB).
