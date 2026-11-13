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
