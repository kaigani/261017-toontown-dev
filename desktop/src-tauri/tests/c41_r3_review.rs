//! Independent host quorum review.
mod part_b_freshness {
    use serde_json::json;
    use std::time::{Duration, Instant};
    use tt_desktop::host::{Host, HostOptions};
    #[test]
    fn opening_closing_restart_and_reload() -> Result<(), Box<dyn std::error::Error>> {
        let h = Host::start(HostOptions::default())?;
        let report = |label: &str, generation| {
            h.report_from(label, json!({"generation":generation,"attached":true,"uiTree":{"id":label},"renderedSeq":h.exec().events_broadcast()}))
        };
        report("main", 0)?;
        h.expect_window("float-history");
        let start = Instant::now();
        assert!(!h.exec().await_rendered(Duration::from_millis(30)));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(h.run("inspect.ui.tree", json!({}), false, None).result.unwrap_or_default()["renderedSeq"].is_null());
        report("float-history", 0)?;
        assert!(h.exec().await_rendered(Duration::ZERO));
        h.expect_window("float-colour");
        let exec = h.exec();
        let waiter = std::thread::spawn(move || exec.await_rendered(Duration::from_secs(2)));
        h.expect_window("float-library");
        report("float-colour", 0)?;
        assert!(!h.exec().await_rendered(Duration::ZERO));
        h.forget_window_reports("float-library");
        assert!(waiter.join().map_err(|_| "wait panic")?);
        let (generation, _) = h.restart()?;
        report("main", generation)?;
        assert!(!h.exec().await_rendered(Duration::ZERO));
        assert_eq!(
            h.report_from("float-history", json!({"generation":0,"uiTree":{"id":"old"},"renderedSeq":0}))?["ignored"],
            "staleGeneration"
        );
        report("float-history", generation)?;
        report("float-colour", generation)?;
        assert!(h.exec().await_rendered(Duration::ZERO));
        h.forget_window_reports("float-history");
        h.forget_window_reports("float-colour");
        h.forget_window_reports("main");
        assert!(!h.exec().await_rendered(Duration::ZERO), "a reloading main cannot vouch for its old tree");
        Ok(())
    }
}
