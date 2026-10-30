//! The generic MCP tool set (11 §7) and the optional typed tools (one per command, `agent.typedTools.set`).
//!
//! Every tool is the command layer: it maps its input onto a registered command (or the registry's discovery API)
//! and returns that command's **envelope** as `structuredContent` (11 §7.8: the generic tools "MUST return the same
//! data" as the commands they wrap). So every tool's `outputSchema` is the envelope schema, and `result` carries the
//! command's own result. Tools whose commands belong to later milestones exist now and answer
//! `command.notImplemented` with that milestone, exactly as `run_command` would.

use serde_json::{Map, Value, json};

/// One tool.
#[derive(Debug, Clone)]
pub struct ToolDef {
    /// Tool name.
    pub name: String,
    /// Title.
    pub title: String,
    /// Description.
    pub description: String,
    /// Input schema.
    pub input: Value,
    /// `readOnlyHint`.
    pub read_only: bool,
    /// `destructiveHint`.
    pub destructive: bool,
    /// `idempotentHint`.
    pub idempotent: bool,
    /// `openWorldHint` (outward-facing actions).
    pub open_world: bool,
}

impl ToolDef {
    /// The `tools/list` entry.
    #[must_use]
    pub fn listing(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input,
            "outputSchema": envelope_schema(),
            "annotations": {
                "title": self.title,
                "readOnlyHint": self.read_only,
                "destructiveHint": self.destructive,
                "idempotentHint": self.idempotent,
                "openWorldHint": self.open_world,
            },
        })
    }
}

