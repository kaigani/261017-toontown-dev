//! Round-3 cache race and token mark/forget contention.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tt_executor::Invocation;
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn live_retry_racing_cancelled_original_executes_once() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    let before = rev(&h);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut inv = Invocation::user("element.create", json!({"type":"peg","name":"Race","sceneId":"scene:#1"}));
    inv.options.request_id = Some("same-r3".into());
    std::thread::scope(|s| {
        let original = inv.clone();
        let first = s.spawn(|| {
            let n = AtomicUsize::new(0);
            h.invoke_served_until(original, &|| {
                if n.fetch_add(1, Ordering::SeqCst) == 1 {
                    tx.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(150));
                    true
                } else {
                    false
                }
            })
            .0
        });
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let retry = h.invoke_served_until(inv.clone(), &|| false);
        assert!(!first.join().unwrap().is_ok());
        assert!(retry.0.is_ok());
        assert!(!retry.1);
    });
    assert_eq!(rev(&h), before + 1);
    assert!(h.invoke_served_until(inv, &|| false).1);
    assert_eq!(rev(&h), before + 1);
}
#[test]
fn mark_after_forget_and_mixed_call_contention_leave_no_tokens() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let state = tt_vector::work::CancelState::default();
    std::thread::scope(|s| {
        for _ in 0..8 {
            let st = &state;
            s.spawn(move || {
                for _ in 0..2000 {
                    let t = st.token();
                    st.mark(t);
                    st.forget(t);
                    st.mark(t);
                }
            });
        }
    });
    let h = fixture();
    std::thread::scope(|s| {
        for k in 0..6 {
            let h = &h;
            s.spawn(move || {
                for i in 0..500 {
                    let n = AtomicUsize::new(0);
                    let inv = Invocation::user(
                        if i % 3 == 0 { "doc.state.get" } else { "edit.selection.set" },
                        if i % 3 == 0 { json!({}) } else { json!({"context":"drawing","ids":[]}) },
                    );
                    let _ = h.invoke_served_until(inv, &|| n.fetch_add(1, Ordering::SeqCst) > (k % 3));
                }
            });
        }
    });
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(tt_vector::work::hooks::queued_tokens(), 0);
    assert_eq!(tt_vector::work::hooks::outstanding_tokens(), 0);
}
#[test]
fn pre_cancelled_undo_redo_are_not_cached() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    ok(&h, "element.create", json!({"type":"peg","name":"Undo","sceneId":"scene:#1"}));
    for id in ["edit.undo", "edit.redo"] {
        let before = rev(&h);
        let mut inv = Invocation::user(id, json!({}));
        inv.options.request_id = Some(id.into());
        assert!(!h.invoke_served_until(inv.clone(), &|| true).0.is_ok());
        assert_eq!(rev(&h), before);
        let (e, c) = h.invoke_served_until(inv.clone(), &|| false);
        assert!(e.is_ok(), "{e:?}");
        assert!(!c);
        let after = rev(&h);
        assert!(h.invoke_served_until(inv, &|| false).1);
        assert_eq!(rev(&h), after);
    }
}

#[test]
fn cancelled_file_effect_and_job_cancel_retry_once() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    let dir = std::env::temp_dir().join(format!("c34-r3-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("Review.animpkg");
    let mut save = Invocation::user("project.saveAs", json!({"path":target}));
    save.options.request_id = Some("save-r3".into());
    assert!(!h.invoke_served_until(save.clone(), &|| true).0.is_ok());
    assert!(!target.exists());
    let (e, c) = h.invoke_served_until(save.clone(), &|| false);
    assert!(e.is_ok(), "{e:?}");
    assert!(!c);
    if let Some(job) = e.result.as_ref().and_then(|r| r["jobId"].as_str()) {
        h.jobs().wait(job, Duration::from_secs(30)).unwrap();
    }
    assert!(h.invoke_served_until(save, &|| false).1);
    assert!(target.exists());
    let mut save = Invocation::user("project.save", json!({}));
    save.options.request_id = Some("save-existing-r3".into());
    let bytes = std::fs::read(&target).unwrap();
    assert!(!h.invoke_served_until(save.clone(), &|| true).0.is_ok());
    assert_eq!(std::fs::read(&target).unwrap(), bytes);
    let (e, c) = h.invoke_served_until(save.clone(), &|| false);
    assert!(e.is_ok(), "{e:?}");
    assert!(!c);
    if let Some(job) = e.result.as_ref().and_then(|r| r["jobId"].as_str()) {
        h.jobs().wait(job, Duration::from_secs(30)).unwrap();
    }
    assert!(h.invoke_served_until(save, &|| false).1);
    let started = ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":100000,"width":8,"directory":dir.join("seq"),"createDirs":true}),
    );
    let job = started["jobId"].as_str().unwrap();
    let mut cancel = Invocation::user("job.cancel", json!({"jobId":job}));
    cancel.options.request_id = Some("cancel-r3".into());
    assert!(!h.invoke_served_until(cancel.clone(), &|| true).0.is_ok());
    let (e, c) = h.invoke_served_until(cancel.clone(), &|| false);
    assert!(e.is_ok());
    assert!(!c);
    assert!(h.invoke_served_until(cancel, &|| false).1);
    h.jobs().wait(job, Duration::from_secs(30)).unwrap();
}
