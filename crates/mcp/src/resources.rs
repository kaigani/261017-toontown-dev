//! MCP resources (11 §9.1): the static resources, the URI templates, how each URI is read through the command layer,
//! and which executor events update which subscribed URIs.

use serde_json::{Map, Value, json};

/// The fixed resources.
#[must_use]
pub fn listing() -> Vec<Value> {
    let r = |uri: &str, name: &str, description: &str, mime: &str| json!({ "uri": uri, "name": name, "title": name, "description": description, "mimeType": mime });
    vec![
        r("app://state", "State", "Same as get_state", "application/json"),
        r("app://project", "Project", "Project object with its scene list", "application/json"),
        r(
            "app://docs/agent-guide",
            "Agent guide",
            "Orientation for agents (same as the initialize instructions)",
            "text/markdown",
        ),
        r("app://docs/object-model", "Object model", "Object types, identity, revisions, schemas", "text/markdown"),
        r(
            "app://docs/coordinates",
            "Coordinate spaces",
            "drawing, scene, camera, view and image spaces",
            "text/markdown",
        ),
        r("app://docs/errors", "Errors", "Generic error codes", "text/markdown"),
        r("app://events", "Events", "Event stream: read with ?after=<seq>", "application/json"),
        r("app://activity", "Activity", "This session's audit log", "application/jsonl"),
    ]
}

/// The URI templates (RFC 6570).
#[must_use]
pub fn templates() -> Vec<Value> {
    let t = |uri: &str, name: &str, description: &str, mime: &str| json!({ "uriTemplate": uri, "name": name, "title": name, "description": description, "mimeType": mime });
    vec![
        t("app://object/{id}", "Object", "Any object as JSON, depth 1", "application/json"),
        t(
            "app://scene/{sceneId}/xsheet{?from,to}",
            "Exposure sheet",
            "Run-length exposure grid (04)",
            "application/json",
        ),
        t("app://drawing/{drawingId}", "Drawing", "Full vector content (02)", "application/json"),
        t("app://drawing/{drawingId}.png{?w}", "Drawing image", "Rendered drawing", "image/png"),
        t("app://scene/{sceneId}/frame/{frame}.png{?w,camera}", "Frame image", "Rendered frame", "image/png"),
        t("app://palette/{paletteId}", "Palette", "Palette with styles and swatches", "application/json"),
        t("app://library/{libraryId}", "Library", "Library tree", "application/json"),
        t("app://command/{commandId}", "Command", "Command declaration (same as describe_command)", "application/json"),
        t("app://schema/{type}", "Schema", "JSON Schema of an object type", "application/json"),
        t("app://docs/{namespace}", "Namespace docs", "Commands of a namespace with examples", "text/markdown"),
    ]
}

/// What reading a URI means.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    /// A command's result (JSON).
    Command(&'static str, Value),
    /// A command's declaration.
    Describe(String),
    /// A docs topic.
    Docs(String),
    /// Events after a sequence number.
    Events(u64),
    /// This session's activity log.
    Activity,
}

fn query(s: &str) -> (&str, Map<String, Value>) {
    let (path, q) = s.split_once('?').unwrap_or((s, ""));
    let mut m = Map::new();
    for pair in q.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = decode(v);
        m.insert(k.to_owned(), v.parse::<i64>().map_or_else(|_| json!(v), |n| json!(n)));
    }
    (path, m)
}

