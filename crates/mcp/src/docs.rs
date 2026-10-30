//! Agent documentation generated from the registry and the object schemas (11 §4 `instructions`, §7.1 `get_docs`,
//! §9.1 `app://docs/*`, §13 "examples everywhere"). The `initialize` instructions are exactly the agent guide.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::Value;
use tt_model::ObjectType;
use tt_registry::{Kind, Registry, Status};

/// The fixed topics.
pub const TOPICS: [&str; 4] = ["agent-guide", "object-model", "coordinates", "errors"];

/// The namespaces of the registry, sorted.
#[must_use]
pub fn namespaces(reg: &Registry) -> Vec<String> {
    let mut v: Vec<String> = reg.entries().filter_map(|e| e.id().split('.').next().map(str::to_owned)).collect();
    v.sort();
    v.dedup();
    v
}

/// A topic's Markdown, or `None` for an unknown topic.
#[must_use]
pub fn topic(reg: &Registry, name: &str) -> Option<String> {
    match name {
        "agent-guide" => Some(agent_guide(reg)),
        "object-model" => Some(object_model()),
        "coordinates" => Some(coordinates()),
        "errors" => Some(errors()),
        ns if namespaces(reg).iter().any(|n| n == ns) => Some(namespace(reg, ns)),
        _ => None,
    }
}

/// The orientation for agents (also the `initialize` instructions).
#[must_use]
pub fn agent_guide(reg: &Registry) -> String {
    let total = reg.entries().filter(|e| e.spec.kind != Kind::Event).count();
    let implemented =
        reg.entries().filter(|e| e.spec.kind != Kind::Event && matches!(e.status(), Status::Implemented)).count();
    format!(
        "# Toontown agent guide\n\n\
Toontown is a 2D animation application. This MCP server is its agent interface: every capability is a registered \
command (`{total}` invocable commands; `{implemented}` implemented in this build, the others answer \
`command.notImplemented` with the milestone that delivers them).\n\n\
## Start\n\n\
1. `get_state` — the open project, scene, frame, selection, mode, jobs, and your session's scopes.\n\
2. `list_commands` (filter by `namespace`, `kind`, `enabledOnly`) and `describe_command` (schemas, examples, \
confirmation conditions, undo label).\n\
3. `get_docs` / `app://docs/{{topic}}`: `object-model`, `coordinates`, `errors`, or a namespace such as `doc`.\n\n\
## Act\n\n\
- `run_command {{id, params}}` runs any command; `batch {{label, steps}}` runs several as one transaction and one undo \
step (`$<n>.result.<field>` or `$<name>…` refer to earlier steps). The generic tools (`get_object`, `find_objects`, \
`set_properties`, `doc_diff`, `undo`, `redo`, `history`, `checkpoint`, `job_*`) wrap commands and return the same data.\n\
- In a sequential batch (`atomic: false`, also the batch-script format of `--run` and of \
`agent.activity.export {{format: \"batch\"}}`), a step `{{\"id\": \"batch\", \"params\": {{label, steps, expectRev}}}}` is an \
**atomic group**: its steps run as one transaction and one undo step, exactly like `batch {{atomic: true}}`, and its \
`$<n>` references count from the group's first step. `expectRev` on an atomic batch or a group is checked once \
before its first step; a mismatch refuses the whole transaction with `rev.conflict` (a sequential batch refuses \
`expectRev`). Groups do not nest: a group inside an atomic batch or another group is `batch.nestedGroup`. The activity export writes every committed atomic batch as one group, so a replay keeps \
its revisions and undo boundary.\n\
- Address objects by id or by path: `scene:\"Intro\"/element:\"Bob\"`, `type:#index`, `type:@id`; special references \
`@current`, `@selection`, `@activeView`, `@activeCamera`.\n\
- Every mutating call returns an envelope: `status`, `result`, `changes` (ids, ops, new revisions), `undo`, \
`warnings`, `documentRev`. Pass `dryRun: true` to see the changes first, `expectRev` for optimistic concurrency and \
`requestId` to make a retry safe.\n\n\
## Verify\n\n\
Read back with `get_object` / `find_objects`, compare revisions with `doc_diff {{sinceRev}}`, and check `doc.validate`. \
Renders (`render_frame`, `render_drawing`, contact sheets) arrive with the renderer milestone (M3).\n\n\
## Safety\n\n\
Your session holds scopes (`read`, `edit`, `files.write`, …); a missing scope is `permission.denied`. Some actions \
need confirmation under your policy: the call then returns `status: \"needs_confirmation\"` with a `confirmToken`, a \
summary and the consequences, and changes nothing. Re-invoke the same call with the same parameters and \
`confirmToken` (single use, five minutes) to proceed. Clients that support elicitation are asked directly. Paths are \
absolute and must lie inside your roots (`fs.outsideRoots`); writes never silently overwrite (`onExists`).\n\n\
## Watch\n\n\
Subscribe to `app://state`, `app://project`, `app://object/{{id}}` or `app://events` (read it with `?after=<seq>`); \
updates are coalesced to one per resource per 100 ms. Your calls are logged in `app://activity`.\n"
    )
}