/// The command result envelope (11 §7.2), the `outputSchema` of every tool.
#[must_use]
pub fn envelope_schema() -> Value {
    json!({
        "type": "object",
        "description": "Command result envelope (11 §7.2).",
        "properties": {
            "status": { "type": "string", "enum": ["ok", "needs_confirmation", "error"] },
            "result": { "description": "The command's result, per its result schema (describe_command)." },
            "targets": { "type": ["object", "null"] },
            "changes": { "type": "array", "items": { "type": "object" } },
            "changesTruncated": { "type": "boolean" },
            "changesSince": { "type": ["integer", "null"] },
            "undo": { "type": ["object", "null"] },
            "warnings": { "type": "array", "items": { "type": "object" } },
            "documentRev": { "type": ["integer", "null"] },
            "error": { "type": ["object", "null"] },
            "dryRun": { "type": "boolean" },
            "steps": { "type": "array", "items": { "type": "object" } },
        },
        "required": ["status"],
        "additionalProperties": true,
    })
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

const REF: &str =
    "Object id or path (`scene:\"Intro\"/element:\"Bob\"`), or a special reference (`@current`, `@selection`).";

/// The generic tools.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one table")]
pub fn generic() -> Vec<ToolDef> {
    let t = |name: &str,
             title: &str,
             description: &str,
             input: Value,
             read_only: bool,
             destructive: bool,
             idempotent: bool| ToolDef {
        name: name.to_owned(),
        title: title.to_owned(),
        description: description.to_owned(),
        input,
        read_only,
        destructive,
        idempotent,
        open_world: false,
    };
    let page = json!({ "type": "object", "properties": { "cursor": { "type": "string" }, "limit": { "type": "integer", "minimum": 1, "maximum": 1000 } }, "additionalProperties": false });
    vec![
        t(
            "get_state",
            "Get state",
            "Current project, scene, frame, selection, mode, views, jobs, UI and your session (`app.state.get`). The first call to make.",
            obj(
                json!({ "include": { "type": "array", "items": { "type": "string", "enum": ["project","scene","frame","selection","tool","views","playback","jobs","ui","agent"] } } }),
                &[],
            ),
            true,
            false,
            true,
        ),
        t(
            "list_commands",
            "List commands",
            "Registered commands with kind, label, summary, enablement in the current context, scopes and undoability.",
            obj(
                json!({ "namespace": { "type": "string" }, "filter": { "type": "string", "description": "Substring, or a glob with `*` and `?`." },
            "kind": { "type": "string", "enum": ["action","toggle","query","job","ui","event"] }, "enabledOnly": { "type": "boolean", "default": false },
            "includeSchema": { "type": "boolean", "default": false }, "page": page }),
                &[],
            ),
            true,
            false,
            true,
        ),
        t(
            "describe_command",
            "Describe command",
            "Full declaration of a command: schemas, defaults, enablement, scopes, confirmation conditions, undo label, examples, GUI locations.",
            obj(json!({ "id": { "type": "string" } }), &["id"]),
            true,
            false,
            true,
        ),
        t(
            "get_docs",
            "Get documentation",
            "Agent documentation: `agent-guide`, `object-model`, `coordinates`, `errors`, or a namespace (same as `app://docs/*`).",
            obj(json!({ "topic": { "type": "string", "default": "agent-guide" } }), &[]),
            true,
            false,
            true,
        ),
        t(
            "run_command",
            "Run command",
            "Run any registered command by id. Returns the command result envelope.",
            obj(
                json!({ "id": { "type": "string" }, "params": { "type": "object" }, "dryRun": { "type": "boolean", "default": false },
            "expectRev": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0 } },
            "confirmToken": { "type": "string" }, "confirm": { "type": "boolean", "default": false }, "requestId": { "type": "string" } }),
                &["id"],
            ),
            false,
            true,
            false,
        ),
        t(
            "batch",
            "Batch",
            "Several commands as one transaction and one undo step (atomic by default). Steps may refer to earlier results with `$<n>.result.<field>` or `$<name>…`.",
            obj(
                json!({ "label": { "type": "string" }, "steps": { "type": "array", "minItems": 1, "items": { "type": "object", "properties": {
              "id": { "type": "string" }, "params": { "type": "object" }, "as": { "type": "string" } }, "required": ["id"], "additionalProperties": false } },
            "atomic": { "type": "boolean", "default": true }, "stopOnError": { "type": "boolean", "default": true }, "dryRun": { "type": "boolean", "default": false },
            "expectRev": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0 } } }),
                &["label", "steps"],
            ),
            false,
            true,
            false,
        ),
        t(
            "get_object",
            "Get object",
            "An object as JSON (section 10 schema), children to `depth` (`doc.object.get`).",
            obj(
                json!({ "ref": { "type": "string", "description": REF }, "depth": { "type": "integer", "minimum": -1, "default": 1 },
            "fields": { "type": "array", "items": { "type": "string" } }, "page": page }),
                &["ref"],
            ),
            true,
            false,
            true,
        ),
        t(
            "find_objects",
            "Find objects",
            "Objects by type, container, name or field predicates (`doc.object.find`).",
            obj(
                json!({ "type": { "type": "string" }, "within": { "type": "string", "description": REF }, "name": { "type": "string" },
            "where": { "type": "object" }, "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 100 },
            "page": page }),
                &[],
            ),
            true,
            false,
            true,
        ),
        t(
            "set_properties",
            "Set properties",
            "Validated patch of writable properties on objects; one undo step (`doc.object.patch`).",
            obj(
                json!({ "targets": { "type": "array", "items": { "type": "string" }, "minItems": 1 }, "props": { "type": "object" },
            "expectRev": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0 } }, "dryRun": { "type": "boolean" } }),
                &["targets", "props"],
            ),
            false,
            false,
            false,
        ),
        t(
            "doc_diff",
            "Document diff",
            "Changes since a revision or a checkpoint (`doc.diff`).",
            obj(
                json!({ "sinceRev": { "type": "integer", "minimum": 0 }, "sinceCheckpoint": { "type": "string" } }),
                &[],
            ),
            true,
            false,
            true,
        ),
        t(
            "render_frame",
            "Render frame",
            "A rendered frame of a scene (`render.frame.run` with `composition: \"camera\"`).",
            obj(
                json!({ "scene": { "type": "string" }, "frame": { "type": "integer", "minimum": 1 }, "camera": { "type": "string" },
            "width": { "type": "integer", "minimum": 1 }, "height": { "type": "integer", "minimum": 1 }, "background": { "type": "string" },
            "quality": { "type": "string", "enum": ["draft", "final"] }, "output": { "type": "string" } }),
                &[],
            ),
            true,
            false,
            true,
        ),
        t(
            "render_drawing",
            "Render drawing",
            "One drawing rendered in drawing space (`draw.drawing.render`).",
            obj(
                json!({ "drawing": { "type": "string" }, "width": { "type": "integer", "minimum": 1 }, "background": { "type": "string" },
            "showStrokes": { "type": "boolean" } }),
                &["drawing"],
            ),
            true,
            false,
            true,
        ),
        t(
            "render_contact_sheet",
            "Render contact sheet",
            "A grid of frames in one image (`inspect.contactSheet.render`).",
            obj(
                json!({ "scene": { "type": "string" }, "frames": {}, "columns": { "type": "integer", "minimum": 1 },
            "cellWidth": { "type": "integer", "minimum": 16 }, "labels": { "type": "boolean" } }),
                &["frames"],
            ),
            true,
            false,
            true,
        ),
        t(
            "screenshot",
            "Screenshot",
            "The GUI (attached) or a virtual view (headless) as PNG (`inspect.screenshot`).",
            obj(json!({ "target": { "type": "string" }, "region": { "type": "object" } }), &[]),
            true,
            false,
            true,
        ),
        t(
            "describe_frame",
            "Describe frame",
            "What is visible and audible at a frame (`inspect.frame.describe`).",
            obj(
                json!({ "scene": { "type": "string" }, "frame": { "type": "integer", "minimum": 1 }, "camera": { "type": "string" },
            "detail": { "type": "string", "enum": ["summary", "full"] } }),
                &[],
            ),
            true,
            false,
            true,
        ),
        t(
            "hit_test",
            "Hit test",
            "Objects under a point, top-most first (`inspect.hitTest`).",
            obj(
                json!({ "space": { "type": "string" }, "point": { "type": "object" }, "scene": { "type": "string" }, "frame": { "type": "integer" },
            "view": { "type": "string" } }),
                &["space", "point"],
            ),
            true,
            false,
            true,
        ),
        t(
            "convert_coordinates",
            "Convert coordinates",
            "Points converted between coordinate spaces (`inspect.coordinates.convert`).",
            obj(
                json!({ "from": { "type": "string" }, "to": { "type": "string" }, "points": { "type": "array", "items": { "type": "object" } },
            "scene": { "type": "string" }, "frame": { "type": "integer" }, "element": { "type": "string" }, "view": { "type": "string" } }),
                &["from", "to", "points"],
            ),
            true,
            false,
            true,
        ),
        t(
            "inspect_ui",
            "Inspect UI",
            "UI tree with the command id each widget invokes (`inspect.ui.tree`).",
            obj(json!({ "target": { "type": "string" }, "depth": { "type": "integer", "minimum": 0 } }), &[]),
            true,
            false,
            true,
        ),
        t(
            "pointer",
            "Pointer input",
            "Simulated pointer gesture in a view; returns the envelope of the command it produced (gesture equivalence, 11 §7.6).",
            obj(
                json!({ "view": { "type": "string" }, "events": { "type": "array", "items": { "type": "object" } }, "space": { "type": "string" },
            "pointerType": { "type": "string" }, "tool": { "type": "string" } }),
                &["view", "events"],
            ),
            false,
            false,
            false,
        ),
        t(
            "key",
            "Key input",
            "Simulated key press; returns the envelope of the command the key invoked.",
            obj(json!({ "keys": { "type": "string" }, "view": { "type": "string" } }), &["keys"]),
            false,
            false,
            false,
        ),
        t(
            "type_text",
            "Type text",
            "Simulated typing into a widget from inspect_ui.",
            obj(json!({ "text": { "type": "string" }, "target": { "type": "string" } }), &["text"]),
            false,
            false,
            false,
        ),
        t(
            "undo",
            "Undo",
            "Undo steps (`edit.undo`); `ifTop` undoes only if that step is still the most recent one.",
            obj(
                json!({ "steps": { "type": "integer", "minimum": 1, "default": 1 }, "ifTop": { "type": "string" } }),
                &[],
            ),
            false,
            false,
            false,
        ),
        t(
            "redo",
            "Redo",
            "Redo steps (`edit.redo`).",
            obj(json!({ "steps": { "type": "integer", "minimum": 1, "default": 1 } }), &[]),
            false,
            false,
            false,
        ),
        t(
            "history",
            "History",
            "Undo history, top first, with origins (`edit.history.get`).",
            obj(json!({ "limit": { "type": "integer", "minimum": 1, "default": 20 } }), &[]),
            true,
            false,
            true,
        ),
        t(
            "checkpoint",
            "Checkpoint",
            "Named in-memory snapshots (`checkpoint.create|restore|list|delete`); restore is one undoable step.",
            obj(
                json!({ "action": { "type": "string", "enum": ["create", "restore", "list", "delete"] }, "name": { "type": "string" } }),
                &["action"],
            ),
            false,
            false,
            false,
        ),
        t(
            "job_status",
            "Job status",
            "Job records of this session (`job.list`).",
            obj(json!({ "jobId": { "type": "string" } }), &[]),
            true,
            false,
            true,
        ),
        t(
            "job_wait",
            "Wait for job",
            "Wait until a job ends or the timeout passes (`job.wait`); progress is reported to clients that ask for it.",
            obj(
                json!({ "jobId": { "type": "string" }, "timeoutMs": { "type": "integer", "minimum": 0, "maximum": 3_600_000, "default": 60000 } }),
                &["jobId"],
            ),
            true,
            false,
            true,
        ),
        t(
            "job_cancel",
            "Cancel job",
            "Cancel a job (`job.cancel`).",
            obj(json!({ "jobId": { "type": "string" } }), &["jobId"]),
            false,
            false,
            true,
        ),
    ]
}