/// Percent-decoding of a URI component.
#[must_use]
pub fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (
                b.get(i + 1).and_then(|c| (*c as char).to_digit(16)),
                b.get(i + 2).and_then(|c| (*c as char).to_digit(16)),
            )
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolves a URI. `None`: no such resource.
#[must_use]
pub fn resolve(uri: &str) -> Option<Read> {
    let rest = uri.strip_prefix("app://")?;
    let (path, q) = query(rest);
    let parts: Vec<&str> = path.split('/').collect();
    Some(match parts.as_slice() {
        ["state"] => Read::Command("app.state.get", json!({})),
        ["project"] => Read::Command("doc.object.get", json!({ "ref": "project", "depth": 1 })),
        ["object", id] if !id.is_empty() => Read::Command("doc.object.get", json!({ "ref": decode(id), "depth": 1 })),
        ["scene", scene, "xsheet"] => {
            let mut p = Map::new();
            p.insert("sceneId".into(), json!(decode(scene)));
            for k in ["from", "to"] {
                if let Some(v) = q.get(k) {
                    p.insert(k.into(), v.clone());
                }
            }
            Read::Command("xsheet.exposures.get", Value::Object(p))
        }
        ["drawing", d] if d.ends_with(".png") => {
            let mut p = json!({ "drawingId": decode(d.trim_end_matches(".png")) });
            if let (Some(w), Some(m)) = (q.get("w"), p.as_object_mut()) {
                m.insert("width".into(), w.clone());
            }
            Read::Command("draw.drawing.render", p)
        }
        ["drawing", d] if !d.is_empty() => Read::Command("doc.object.get", json!({ "ref": decode(d), "depth": -1 })),
        ["scene", scene, "frame", f] if f.ends_with(".png") => {
            let frame: u64 = f.trim_end_matches(".png").parse().ok()?;
            let mut p = json!({ "sceneId": decode(scene), "frame": frame, "composition": "camera", "format": "png" });
            if let (Some(w), Some(m)) = (q.get("w"), p.as_object_mut()) {
                m.insert("width".into(), w.clone());
            }
            Read::Command("render.frame.run", p)
        }
        ["palette", id] if !id.is_empty() => Read::Command("doc.object.get", json!({ "ref": decode(id), "depth": -1 })),
        ["library", id] if !id.is_empty() => Read::Command("doc.object.get", json!({ "ref": decode(id), "depth": -1 })),
        ["command", id] if !id.is_empty() => Read::Describe(decode(id)),
        ["schema", ty] if !ty.is_empty() => Read::Command("doc.schema.get", json!({ "type": decode(ty) })),
        ["docs", topic] if !topic.is_empty() => Read::Docs(decode(topic)),
        ["events"] => Read::Events(q.get("after").and_then(Value::as_u64).unwrap_or(0)),
        ["activity"] => Read::Activity,
        _ => return None,
    })
}

/// The base of a URI (without query) for matching subscriptions.
fn base(uri: &str) -> &str {
    uri.split('?').next().unwrap_or(uri)
}

/// Whether an executor event updates a subscribed URI (11 §9.1: state, project, objects, xsheet, drawings, palettes,
/// events change). `changed`: the ids of the changed objects (from `doc.changed`).
#[must_use]
pub fn affected(uri: &str, event: &str, changed: &[String]) -> bool {
    let b = base(uri);
    if b == "app://events" {
        return true;
    }
    match event {
        "project.opened" | "project.closed" | "project.saved" => !b.starts_with("app://docs/") && b != "app://activity",
        "doc.changed" => {
            b == "app://state"
                || b == "app://project"
                || changed.iter().any(|id| {
                    b == format!("app://object/{id}")
                        || b == format!("app://drawing/{id}")
                        || b == format!("app://palette/{id}")
                        || b.starts_with(&format!("app://scene/{id}/"))
                })
        }
        e if e.starts_with("app.")
            || e.starts_with("window.")
            || e.starts_with("workspace.")
            || e.starts_with("job.")
            || e.starts_with("state.") =>
        {
            b == "app://state"
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
    use super::*;

    #[test]
    fn uris_resolve_to_commands_and_events_update_subscriptions() {
        assert_eq!(resolve("app://state"), Some(Read::Command("app.state.get", json!({}))));
        assert_eq!(
            resolve("app://object/el_1"),
            Some(Read::Command("doc.object.get", json!({ "ref": "el_1", "depth": 1 })))
        );
        assert_eq!(
            resolve("app://scene/sc_1/xsheet?from=2&to=9"),
            Some(Read::Command("xsheet.exposures.get", json!({ "sceneId": "sc_1", "from": 2, "to": 9 })))
        );
        assert_eq!(resolve("app://events?after=41"), Some(Read::Events(41)));
        assert_eq!(resolve("app://docs/doc"), Some(Read::Docs("doc".into())));
        assert_eq!(
            resolve("app://object/scene%3A%22Intro%22"),
            Some(Read::Command("doc.object.get", json!({ "ref": "scene:\"Intro\"", "depth": 1 })))
        );
        assert_eq!(resolve("file:///etc/passwd"), None);
        assert_eq!(resolve("app://nope"), None);
        let ids = vec!["el_1".to_owned()];
        assert!(affected("app://object/el_1", "doc.changed", &ids));
        assert!(!affected("app://object/el_2", "doc.changed", &ids));
        assert!(affected("app://events?after=3", "anything", &[]));
        assert!(affected("app://state", "app.state.changed", &[]));
        assert!(!affected("app://docs/agent-guide", "doc.changed", &ids));
    }
}
