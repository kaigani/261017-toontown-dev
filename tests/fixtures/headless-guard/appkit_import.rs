//! Negative control for scripts/check-headless-deps.py on macOS: an executable with a real AppKit import.
//! Compiled and inspected by bins/headless/tests/guard_fixtures.rs; never executed.

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    fn NSApplicationLoad() -> bool;
}

fn main() {
    // Retain the import without calling it.
    std::hint::black_box(NSApplicationLoad as *const ());
}