/// The name of a command's typed tool: the id with `.` → `_` (`doc.object.get` → `doc_object_get`).
#[must_use]
pub fn typed_name(command: &str) -> String {
    command.replace('.', "_")
}

/// A command's typed tool.
#[must_use]
pub fn typed(declaration: &Value) -> ToolDef {
    let id = declaration.get("id").and_then(Value::as_str).unwrap_or_default();
    let kind = declaration.get("kind").and_then(Value::as_str).unwrap_or_default();
    let mut input = declaration.get("paramsSchema").cloned().unwrap_or_else(|| json!({ "type": "object" }));
    if input.get("type").and_then(Value::as_str) != Some("object") {
        input = json!({ "type": "object", "properties": {}, "additionalProperties": false });
    }
    let destructive = declaration.get("confirmation").and_then(Value::as_array).is_some_and(|c| !c.is_empty());
    ToolDef {
        name: typed_name(id),
        title: declaration.get("label").and_then(Value::as_str).unwrap_or(id).to_owned(),
        description: format!("`{id}`: {}", declaration.get("summary").and_then(Value::as_str).unwrap_or_default()),
        input,
        read_only: kind == "query",
        destructive,
        idempotent: kind == "query",
        open_world: declaration
            .get("scopes")
            .and_then(Value::as_array)
            .is_some_and(|s| s.iter().any(|x| x == "network")),
    }
}

