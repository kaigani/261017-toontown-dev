//! Ruling D-50 (C4.3b closure): dev-only delays of two answers the canvas waits on, so the preview lifecycle can be
//! checked through the real window without luck. Present only in a build with the `test-hooks` feature (never in a
//! product build: `scripts/check-release-features.py` proves `test-hooks` absent from the `tt-desktop` product graph,
//! plan §3.2). Without the feature both functions answer `None` and nothing is delayed.
//!
//! - `TT_TEST_COMMIT_DELAY_MS`: a gesture commit's answer (`gesture_commit`) is delivered this many milliseconds after the
//!   kernel answered it. The commit itself is not delayed: the document changes at once, only the window learns late.
//! - `TT_TEST_SCENE_DELAY_MS`: a scene answer (`canvas_scene`) is delivered this many milliseconds after the kernel built
//!   it, so a scene requested before a commit, built at the earlier revision, can arrive after the commit was made.
//!
//! Both are read from the environment when the application starts; turn them on by building with
//! `cargo build --release -p tt-desktop --features test-hooks` and launching with the variables set.

use std::time::Duration;

#[cfg(feature = "test-hooks")]
fn read(name: &str) -> Option<Duration> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<std::collections::BTreeMap<String, Option<Duration>>> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        ["TT_TEST_COMMIT_DELAY_MS", "TT_TEST_SCENE_DELAY_MS"]
            .iter()
            .map(|n| {
                let d = std::env::var(n).ok().and_then(|v| v.trim().parse::<u64>().ok()).filter(|ms| *ms > 0);
                ((*n).to_owned(), d.map(Duration::from_millis))
            })
            .collect()
    });
    all.get(name).copied().flatten()
}

/// The delay before a gesture commit's answer reaches the window, when the hook is on.
#[must_use]
pub fn commit_delay() -> Option<Duration> {
    #[cfg(feature = "test-hooks")]
    {
        read("TT_TEST_COMMIT_DELAY_MS")
    }
    #[cfg(not(feature = "test-hooks"))]
    {
        None
    }
}

/// The delay before a scene answer reaches the window, when the hook is on.
#[must_use]
pub fn scene_delay() -> Option<Duration> {
    #[cfg(feature = "test-hooks")]
    {
        read("TT_TEST_SCENE_DELAY_MS")
    }
    #[cfg(not(feature = "test-hooks"))]
    {
        None
    }
}
