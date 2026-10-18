//! Permanent C0.1 regressions, wired into `cargo test --workspace` through tt-headless.

#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;
use tt_engine::{Engine, EngineConfig, RunMode};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn node_case(case: &str) {
    let output = Command::new("node")
        .args(["tests/adversarial/guards.mjs", case])
        .current_dir(root())
        .output()
        .expect("Node is a documented development prerequisite");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn c01_r1_01_license_grouping_preserves_mandatory_copyleft() {
    node_case("licensing");
}

#[test]
fn c01_r1_01_license_allowlist_stays_within_user_policy() {
    node_case("allowlist");
}

#[test]
fn c01_r1_02_dynamic_import_cannot_bypass_ipc_boundary() {
    node_case("imports");
}

#[test]
fn c01_r1_04_info_is_a_superset_of_app_info_get() {
    let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless }).unwrap();
    let value = serde_json::to_value(engine.info()).unwrap();
    assert!(value["locale"].is_string(), "01 section 12.1 / A: app.info.get requires locale; got {value}");
    assert!(value["languages"].is_array(), "app.info.get requires languages[]");
}

#[test]
fn c01_r1_05_rust_control_rejects_multibyte_over_limit() {
    let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless }).unwrap();
    let error = engine.ping(tt_engine::PingRequest { seq: 1, payload: Some("é".repeat(524_289)) }).unwrap_err();
    assert_eq!(error.to_info().details["value"], 1_048_578);
}

#[test]
fn c01_r1_06_dependency_proof_rejects_graphics_dependencies() {
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg("tests/adversarial/headless_guard.py")
        .current_dir(root())
        .output()
        .expect("Python is a documented development prerequisite");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn wire_engine_info_field_names_match_typescript() {
    let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless }).unwrap();
    let value = serde_json::to_value(engine.info()).unwrap();
    let types = include_str!("../../desktop/src/ipc/types.ts");
    let body = types.split("export interface EngineInfo {").nth(1).unwrap().split('}').next().unwrap();
    let mut declared: Vec<_> =
        body.lines().filter_map(|line| line.trim().split_once(':').map(|(name, _)| name)).collect();
    declared.sort_unstable();
    let mut actual: Vec<_> = value.as_object().unwrap().keys().map(String::as_str).collect();
    actual.sort_unstable();
    assert_eq!(actual, declared, "hand-written Rust/TS wire keys have drifted");
}
