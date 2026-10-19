//! The load report (10 §7.3) and its issues (10 §10.4 `Issue`).

use serde::Serialize;
use serde_json::Value;
use tt_model::document::Severity;

/// One problem found or repaired while loading.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadIssue {
    /// `is_<n>`, unique within the report.
    pub issue_id: String,
    /// Code (10 §7.2, §9).
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// The object concerned, when there is one.
    pub object_id: Option<String>,
    /// Its canonical path in the loaded document, when it still exists.
    pub path: Option<String>,
    /// The package entry concerned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Human-readable message.
    pub message: String,
    /// Structured details; content removed by a repair is listed here with its old JSON (REP-02).
    pub details: Value,
    /// Machine-applicable fixes (none for load issues: the repairs were applied).
    pub fixes: Vec<Value>,
    /// Whether a repair was applied for it.
    #[serde(skip)]
    pub repaired: bool,
}

/// The report of one load (10 §7.3).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadReport {
    /// Project ID.
    pub project_id: String,
    /// The path opened.
    pub path: String,
    /// `package` or `folder`.
    pub form: String,
    /// The file's format version.
    pub format_version: String,
    /// The version migrated from, if any.
    pub migrated_from: Option<String>,
    /// Whether the project is open read-only.
    pub read_only: bool,
    /// Load time.
    pub duration_ms: u64,
    /// Issues, in the order found.
    pub issues: Vec<LoadIssue>,
    /// Number of repairs applied.
    pub repaired_count: usize,
}

/// Collects issues with sequential ids.
#[derive(Debug, Clone, Default)]
pub struct Issues {
    list: Vec<LoadIssue>,
}

impl Issues {
    /// Adds an issue; returns its index.
    #[allow(clippy::too_many_arguments, reason = "the fields of an issue")]
    pub fn push(
        &mut self,
        code: &str,
        severity: Severity,
        object_id: Option<&str>,
        entry: Option<&str>,
        message: impl Into<String>,
        details: Value,
        repaired: bool,
    ) -> usize {
        let n = self.list.len();
        self.list.push(LoadIssue {
            issue_id: format!("is_{}", n + 1),
            code: code.to_owned(),
            severity,
            object_id: object_id.map(str::to_owned),
            path: None,
            entry: entry.map(str::to_owned),
            message: message.into(),
            details,
            fixes: Vec::new(),
            repaired,
        });
        n
    }

    /// A repaired error (10 §7.2 severity "error", repaired by the load).
    pub fn repaired_error(
        &mut self,
        code: &str,
        object_id: Option<&str>,
        entry: Option<&str>,
        message: impl Into<String>,
        details: Value,
    ) {
        self.push(code, Severity::Error, object_id, entry, message, details, true);
    }

    /// A repaired warning.
    pub fn repaired_warning(
        &mut self,
        code: &str,
        object_id: Option<&str>,
        entry: Option<&str>,
        message: impl Into<String>,
        details: Value,
    ) {
        self.push(code, Severity::Warning, object_id, entry, message, details, true);
    }

    /// A warning that needed no repair.
    pub fn warning(
        &mut self,
        code: &str,
        object_id: Option<&str>,
        entry: Option<&str>,
        message: impl Into<String>,
        details: Value,
    ) {
        self.push(code, Severity::Warning, object_id, entry, message, details, false);
    }

    /// An informational issue.
    pub fn info(
        &mut self,
        code: &str,
        object_id: Option<&str>,
        entry: Option<&str>,
        message: impl Into<String>,
        details: Value,
    ) {
        self.push(code, Severity::Info, object_id, entry, message, details, false);
    }

    /// The issues.
    #[must_use]
    pub fn list(&self) -> &[LoadIssue] {
        &self.list
    }

    /// Takes the issues.
    #[must_use]
    pub fn into_list(self) -> Vec<LoadIssue> {
        self.list
    }

    /// Number of repairs.
    #[must_use]
    pub fn repaired(&self) -> usize {
        self.list.iter().filter(|i| i.repaired).count()
    }
}
