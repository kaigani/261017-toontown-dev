//! M4 C4.3 builder tests of `tt-executor`, one test binary for the checkpoint (orchestrator ruling D-2, kept for C4.3).
//! None of these tests sets a process-wide hook, counter or environment variable; each executor works in its own
//! temporary user folder, so they run in parallel. Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

#[path = "c43/camera.rs"]
mod camera;
#[path = "c43/labels.rs"]
mod labels;
#[path = "c43/mapping.rs"]
mod mapping;
#[path = "c43/renders.rs"]
mod renders;
#[path = "c43/scene_tools.rs"]
mod scene_tools;
