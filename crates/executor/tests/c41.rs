//! M4 C4.1 builder tests of `tt-executor`, one test binary for the checkpoint (orchestrator ruling D-2: every file
//! directly under `tests/` is its own executable linked against the workspace). None of these tests sets a process-wide
//! hook, counter or environment variable; each works in its own temporary user folder, so they run in parallel.
//! Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

#[path = "c41/busy.rs"]
mod busy;
#[path = "c41/kernel.rs"]
mod kernel;
#[path = "c41/keymap.rs"]
mod keymap;
#[path = "c41/restart.rs"]
mod restart;
#[path = "c41/settings.rs"]
mod settings;
