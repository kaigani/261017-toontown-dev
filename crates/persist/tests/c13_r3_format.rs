//! Independent lock and portable-folder final-round variants.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regressions")]
use std::fs;
use std::time::Duration;
use tt_persist::lock::{self, Locked};

#[test]
fn heartbeat_refuses_an_unreadable_replacement_lock() {
    let dir = std::env::temp_dir().join(format!("tt-c13-r3-{}", tt_persist::fsx::random_token(6)));
    fs::create_dir_all(&dir).unwrap();
    let project = dir.join("Lock.animpkg");
    fs::write(&project, b"placeholder").unwrap();
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("owner") };
    // Reproduce the takeover boundary: the old file is removed and the new writer has exclusively created its
    // replacement but has not written JSON yet. A heartbeat of the old owner must not publish over this file.
    fs::remove_file(handle.path()).unwrap();
    let replacement = fs::OpenOptions::new().write(true).create_new(true).open(handle.path()).unwrap();
    let result = handle.refresh();
    drop(replacement);
    assert!(result.is_err(), "old heartbeat recreated ownership over an unreadable replacement: {result:?}");
    drop(handle);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_live_writer_paused_beyond_two_seconds_is_not_taken_over() {
    let dir = std::env::temp_dir().join(format!("tt-c13-r3-{}", tt_persist::fsx::random_token(6)));
    fs::create_dir_all(&dir).unwrap();
    let project = dir.join("Lock.animpkg");
    fs::write(&project, b"placeholder").unwrap();
    let path = lock::lock_path(&project);
    // This is the actual create_new/write_all boundary of acquire, held open as a paused writer. No PID or clock
    // manipulation: the writer is live. Elapsed mtime alone cannot distinguish it from a dead writer.
    let writer = fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    std::thread::sleep(Duration::from_millis(2200));
    let result = lock::acquire(&project);
    drop(writer);
    assert!(!matches!(result, Locked::Acquired { .. }), "a live paused writer was taken over: {result:?}");
    drop(result);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn two_hundred_byte_unicode_components_work_in_windows_folder_form() {
    let dir = std::env::temp_dir().join(format!("tt-c13-r3-{}", tt_persist::fsx::random_token(6)));
    fs::create_dir_all(&dir).unwrap();
    for component in ["é".repeat(100), "😀".repeat(50)] {
        assert_eq!(component.len(), 200);
        assert!(tt_persist::zip::safe_name(&format!("vendor/{component}")));
        let folder = dir.join(&component);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(&component), b"portable").unwrap();
        let name = format!("vendor/{component}/{component}");
        let doc = tt_model::fixture::sample_document(57).unwrap();
        let preserved = tt_persist::load::Preserved { unknown_entries: vec![name.clone()], ..Default::default() };
        let mut blobs = tt_persist::container::Blobs::default();
        blobs.insert(name.clone(), tt_persist::container::BlobData::Bytes(std::sync::Arc::new(b"portable".to_vec())));
        let saved = dir.join(format!("saved-{}", tt_persist::fsx::random_token(3)));
        tt_persist::save::save(
            &saved,
            tt_persist::container::Form::Folder,
            &tt_persist::save::SaveInput {
                doc: &doc,
                preserved: &preserved,
                blobs: &blobs,
                session: None,
                saved_at: "2026-10-29T12:00:00.000Z".into(),
            },
            None,
            false,
        )
        .unwrap();
        assert_eq!(fs::read(saved.join(name)).unwrap(), b"portable");
    }
    assert!(!tt_persist::zip::safe_name(&format!("vendor/{}", "é".repeat(101))));
    fs::remove_dir_all(dir).unwrap();
}