/// The object model: types, their schema resources, identity and revisions.
#[must_use]
pub fn object_model() -> String {
    let mut s = String::from(
        "# Object model\n\nEvery object has a stable opaque `id` (type prefix + ULID, never reused), a `type` and a \
revision `rev` that increments on every change; the document has `documentRev`. Objects are read with `get_object` \
(depth 1 by default: children as `{id, type, name}` stubs) and found with `find_objects`. The JSON Schema of a type is \
the resource `app://schema/{type}` (same as `doc.schema.get`).\n\n| Type | Schema |\n|---|---|\n",
    );
    for t in ObjectType::ALL {
        let _ = writeln!(s, "| `{0}` | `app://schema/{0}` |", t.as_str());
    }
    s
}

/// Coordinate spaces (11 §5.3).
#[must_use]
pub fn coordinates() -> String {
    "# Coordinate spaces\n\n| Space | Definition |\n|---|---|\n\
| `drawing` | Local space of a drawing: origin at the drawing centre, y up, 1 field = 100 drawing units |\n\
| `scene` | 3-D staging space of a scene in fields (1 field = 1 inch; the default camera frame is 12 fields wide) |\n\
| `camera` | The active camera's frame: x, y in field units from the camera centre; `normalized` is 0..1 from top-left |\n\
| `view` | Pixels of a named view, origin top-left, y down, at the view's zoom and pan |\n\
| `image` | Pixels of a returned image, origin top-left; results include the transform to `camera`/`scene` |\n\n\
Frames are 1-based and ranges inclusive; angles are degrees, counter-clockwise positive; colours are `#RRGGBB[AA]` \
(documents reference colours by colour id). `convert_coordinates` (`inspect.coordinates.convert`) converts between \
spaces; it arrives with the renderer milestone (M3).\n"
        .to_owned()
}

/// The generic error codes (11 §10).
#[must_use]
pub fn errors() -> String {
    let rows = [
        ("command.unknown", "No such command id"),
        ("command.disabled", "Not enabled in the current context (`details.reason`)"),
        ("command.notImplemented", "Registered, implemented by a later milestone (`details.milestone`)"),
        ("command.timeout", "The call exceeded its budget; it may still complete — read `get_state` before retrying"),
        ("arg.missing / arg.unknown / arg.invalid", "Parameter validation (`details.pointer`, `details.allowed`)"),
        ("ref.notFound / ref.ambiguous / ref.wrongType", "Object references (`details.candidates`)"),
        ("rev.conflict", "`expectRev` mismatch (`details.currentRev`)"),
        ("project.none", "Needs an open project"),
        ("project.unsavedChanges", "Would discard changes; pass `onUnsaved`"),
        ("permission.denied", "Missing scope (`details.missingScopes`)"),
        ("confirmation.required / .declined / .expired", "Confirmation protocol (11 §6.3)"),
        ("fs.outsideRoots / fs.invalidPath / fs.exists / fs.notFound", "File problems"),
        (
            "job.failed / job.cancelled / batch.jobNotAllowed / batch.stepFailed",
            "Jobs and batches (`details.stepIndex`)",
        ),
        ("undo.notTop / edit.nothingToUndo", "History"),
        ("agent.paused", "The user paused your session"),
        ("limit.exceeded", "A session limit (`details.limit`, `details.value`)"),
        ("internal", "Unexpected failure"),
    ];
    let mut s = String::from(
        "# Errors\n\nErrors use stable dotted codes `area.reason`, a human-readable `message`, `details` and, where \
possible, a `hint`. Warnings use the same code space and never abort a command. Area-specific codes are documented \
with their commands (`describe_command`).\n\n| Code | Meaning |\n|---|---|\n",
    );
    for (c, m) in rows {
        let _ = writeln!(s, "| `{c}` | {m} |");
    }
    s
}

