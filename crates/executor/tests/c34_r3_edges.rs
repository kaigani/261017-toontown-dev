//! Round-3 acceptance, commit arbitration and real-death probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tt_executor::{Invocation, BatchRequest, Origin};
use tt_persist::lock::{hooks, Boundary};
static TURN: Mutex<()> = Mutex::new(());
struct Reset;
impl Drop for Reset { fn drop(&mut self) { hooks::set(None); } }
fn edit() -> Invocation { Invocation::user("element.create",json!({"type":"peg","name":"R3","sceneId":"scene:#1"})) }
#[test]
fn worker_boundary_refusal_identifies_cancellation_reason() {
 let _t=TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
 let h=fixture(); let n=AtomicUsize::new(0);
 let e=h.invoke_served_until(edit(), &|| n.fetch_add(1,Ordering::SeqCst)==2).0;
 assert_eq!(e.error.as_ref().unwrap().code,"command.timeout");
 assert_eq!(e.error.unwrap().details["reason"],"cancelled");
}
#[test]
fn commit_wins_returns_success_and_cached_envelope() {
 let _t=TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner); let _r=Reset;
 let h=fixture(); let flag=Arc::new(AtomicBool::new(false)); let f=flag.clone();
 hooks::set(Some(Arc::new(move |b,_| { if b==Boundary::Committed {f.store(true,Ordering::SeqCst);} })));
 let mut inv=edit(); inv.options.request_id=Some("commit-r3".into());
 let before=rev(&h); let (first,_)=h.invoke_served_until(inv.clone(), &||flag.load(Ordering::SeqCst));
 hooks::set(None);
 assert!(first.is_ok(),"{first:?}"); assert_eq!(rev(&h),before+1);
 let (again,cached)=h.invoke_served_until(inv,&||false); assert!(cached);
 assert_eq!(serde_json::to_value(first).unwrap(),serde_json::to_value(again).unwrap());
 assert_eq!(rev(&h),before+1);
}
#[test]
fn cancelled_queued_request_reports_real_worker_death_as_internal() {
 let _t=TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner); let _r=Reset;
 let h=fixture(); let (tx,rx)=std::sync::mpsc::channel();
 hooks::set(Some(Arc::new(move |b,_| { if b==Boundary::Committed {tx.send(()).unwrap(); std::thread::sleep(Duration::from_millis(300)); panic!("review worker death");} })));
 let e=std::thread::scope(|s| {
  let first=s.spawn(||h.invoke(edit())); rx.recv_timeout(Duration::from_secs(10)).unwrap();
  let n=AtomicUsize::new(0);
  let e=h.invoke_served_until(edit(),&||n.fetch_add(1,Ordering::SeqCst)>0).0;
  first.join().unwrap(); e
 });
 hooks::set(None);
 assert_eq!(run(&h,"doc.state.get",json!({})).error.unwrap().code,"internal");
 assert_eq!(e.error.unwrap().code,"internal","a marked token cannot prove intentional discard");
}
#[test]
fn pre_cancelled_batch_modes_and_job_submission_retry_once() {
 let _t=TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
 for (atomic,dry) in [(false,false),(true,false),(false,true),(true,true)] {
  let h=fixture(); let before=rev(&h);
  let req:BatchRequest=serde_json::from_value(json!({"label":"r3","atomic":atomic,"dryRun":dry,"steps":[{"id":"element.create","params":{"type":"peg","name":"R3","sceneId":"scene:#1"}}]})).unwrap();
  assert!(!h.batch_until(req.clone(),Origin::User,&||true).is_ok()); assert_eq!(rev(&h),before);
  assert!(h.batch_until(req,Origin::User,&||false).is_ok()); assert_eq!(rev(&h),before+u64::from(!dry));
 }
 for wait in [false,true] {
  let h=fixture(); let dir=std::env::temp_dir().join(format!("c34-r3-{}-{wait}",std::process::id()));
  let mut inv=Invocation::user("render.range.run",json!({"from":1,"to":1,"width":8,"directory":dir,"createDirs":true,"wait":wait}));
  inv.options.request_id=Some(format!("job-{wait}"));
  assert!(!h.invoke_served_until(inv.clone(),&||true).0.is_ok()); assert!(!dir.exists());
  let (e,cached)=h.invoke_served_until(inv.clone(),&||false); assert!(e.is_ok(),"{e:?}"); assert!(!cached);
  let (_,cached)=h.invoke_served_until(inv,&||false); assert!(cached);
 }
}
