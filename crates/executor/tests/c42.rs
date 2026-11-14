//! M4 C4.2 builder tests of `tt-executor`, one test binary for the checkpoint (orchestrator ruling D-2). None of these
//! tests sets a process-wide hook, counter or environment variable; each executor works in its own temporary user
//! folder, so they run in parallel. Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

#[path = "c42/aids.rs"]
mod aids;
#[path = "c42/budgets.rs"]
mod budgets;
#[path = "c42/equivalence.rs"]
mod equivalence;
#[path = "c42/equivalence_more.rs"]
mod equivalence_more;
#[path = "c42/pointer.rs"]
mod pointer;
#[path = "c42/screenshot.rs"]
mod screenshot;
#[path = "c42/tools.rs"]
mod tools;
#[path = "c42/views.rs"]
mod views;
