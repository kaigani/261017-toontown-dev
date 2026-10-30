//! Scoped closure boundary test.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review test")]
use std::fs;
use tt_persist::lock::{self, Locked};
#[test]
fn heartbeat_and_drop_do_not_write_while_takeover_holds_arbiter() {
    let dir = std::env::temp_dir().join(format!("tt-r4-{}", tt_persist::fsx::random_token(8)));
    fs::create_dir_all(&dir).unwrap();
    let project = dir.join("Boundary.animpkg");
    let Locked::Acquired { handle, .. } = lock::acquire(&project) else { panic!("owner") };
    let path = handle.path().to_owned();
    let arbiter = std::path::PathBuf::from(format!("{}.takeover", path.display()));
    let guard = fs::OpenOptions::new().write(true).create_new(true).open(&arbiter).unwrap();
    let before = fs::read(&path).unwrap();
    handle.refresh().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut foreign = handle.info();
    foreign.lock_id = "foreign-review-session".into();
    let publication = serde_json::to_vec(&foreign).unwrap();
    fs::write(&path, &publication).unwrap();
    drop(guard);
    fs::remove_file(&arbiter).unwrap();
    assert_eq!(handle.refresh().unwrap_err().code(), "project.lockedByOther");
    assert_eq!(fs::read(&path).unwrap(), publication);
    let guard = fs::OpenOptions::new().write(true).create_new(true).open(&arbiter).unwrap();
    drop(handle);
    assert_eq!(fs::read(&path).unwrap(), publication);
    assert!(arbiter.exists());
    drop(guard);
    fs::remove_dir_all(dir).unwrap();
}
