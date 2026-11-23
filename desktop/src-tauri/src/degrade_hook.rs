//! Ruling E-29 item 3 (M5 C5.1b): a dev-only way to stop the engine in the running application, so the watchdog's
//! live check (`scripts/c51b-engine-restart.py`) can see the window mark a stopped engine not responding at once, then
//! restart it and recover the autosave through real input. Present only in a build with the `test-hooks` feature (never
//! in a product build: `scripts/check-release-features.py` proves `test-hooks` absent from the `tt-desktop` product
//! graph). Without the feature [`install`] does nothing.
//!
//! - `TT_TEST_DEGRADE_TRIGGER`: a file path. While the file exists, the next document commit unwinds at the commit
//!   boundary (outside the handler's own boundary, as the executor's degraded test does with the same persistence hook),
//!   and the file is removed: the engine marks itself degraded and answers `internal` (reason `engineStopped`) from then
//!   on, until the host restarts it.

/// Installs the hook when the variable names a trigger file (read once, at start-up).
pub fn install() {
    #[cfg(feature = "test-hooks")]
    {
        let Some(trigger) = std::env::var_os("TT_TEST_DEGRADE_TRIGGER").map(std::path::PathBuf::from) else {
            return;
        };
        tt_persist::lock::hooks::set(Some(std::sync::Arc::new(move |at, _path: &std::path::Path| {
            if at == tt_persist::lock::Boundary::Committed && std::fs::remove_file(&trigger).is_ok() {
                // An unwind with no panic message: the executor's boundary takes it as any panic on its thread.
                std::panic::resume_unwind(Box::new(
                    "test hook: the engine stops at a commit (TT_TEST_DEGRADE_TRIGGER)",
                ));
            }
        })));
    }
}
