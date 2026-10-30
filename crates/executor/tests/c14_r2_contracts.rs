//! Round-two content-at-save and persisted-policy variants.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]
mod c13_support;
mod common;

use c13_support::*;
use common::*;
use serde_json::json;
use tt_executor::agents::{AgentConnect, Policy};
use tt_executor::{Invocation, InvokeOptions, Origin, Status};

#[test]
fn downgrade_and_corrupt_preferences_have_explicit_restart_behaviour() {
    let dir = tmp();
    let user = dir.join("user");
    let h = spawn(&user);
    ok(&h, "agent.policy.defaults.set", json!({"policy": "assist", "maxBatchSteps": 3}));
    ok(&h, "agent.policy.defaults.set", json!({"policy": "observe", "maxJobs": 1}));
    drop(h);
    let h = spawn(&user);
    let session = h.connect_agent(AgentConnect::default()).unwrap();
    let status = h.agent_status(&session.session_id).unwrap();
    assert_eq!((session.policy, status.limits.max_batch_steps, status.limits.max_jobs), (Policy::Observe, 3, 1));
    drop(h);
    std::fs::write(user.join("Settings/preferences.json"), b"{broken").unwrap();
    let h = spawn(&user);
    let session = h.connect_agent(AgentConnect::default()).unwrap();
    // ADR-0014 section 13 explicitly specifies defaults plus a retained corrupt copy.
    assert_eq!(session.policy, Policy::Assist);
    assert!(
        std::fs::read_dir(user.join("Settings"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains("corrupt"))
    );
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn quit_save_and_agent_save_as_current_preserve_external_content() {
    for folder in [false, true] {
        let dir = tmp();
        let target = dir.join(if folder { "P" } else { "P.animpkg" });
        let path = if folder {
            format!("{}{sep}", target.display(), sep = std::path::MAIN_SEPARATOR)
        } else {
            target.to_string_lossy().into_owned()
        };
        let h = spawn(&dir.join("user"));
        ok(&h, "project.create", json!({"name": "P", "path": path}));
        ok(&h, "doc.object.patch", json!({"targets": ["scene:#1"], "props": {"meta": {"n": 1}}}));
        let victim = if folder { target.join("project.json") } else { target.clone() };
        let time = std::fs::metadata(&victim).unwrap().modified().unwrap();
        let mut external = std::fs::read(&victim).unwrap();
        let last = external.len() - 1;
        external[last] ^= 1;
        std::fs::write(&victim, &external).unwrap();
        std::fs::File::options().write(true).open(&victim).unwrap().set_modified(time).unwrap();
        assert_eq!(err(&h, "app.quit", json!({"onUnsaved": "save"})).code, "project.changedOnDisk");
        let agent = h.connect_agent(AgentConnect { roots: vec![dir.clone()], ..Default::default() }).unwrap();
        let env = h.invoke(Invocation {
            command: "project.saveAs".into(),
            params: json!({"path": path, "onExists": "overwrite"}),
            options: InvokeOptions::default(),
            origin: Origin::Agent { session: agent.session_id, name: agent.client, scopes: agent.scopes },
        });
        assert_eq!(env.status, Status::NeedsConfirmation, "{env:?}");
        assert_eq!(env.result.unwrap()["category"], "externalChange");
        assert_eq!(std::fs::read(&victim).unwrap(), external);
        drop(h);
        let _ = std::fs::remove_dir_all(dir);
    }
}