/// How a generic tool maps onto the command layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Run one command with these parameters (and invocation options).
    Command {
        /// Command id.
        id: String,
        /// Parameters.
        params: Value,
        /// `dryRun`.
        dry_run: bool,
        /// `expectRev`.
        expect_rev: Map<String, Value>,
        /// `confirmToken`.
        confirm_token: Option<String>,
        /// `confirm: true`.
        confirm: bool,
        /// `requestId`.
        request_id: Option<String>,
    },
    /// Handled by the server (discovery, docs, batches, job_wait).
    Server,
    /// A tool with no command until a later milestone (simulated input).
    Unavailable {
        /// The milestone that brings it.
        milestone: &'static str,
        /// Why.
        reason: &'static str,
    },
}

fn command(id: &str, params: Value) -> Plan {
    Plan::Command {
        id: id.to_owned(),
        params,
        dry_run: false,
        expect_rev: Map::new(),
        confirm_token: None,
        confirm: false,
        request_id: None,
    }
}

/// Renames and copies tool arguments into command parameters.
fn pick(args: &Map<String, Value>, map: &[(&str, &str)]) -> Value {
    let mut out = Map::new();
    for (from, to) in map {
        if let Some(v) = args.get(*from) {
            out.insert((*to).to_owned(), v.clone());
        }
    }
    Value::Object(out)
}