/// One namespace: its commands, with a worked example each.
#[must_use]
pub fn namespace(reg: &Registry, ns: &str) -> String {
    let mut s = format!("# Namespace `{ns}`\n\nCommands in id order. Run any of them with `run_command`.\n\n");
    let mut by_id: BTreeMap<&str, &tt_registry::Entry> = BTreeMap::new();
    for e in reg.entries().filter(|e| e.id().split('.').next() == Some(ns)) {
        by_id.insert(e.id(), e);
    }
    for (id, e) in by_id {
        let d = e.describe();
        let status = match e.status() {
            Status::Implemented => "implemented".to_owned(),
            Status::NotImplemented { owner } => format!("arrives with {owner}"),
        };
        let scopes: Vec<&str> = e.scopes().iter().map(|x| x.as_str()).collect();
        let _ = write!(
            s,
            "## `{id}`\n\n{} — kind `{}`, scopes `{}`, {status}.\n\n",
            d.get("summary").and_then(Value::as_str).unwrap_or_default(),
            e.spec.kind.as_str(),
            scopes.join("`, `"),
        );
        if let Some(ex) = d.get("examples").and_then(Value::as_array).and_then(|a| a.first()) {
            let params = ex.get("params").cloned().unwrap_or(Value::Null);
            let call = serde_json::json!({ "id": id, "params": params });
            let _ = write!(
                s,
                "Example ({}):\n\n```json\n{}\n```\n\n",
                ex.get("title").and_then(Value::as_str).unwrap_or("example"),
                serde_json::to_string_pretty(&call).unwrap_or_default()
            );
        }
    }
    worked_example(&mut s, ns);
    s
}

/// The worked end-to-end examples, executed through the MCP server and retained (M2 C2.1 obligation 1; recorded and
/// re-checked by `tests/c21_worked_examples.rs`).
fn worked_examples() -> &'static Value {
    static ALL: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(include_str!("worked-examples.json")).unwrap_or(Value::Null))
}

/// Appends a namespace's worked example: each step's tool call and its executed response (11 §13).
fn worked_example(s: &mut String, ns: &str) {
    let Some(w) = worked_examples().get(ns) else { return };
    let _ = write!(
        s,
        "## Worked example

{}. Each step is a tool call an agent made and the response it received, recorded          from a real run (values listed under \"varies\" — generated ids, times, tokens — differ from run to run;          paths are shown under `/tmp`).

",
        w.get("title").and_then(Value::as_str).unwrap_or("Worked example")
    );
    for (i, st) in w.get("steps").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let _ = write!(
            s,
            "### Step {}: {}

Request:

```json
{}
```

Response:

```json
{}
```

",
            i + 1,
            st.get("note").and_then(Value::as_str).unwrap_or_default(),
            serde_json::to_string_pretty(st.get("request").unwrap_or(&Value::Null)).unwrap_or_default(),
            serde_json::to_string_pretty(st.get("response").unwrap_or(&Value::Null)).unwrap_or_default(),
        );
        if let Some(v) = st.get("varies").and_then(Value::as_array).filter(|v| !v.is_empty()) {
            let list: Vec<&str> = v.iter().filter_map(Value::as_str).collect();
            let _ = write!(
                s,
                "Varies: `{}`

",
                list.join("`, `")
            );
        }
    }
}
