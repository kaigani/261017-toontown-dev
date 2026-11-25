//! M5 ruling E-62 (C51B-R3S-02): the kernel's text-focus fact is the active window's. A window reports its own fact;
//! which window is active is the operating system's (`Host::window_focused`, from the window's `Focused(true)`
//! event). A report from a window that is not active never makes it the active one and never overrides the active
//! window's fact; when the active window changes, the kernel takes the new window's current fact.
//!
//! The kernel's fact is read through the key dispatch: an unmodified key belongs to a field being edited
//! (`Suspended { reason: "textField" }`), otherwise it reaches the keymap.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs, reason = "test")]

use std::error::Error;

use serde_json::json;
use tt_desktop::host::{Host, HostOptions};
use tt_executor::host::KeyResolution;

fn host() -> Result<(Host, std::path::PathBuf), Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("tt-c51b-r3-focus-{}", tt_executor::random_token(12)));
    let h = Host::start(HostOptions { user_dir: Some(dir.clone()), ..Default::default() })?;
    Ok((h, dir))
}

/// Whether the kernel holds that a text field is being edited (an unmodified key is the field's).
fn editing(h: &Host) -> bool {
    matches!(h.exec().resolve_key("a"), Ok(KeyResolution::Suspended { ref reason, .. }) if reason == "textField")
}

#[test]
fn a_page_change_in_the_inactive_main_window_does_not_end_the_floats_editing() -> Result<(), Box<dyn Error>> {
    // The reviewer's sequence: the rename editor open in the main window, then the person types in a field of a
    // Timeline float, then an agent deletes the main window's row (its editor leaves the page: the main window
    // reports false).
    let (h, dir) = host()?;
    h.window_focused("main");
    h.report_from("main", json!({ "generation": 0, "textFocus": true }))?;
    assert!(editing(&h), "the main window's editor");
    h.window_focused("float-timeline");
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": true }))?;
    assert!(editing(&h), "the float's field");
    h.report_from("main", json!({ "generation": 0, "textFocus": false }))?;
    assert!(editing(&h), "the inactive main window's report overrode the float's fact");
    // The person leaves the float's field: its own report ends the editing.
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": false }))?;
    assert!(!editing(&h));
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn an_inactive_floats_report_does_not_start_or_end_the_main_windows_editing() -> Result<(), Box<dyn Error>> {
    // The reverse: the person types in the main window; a float's field gains or loses the focus in its own page.
    let (h, dir) = host()?;
    h.window_focused("main");
    h.report_from("main", json!({ "generation": 0, "textFocus": true }))?;
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": false }))?;
    assert!(editing(&h), "the inactive float's false overrode the main window's editing");
    h.report_from("main", json!({ "generation": 0, "textFocus": false }))?;
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": true }))?;
    assert!(!editing(&h), "the inactive float's true started editing in the kernel");
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn the_active_window_changing_brings_its_current_fact() -> Result<(), Box<dyn Error>> {
    let (h, dir) = host()?;
    // Before any focus event the main window's fact counts.
    h.report_from("main", json!({ "generation": 0, "textFocus": true }))?;
    assert!(editing(&h));
    // A float that reported a field being edited earlier becomes active: its fact, at once, without a new report.
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": false }))?;
    h.window_focused("float-timeline");
    assert!(!editing(&h), "the float's own fact");
    h.window_focused("main");
    assert!(editing(&h), "back in the main window, its fact again");
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

/// Ruling E-63 (C51B-R4S-02): which window has the operating system's focus is the host's fact; an engine restart keeps
/// it. The windows report their facts again on the new generation (as they do on its notice), and the float's counts.
#[test]
fn an_engine_restart_keeps_the_active_float() -> Result<(), Box<dyn Error>> {
    let (h, dir) = host()?;
    h.window_focused("float-timeline");
    h.report_from("main", json!({ "generation": 0, "textFocus": false }))?;
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": true }))?;
    assert!(editing(&h));
    let (generation, restarted) = h.restart()?;
    assert!(restarted);
    h.report_from("main", json!({ "generation": generation, "textFocus": false }))?;
    h.report_from("float-timeline", json!({ "generation": generation, "textFocus": true }))?;
    assert!(editing(&h), "the float is still active after the restart: its field keeps the key");
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

/// E-63, the reviewer's two points beside it: the field kind comes from the same window as the boolean, and an older
/// report of a window never replaces a newer one.
#[test]
fn the_field_kind_is_the_active_windows_and_older_reports_lose() -> Result<(), Box<dyn Error>> {
    let (h, dir) = host()?;
    h.window_focused("float-timeline");
    // The float's field is the canvas's text editor: its undo chord is the document's, not the field's (SI-193).
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": true, "textFieldKind": "canvasText" }))?;
    let undo_is_the_fields = |h: &Host| matches!(h.exec().resolve_key("Primary+Z"), Ok(KeyResolution::Suspended { ref reason, .. }) if reason == "textField");
    assert!(editing(&h) && !undo_is_the_fields(&h));
    // The inactive main window's ordinary field kind does not replace the active float's.
    h.report_from("main", json!({ "generation": 0, "textFocus": false, "textFieldKind": "ordinary" }))?;
    assert!(editing(&h) && !undo_is_the_fields(&h), "the field kind came from the inactive window");

    // Out of order: the float's report 3 (no field) arrives, then its older report 2 (a field): the newer stays.
    let order = |n: u64| json!({ "page": "p1", "seq": n });
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": false, "reportOrder": order(3) }))?;
    h.report_from("float-timeline", json!({ "generation": 0, "textFocus": true, "reportOrder": order(2) }))?;
    assert!(!editing(&h), "an older report replaced a newer one");
    // A reloaded page numbers from 1 again: its reports apply.
    h.report_from(
        "float-timeline",
        json!({ "generation": 0, "textFocus": true, "reportOrder": { "page": "p2", "seq": 1 } }),
    )?;
    assert!(editing(&h));
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