/// The plan of a generic tool call. `Err` is an `arg.*` message for the tool's own input.
///
/// # Errors
/// A message naming the invalid argument.
pub fn plan(name: &str, args: &Map<String, Value>) -> Result<Plan, String> {
    Ok(match name {
        "get_state" => command("app.state.get", json!({})),
        "list_commands" | "describe_command" | "get_docs" | "batch" | "job_wait" => Plan::Server,
        "run_command" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("`id` is required")?;
            Plan::Command {
                id: id.to_owned(),
                params: args.get("params").cloned().unwrap_or_else(|| json!({})),
                dry_run: args.get("dryRun").and_then(Value::as_bool).unwrap_or(false),
                expect_rev: args.get("expectRev").and_then(Value::as_object).cloned().unwrap_or_default(),
                confirm_token: args.get("confirmToken").and_then(Value::as_str).map(str::to_owned),
                confirm: args.get("confirm").and_then(Value::as_bool).unwrap_or(false),
                request_id: args.get("requestId").and_then(Value::as_str).map(str::to_owned),
            }
        }
        "get_object" => command(
            "doc.object.get",
            pick(args, &[("ref", "ref"), ("depth", "depth"), ("fields", "fields"), ("page", "page")]),
        ),
        "find_objects" => {
            let mut p = pick(
                args,
                &[("type", "type"), ("within", "within"), ("name", "name"), ("where", "where"), ("limit", "limit")],
            );
            if let (Some(offset), Some(m)) =
                (args.get("page").and_then(|p| p.get("cursor")).and_then(Value::as_str), p.as_object_mut())
            {
                let n: u64 = offset.parse().map_err(|_| "`page.cursor` is not a cursor of find_objects")?;
                m.insert("offset".into(), json!(n));
            }
            command("doc.object.find", p)
        }
        "set_properties" => {
            let mut p = command("doc.object.patch", pick(args, &[("targets", "targets"), ("props", "props")]));
            if let Plan::Command { expect_rev, dry_run, .. } = &mut p {
                *expect_rev = args.get("expectRev").and_then(Value::as_object).cloned().unwrap_or_default();
                *dry_run = args.get("dryRun").and_then(Value::as_bool).unwrap_or(false);
            }
            p
        }
        "doc_diff" => {
            command("doc.diff", pick(args, &[("sinceRev", "sinceRev"), ("sinceCheckpoint", "sinceCheckpoint")]))
        }
        "render_frame" => {
            let mut p = pick(
                args,
                &[
                    ("scene", "sceneId"),
                    ("frame", "frame"),
                    ("width", "width"),
                    ("height", "height"),
                    ("quality", "quality"),
                    ("background", "background"),
                    ("output", "path"),
                ],
            );
            if let Some(m) = p.as_object_mut() {
                m.insert("composition".into(), json!("camera"));
            }
            command("render.frame.run", p)
        }
        "render_drawing" => command(
            "draw.drawing.render",
            pick(
                args,
                &[
                    ("drawing", "drawingId"),
                    ("width", "width"),
                    ("background", "background"),
                    ("showStrokes", "showStrokes"),
                ],
            ),
        ),
        "render_contact_sheet" => command(
            "inspect.contactSheet.render",
            pick(
                args,
                &[
                    ("scene", "sceneId"),
                    ("frames", "frames"),
                    ("columns", "columns"),
                    ("cellWidth", "cellWidth"),
                    ("labels", "labels"),
                ],
            ),
        ),
        "screenshot" => command("inspect.screenshot", pick(args, &[("target", "target"), ("region", "region")])),
        "describe_frame" => command(
            "inspect.frame.describe",
            pick(args, &[("scene", "sceneId"), ("frame", "frame"), ("camera", "cameraId"), ("detail", "detail")]),
        ),
        "hit_test" => command(
            "inspect.hitTest",
            pick(
                args,
                &[("space", "space"), ("point", "point"), ("scene", "sceneId"), ("frame", "frame"), ("view", "view")],
            ),
        ),
        "convert_coordinates" => command(
            "inspect.coordinates.convert",
            pick(
                args,
                &[
                    ("from", "from"),
                    ("to", "to"),
                    ("points", "points"),
                    ("scene", "sceneId"),
                    ("frame", "frame"),
                    ("element", "elementId"),
                    ("view", "view"),
                ],
            ),
        ),
        "inspect_ui" => command("inspect.ui.tree", pick(args, &[("target", "target"), ("depth", "depth")])),
        "pointer" => Plan::Unavailable {
            milestone: "M2 C2.3",
            reason: "pointer gestures produce the structured commands of the drawing tools (gesture equivalence, 11 §7.6), which arrive with M2",
        },
        "key" => Plan::Unavailable {
            milestone: "M4 C4.2",
            reason: "key input reaches commands through the keymap of the desktop shell (M4)",
        },
        "type_text" => Plan::Unavailable {
            milestone: "M4 C4.2",
            reason: "typing targets widgets of the UI tree (inspect_ui), which arrives with the desktop shell (M4)",
        },
        "undo" => command("edit.undo", pick(args, &[("steps", "steps"), ("ifTop", "ifTop")])),
        "redo" => command("edit.redo", pick(args, &[("steps", "steps")])),
        "history" => command("edit.history.get", pick(args, &[("limit", "limit")])),
        "checkpoint" => {
            let action = args.get("action").and_then(Value::as_str).ok_or("`action` is required")?;
            let id = match action {
                "create" => "checkpoint.create",
                "restore" => "checkpoint.restore",
                "list" => "checkpoint.list",
                "delete" => "checkpoint.delete",
                _ => return Err(format!("`action` must be create, restore, list or delete, not `{action}`")),
            };
            command(id, if action == "list" { json!({}) } else { pick(args, &[("name", "name")]) })
        }
        "job_status" => command("job.list", pick(args, &[("jobId", "jobId")])),
        "job_cancel" => command("job.cancel", pick(args, &[("jobId", "jobId")])),
        other => return Err(format!("unknown tool `{other}`")),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
    use super::*;

    #[test]
    fn every_generic_tool_has_a_plan_and_wraps_a_registered_command() {
        let reg = tt_executor::registry().unwrap();
        let tools = generic();
        assert_eq!(tools.len(), 28, "the generic tool set of 11 §7.1–§7.7");
        for t in &tools {
            let args: Map<String, Value> = match t.name.as_str() {
                "run_command" => json!({ "id": "app.state.get" }),
                "checkpoint" => json!({ "action": "list" }),
                _ => json!({}),
            }
            .as_object()
            .cloned()
            .unwrap();
            match plan(&t.name, &args).unwrap() {
                Plan::Command { id, .. } => assert!(reg.get(&id).is_some(), "{} wraps unregistered `{id}`", t.name),
                Plan::Server | Plan::Unavailable { .. } => {}
            }
            let l = t.listing();
            assert_eq!(l["inputSchema"]["type"], "object", "{}", t.name);
            assert_eq!(l["outputSchema"]["type"], "object", "{}", t.name);
        }
        assert!(plan("nope", &Map::new()).is_err());
    }
}
