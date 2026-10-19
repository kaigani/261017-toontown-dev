//! `checkpoint.*` (11 §7.7, §15.2): named copy-on-write snapshots of the document, independent of the undo stack.
//! Restoring is one undoable step ("Restore checkpoint <name>").

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_registry::{CommandError, command};

use super::common::{Empty, NoParams};
use crate::cx::Cx;
use crate::rules;

/// Checkpoints kept per project (ADR-0013 §7).
pub(crate) const MAX_CHECKPOINTS: usize = 100;

/// `{name}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NameParams {
    /// Checkpoint name.
    #[schemars(length(min = 1, max = 128))]
    name: String,
}

/// `{name, documentRev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointRef {
    /// Name.
    name: String,
    /// The document revision it holds.
    document_rev: u64,
}

fn not_found(name: &str) -> CommandError {
    CommandError::with_details("checkpoint.notFound", format!("no checkpoint `{name}`"), json!({ "name": name }))
}

#[command(
    id = "checkpoint.create",
    kind = ui,
    label = "Create checkpoint",
    scopes = [ui],
    needs_project,
    example(title = "Checkpoint before an experiment", params = r#"{"name": "experiment-1"}"#),
    agent_only = "agent safety net (11 §7.7); the GUI's equivalent is the History panel",
)]
pub(crate) fn checkpoint_create(cx: &mut Cx, p: NameParams) -> Result<CheckpointRef, CommandError> {
    let name = p.name.trim().to_owned();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(super::common::invalid(
            "/name",
            "a checkpoint name is 1–128 characters without control characters",
        ));
    }
    // A checkpoint is staged like every other change of the call (C12-R2-03): it captures the content staged so far
    // at the revision that content has (the predicted revision when something is staged), later steps of a batch see
    // it, and it goes with the batch's rollback, a failed call or a dry run (the session copy is discarded).
    let origin = cx.origin().label();
    let project = cx.project()?;
    if project.checkpoints.iter().any(|c| c.name == name) {
        return Err(CommandError::with_details(
            "checkpoint.exists",
            format!("a checkpoint named `{name}` exists"),
            json!({ "name": name }),
        )
        .hint("delete it first or choose another name"));
    }
    if project.checkpoints.len() >= MAX_CHECKPOINTS {
        return Err(CommandError::with_details(
            tt_registry::codes::LIMIT_EXCEEDED,
            format!("at most {MAX_CHECKPOINTS} checkpoints are kept"),
            json!({ "limit": MAX_CHECKPOINTS }),
        ));
    }
    // The content staged so far, stamped as it will be committed: its `documentRev` and object revisions are the
    // revision the checkpoint advertises (C12-R3-02).
    let doc = cx.stamped_document()?;
    let rev = doc.document_rev();
    cx.project_mut()?.checkpoints.push(crate::session::Checkpoint {
        name: name.clone(),
        created_at: crate::time::now_iso(),
        document_rev: rev,
        origin,
        doc,
    });
    Ok(CheckpointRef { name, document_rev: rev })
}

/// A checkpoint as listed.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointEntry {
    /// Name.
    name: String,
    /// Creation time.
    created_at: String,
    /// Document revision.
    document_rev: u64,
    /// Who created it.
    origin: String,
}

#[command(
    id = "checkpoint.list",
    kind = query,
    label = "List checkpoints",
    scopes = [read],
    needs_project,
    example(title = "All checkpoints, oldest first", params = "{}"),
    agent_only = "agent safety net (11 §7.7)",
)]
pub(crate) fn checkpoint_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<CheckpointEntry>, CommandError> {
    Ok(cx
        .project()?
        .checkpoints
        .iter()
        .map(|c| CheckpointEntry {
            name: c.name.clone(),
            created_at: c.created_at.clone(),
            document_rev: c.document_rev,
            origin: c.origin.clone(),
        })
        .collect())
}

#[command(
    id = "checkpoint.restore",
    kind = action,
    label = "Restore checkpoint",
    scopes = [edit],
    undo = "Restore checkpoint",
    needs_project,
    enablement = rules::HAS_CHECKPOINTS,
    example(title = "Restore a checkpoint", params = r#"{"name": "before-walk"}"#),
    agent_only = "agent safety net (11 §7.7)",
)]
pub(crate) fn checkpoint_restore(cx: &mut Cx, p: NameParams) -> Result<CheckpointRef, CommandError> {
    let target = cx.project()?.checkpoints.iter().find(|c| c.name == p.name).map(|c| c.doc.clone());
    let target = target.ok_or_else(|| not_found(&p.name))?;
    let label = format!("Restore checkpoint {}", p.name);
    cx.set_label(label.clone());
    cx.restore(&target, &label)?;
    let rev = cx.document()?.document_rev();
    Ok(CheckpointRef { name: p.name, document_rev: rev })
}

#[command(
    id = "checkpoint.delete",
    kind = ui,
    label = "Delete checkpoint",
    scopes = [ui],
    needs_project,
    example(title = "Delete a checkpoint", params = r#"{"name": "before-walk"}"#),
    agent_only = "agent safety net (11 §7.7)",
)]
pub(crate) fn checkpoint_delete(cx: &mut Cx, p: NameParams) -> Result<Empty, CommandError> {
    let project = cx.project_mut()?;
    let before = project.checkpoints.len();
    project.checkpoints.retain(|c| c.name != p.name);
    if project.checkpoints.len() == before {
        return Err(not_found(&p.name));
    }
    Ok(Empty {})
}
