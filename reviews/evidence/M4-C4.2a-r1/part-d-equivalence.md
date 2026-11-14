# Part D review receipt

Product: c745a55. Windows 11. Reviewer additions only.

`cargo test -p tt-executor --test c42a_r1_review --locked --features tt-persist/test-hooks part_d_equivalence -- --nocapture`: 18 tests, 17 passed, 1 failed.

Failure: part_d_equivalence::zoom_marquee_fits_region. Actual zoom 1.5; expected 9.0. Requirement 02 section 7.5 line 658.

`cargo clippy -p tt-executor --all-targets --locked --features tt-persist/test-hooks -- -D warnings -A clippy::empty_line_after_doc_comments`: passed.

Coverage, derivations, per-tool verdicts and method limitations are in the Part D sections of the main review. No blind independence or complete coverage is claimed. No raw logs retained.
