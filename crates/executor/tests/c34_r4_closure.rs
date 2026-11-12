//! Independent scoped closure probes. Run serially: work counters are process-wide.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason="review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize,Ordering};
use std::time::{Duration,Instant};
use tt_executor::{Invocation,ExecutorConfig,Origin,BatchRequest};
use tt_vector::work::hooks as work;
fn edit() -> Invocation { Invocation::user("element.create",json!({"type":"peg","name":"Closure","sceneId":"scene:#1"})) }
#[test]
fn deadline_precedes_reply_but_retry_recovers_exactly_one_commit() {
 let h=fixture_with(ExecutorConfig{budget:Duration::from_millis(200)});
 let before=rev(&h); let n=AtomicUsize::new(0);
 let mut inv=edit(); inv.options.request_id=Some("r4-deadline".into());
 let (env,_)=h.invoke_served_until(inv.clone(),&|| {if n.fetch_add(1,Ordering::SeqCst)==1 {std::thread::sleep(Duration::from_millis(400));} false});
 let e=env.error.unwrap(); assert_eq!(e.code,"command.timeout"); assert_ne!(e.details["reason"],"cancelled");
 assert_eq!(rev(&h),before+1);
 let (answer,cached)=h.invoke_served(inv); assert!(answer.is_ok(),"{answer:?}"); assert!(cached); assert!(answer.undo.is_some()); assert_eq!(rev(&h),before+1);
 assert_eq!(work::outstanding_tokens(),0); assert_eq!(work::queued_tokens(),0);
}
#[test]
fn bounded_questions_latency_distribution_and_latch() {
 let mut samples=Vec::new();
 for trial in 0..100 {
  let h=fixture_with(ExecutorConfig{budget:Duration::from_millis(1000)});
  let (tx,rx)=std::sync::mpsc::channel(); let n=AtomicUsize::new(0);
  let elapsed=std::thread::scope(|s| {
   let call=s.spawn(||h.invoke_served_until(edit(),&|| {if n.fetch_add(1,Ordering::SeqCst)==1 {tx.send(()).unwrap();std::thread::sleep(Duration::from_millis(350));} false}));
   rx.recv_timeout(Duration::from_secs(10)).unwrap(); let start=Instant::now();
   assert!(run(&h,"doc.state.get",json!({})).is_ok()); let elapsed=start.elapsed(); call.join().unwrap(); elapsed
  });
  eprintln!("LATENCY trial={trial} ms={}",elapsed.as_secs_f64()*1000.0); samples.push(elapsed);
 }
 samples.sort(); eprintln!("LATENCY min={:?} median={:?} p95={:?} p99={:?} max={:?} bound=250ms",samples[0],samples[50],samples[94],samples[98],samples[99]);
 for (stall,block_index) in [(240,1),(260,1),(350,2)] {
  let h=fixture(); let n=AtomicUsize::new(0); work::reset_asks();
  let steps=(0..1000).map(|_|json!({"id":"edit.selection.set","params":{"context":"drawing","ids":[]}})).collect::<Vec<_>>();
  let req:BatchRequest=serde_json::from_value(json!({"label":"r4 latch","atomic":false,"steps":steps})).unwrap();
  let env=h.batch_until(req,Origin::User,&|| {if n.fetch_add(1,Ordering::SeqCst)==block_index {std::thread::sleep(Duration::from_millis(stall));} false});
  assert!(env.is_ok(),"{env:?}"); let asks=work::asks();
  eprintln!("LATCH stall={stall} index={block_index} asks={asks}");
  if stall>250 {assert_eq!(asks,block_index as i64);} else {assert!(asks>=1);}
 }
}
#[test]
fn queries_ask_zero_and_work_cancellation_names_its_cause() {
 let h=fixture(); work::reset_asks();
 assert!(h.invoke_served_until(Invocation::user("doc.state.get",json!({})),&||true).0.is_ok()); assert_eq!(work::asks(),0);
 work::set_spin_micros(20000); let start=Instant::now();
 let env=h.invoke_served_until(Invocation::user("render.frame.run",json!({"frame":1,"scale":0.25})),&||start.elapsed()>Duration::from_millis(60)).0;
 work::set_spin_micros(0); let e=env.error.unwrap(); assert_eq!(e.code,"command.timeout"); assert_eq!(e.details["reason"],"cancelled"); assert_eq!(work::asks(),0);
}
#[test]
fn job_before_handler_refuses_and_after_start_keeps_identity() {
 for after in [false,true] {
  let h=fixture(); let dir=std::env::temp_dir().join(format!("c34-r4-job-{}-{after}",std::process::id()));
  let env=h.invoke_served_until(Invocation::user("render.range.run",json!({"from":1,"to":1000,"width":8,"directory":dir,"createDirs":true,"wait":true})),&||!after || !h.jobs().list(None,None).is_empty()).0;
  let e=env.error.unwrap(); assert_eq!(e.details["reason"],"cancelled");
  if after {let id=e.details["jobId"].as_str().unwrap(); ok(&h,"job.cancel",json!({"jobId":id})); h.jobs().wait(id,Duration::from_secs(60)).unwrap();}
  else {assert!(h.jobs().list(None,None).is_empty());}
  assert!(!std::fs::read_dir(&dir).into_iter().flatten().flatten().any(|x|x.file_name().to_string_lossy().contains("tt-partial")));
 }
}
