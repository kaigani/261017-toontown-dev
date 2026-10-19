//! Engine self-test: exercises the facade end to end without a display, window system or GPU.

use serde::Serialize;

use crate::{Engine, EngineInfo, PingRequest, components, determinism};

/// Outcome of one self-test check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    /// Stable check id (`area.check`).
    pub id: &'static str,
    /// Whether the check passed.
    pub ok: bool,
    /// What was observed.
    pub detail: String,
}

/// The full self-test report, printed as JSON by `toontown-headless --selftest --json`.
#[derive(Debug, Clone, Serialize)]
pub struct SelftestReport {
    /// True when every check passed.
    pub ok: bool,
    /// Engine information at the time of the test.
    pub engine: EngineInfo,
    /// Individual checks, in execution order.
    pub checks: Vec<CheckResult>,
}

impl SelftestReport {
    /// Appends a check performed by a shell (e.g. the headless binary adding `tt-mcp`), keeping `ok` consistent.
    pub fn push(&mut self, check: CheckResult) {
        self.ok &= check.ok;
        self.checks.push(check);
    }
}

pub(crate) fn run(engine: &Engine) -> SelftestReport {
    let checks =
        vec![check_components(), check_versions(engine), check_ping(engine), check_determinism(), check_kernel()];
    let ok = checks.iter().all(|c| c.ok);
    SelftestReport { ok, engine: engine.info(), checks }
}

fn check_components() -> CheckResult {
    let list = components();
    let missing: Vec<_> = list.iter().filter(|c| c.spec_sections.is_empty()).map(|c| c.name).collect();
    CheckResult {
        id: "engine.components",
        ok: list.len() == 13 && missing.is_empty(),
        detail: format!("{} crates linked: {}", list.len(), list.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")),
    }
}

fn check_versions(engine: &Engine) -> CheckResult {
    let info = engine.info();
    let format_ok = is_numeric_dotted(info.format_version, 2);
    let registry_ok = is_numeric_dotted(info.registry_version, 3);
    CheckResult {
        id: "engine.versions",
        ok: format_ok && registry_ok,
        detail: format!(
            "formatVersion {} (MAJOR.MINOR), registryVersion {} (semver)",
            info.format_version, info.registry_version
        ),
    }
}

fn check_ping(engine: &Engine) -> CheckResult {
    let payload = "toontown".repeat(128);
    match engine.ping(PingRequest { seq: 0xC0FFEE, payload: Some(payload.clone()) }) {
        Ok(r) if r.seq == 0xC0FFEE && r.payload.as_deref() == Some(payload.as_str()) => CheckResult {
            id: "engine.ping",
            ok: true,
            detail: format!("echoed seq and {} payload bytes in {} ns", payload.len(), r.handler_ns),
        },
        Ok(r) => CheckResult { id: "engine.ping", ok: false, detail: format!("wrong echo: seq {}", r.seq) },
        Err(e) => CheckResult { id: "engine.ping", ok: false, detail: format!("{}: {e}", e.code()) },
    }
}

fn check_determinism() -> CheckResult {
    let got = determinism::probe_sha256();
    let ok = got == determinism::GOLDEN_SHA256;
    CheckResult {
        id: "determinism.ieee754",
        ok,
        detail: if ok {
            format!("sha256 {got} matches golden")
        } else {
            format!("sha256 {got} != golden {}", determinism::GOLDEN_SHA256)
        },
    }
}

/// The C1.2 kernel end to end: the registry assembles over spec A, and the executor opens the sample document, commits
/// one patch as one revision, undoes it as the next and answers a stub with its owner.
fn check_kernel() -> CheckResult {
    let fail = |detail: String| CheckResult { id: "kernel.executor", ok: false, detail };
    let registry = match tt_executor::registry() {
        Ok(r) => r,
        Err(e) => return fail(format!("registry assembly: {}", e.0)),
    };
    let doc = match tt_model::fixture::sample_document(1) {
        Ok(d) => d,
        Err(e) => return fail(format!("sample document: {e:?}")),
    };
    let rev = doc.document_rev();
    let scene = doc.project().scenes().ids().first().map(|s| s.as_str().to_owned()).unwrap_or_default();
    let h = match tt_executor::ExecutorHandle::spawn(tt_executor::ExecutorConfig::default()) {
        Ok(h) => h,
        Err(e) => return fail(e.to_string()),
    };
    let run = |id: &str, params: serde_json::Value| h.invoke(tt_executor::Invocation::user(id, params));
    let opened = h.open_document(doc, None, false);
    let patched =
        run("doc.object.patch", serde_json::json!({ "targets": [scene], "props": { "meta": { "selftest": true } } }));
    let undone = run("edit.undo", serde_json::json!({}));
    let stub = run("render.frame.run", serde_json::json!({}));
    let ok = registry.len() == 796
        && opened.is_ok()
        && patched.document_rev == Some(rev + 1)
        && undone.document_rev == Some(rev + 2)
        && stub.error.as_ref().is_some_and(|e| e.code == "command.notImplemented");
    CheckResult {
        id: "kernel.executor",
        ok,
        detail: format!(
            "registry {} entries (v{}); patch rev {:?}, undo rev {:?} from {rev}; stub answered {:?}",
            registry.len(),
            tt_registry::REGISTRY_VERSION,
            patched.document_rev,
            undone.document_rev,
            stub.error.map(|e| e.code)
        ),
    }
}

fn is_numeric_dotted(s: &str, parts: usize) -> bool {
    let split: Vec<_> = s.split('.').collect();
    split.len() == parts && split.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EngineConfig, RunMode};

    #[test]
    fn selftest_passes() {
        let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless }).unwrap();
        let report = engine.selftest();
        assert!(report.ok, "{report:#?}");
        let ids: Vec<_> = report.checks.iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            ["engine.components", "engine.versions", "engine.ping", "determinism.ieee754", "kernel.executor"]
        );
    }

    #[test]
    fn push_keeps_ok_consistent() {
        let engine = Engine::new(EngineConfig { run_mode: RunMode::Headless }).unwrap();
        let mut report = engine.selftest();
        report.push(CheckResult { id: "x.fail", ok: false, detail: String::new() });
        assert!(!report.ok);
    }

    #[test]
    fn dotted_version_parser() {
        assert!(is_numeric_dotted("0.0", 2));
        assert!(is_numeric_dotted("1.12.3", 3));
        assert!(!is_numeric_dotted("1.2", 3));
        assert!(!is_numeric_dotted("1.x", 2));
        assert!(!is_numeric_dotted("1..2", 3));
    }
}
