//! M4 C4.3 builder tests of `tt-executor`, one test binary for the checkpoint (orchestrator ruling D-2, kept for C4.3).
//! None of these tests sets a process-wide hook, counter or environment variable; each executor works in its own
//! temporary user folder, so they run in parallel. Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;
// The package helpers (temporary folders, executors with known user folders, jobs): C43B-R1-03's table opens a
// package with image bytes.
#[path = "c13_support/mod.rs"]
mod c13_support;

#[path = "c43/budgets.rs"]
mod budgets;
#[path = "c43/camera.rs"]
mod camera;
#[path = "c43/labels.rs"]
mod labels;
#[path = "c43/mapping.rs"]
mod mapping;
#[path = "c43/menu_bar.rs"]
mod menu_bar;
#[path = "c43/order.rs"]
mod order;
#[path = "c43/parity.rs"]
mod parity;
#[path = "c43/preview.rs"]
mod preview;
#[path = "c43/renders.rs"]
mod renders;
#[path = "c43/scene_tools.rs"]
mod scene_tools;
#[path = "c43/selection_parts.rs"]
mod selection_parts;
#[path = "c43/spec_values.rs"]
mod spec_values;
