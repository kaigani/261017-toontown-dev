//! Round 2 discovery lifecycle through the public host API.
use std::error::Error;
use tt_desktop::host::{Host, HostOptions};

#[test]
fn malformed_token_and_dead_endpoint_are_replaced_then_cleared() -> Result<(), Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("tt-c41-r2-{}", tt_executor::random_token(12)));
    let h = Host::start(HostOptions {
        user_dir: Some(dir.clone()),
        mcp_http: Some("127.0.0.1:0".parse()?),
        ..Default::default()
    })?;
    let file = h.settings_dir.join("agent-discovery.json");
    std::fs::write(&file, r#"{"token":"malformed","endpoint":"http://127.0.0.1:1/mcp","processId":4294967295}"#)?;
    h.stop_http();
    let cleared: serde_json::Value = serde_json::from_slice(&std::fs::read(&file)?)?;
    assert!(cleared["endpoint"].is_null() && cleared["processId"].is_null());
    h.start_http()?;
    let live: serde_json::Value = serde_json::from_slice(&std::fs::read(&file)?)?;
    assert!(tt_executor::is_secret_token(live["token"].as_str().ok_or("token")?, 24));
    h.stop_http();
    let stopped: serde_json::Value = serde_json::from_slice(&std::fs::read(&file)?)?;
    assert!(stopped["endpoint"].is_null() && stopped["processId"].is_null());
    assert_eq!(stopped["token"], live["token"]);
    drop(h);
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

mod part_c1_freshness {
    use super::*;
    use serde_json::json;
    use std::time::Duration;
    #[test]
    fn unreported_float_prevents_combined_freshness() -> Result<(), Box<dyn Error>> {
        let h = Host::start(HostOptions::default())?;
        h.report_from("float-history", json!({"generation":0,"textFocus":false}))?;
        let seq = h.exec().events_broadcast();
        h.report_from("main", json!({"generation":0,"attached":true,"uiTree":{"id":"main"},"renderedSeq":seq}))?;
        assert!(!h.exec().await_rendered(Duration::ZERO), "a known float has supplied no tree");
        h.forget_window_reports("float-history");
        assert!(h.exec().await_rendered(Duration::ZERO));
        Ok(())
    }
    #[test]
    fn generations_and_closed_float() -> Result<(), Box<dyn Error>> {
        let h = Host::start(HostOptions::default())?;
        for generation in [1, u64::MAX] {
            assert_eq!(
                h.report_from("main", json!({"generation":generation,"uiTree":{"id":"wrong"}}))?["ignored"],
                "staleGeneration"
            );
        }
        h.report_from("float-history", json!({"generation":0,"uiTree":{"id":"float"}}))?;
        let seq = h.exec().events_broadcast();
        h.report_from("main", json!({"generation":0,"attached":true,"uiTree":{"id":"main"},"renderedSeq":seq}))?;
        assert!(!h.exec().await_rendered(Duration::ZERO));
        h.forget_window_reports("float-history");
        assert!(h.exec().await_rendered(Duration::ZERO));
        Ok(())
    }
}
