//! The parity gate's checks (parity note 9; rulings D-25, D-31, D-35, D-36), shared by every test binary that declares
//! `mod common;`: the builder's `tests/c43/parity.rs` and the reviewers' files call these functions, so a fixture in
//! any of them runs the one gate.
//!
//! - **PA2** ([`pa2`]): every command a GUI action of the manifest sends is registered, not an event, and its fixed
//!   parameters validate once completed as the GUI runs them.
//! - **PA3** ([`pa3`], [`pa3_raw`]): every implemented, non-event command is exactly one of bound, agent-only or
//!   deferred. Bound (D-35): a mutating command only by an action entry of the manifest (a menu item, a keymap default,
//!   a rendered actionable element, a toolbar item, a gesture row or a window-gesture row); a query only by a rendered
//!   surface's declared read (`reads.json`). A mention in the source binds nothing.
//! - **Deferred entries** (D-36, [`deferred_problems`]): the owner is a checkpoint of the catalogue generated from
//!   `plan/IMPLEMENTATION_PLAN.md` §4 plus M4 C4.4, in its exact syntax, and has not passed; every cited section
//!   heading exists in `spec/`; the command is registered, implemented and not an event; a duplicate entry fails while
//!   the raw file is parsed ([`deferred_entries`]).
//! - **Declared reads** (D-35, [`reads_problems`]): every query call site in product code (the manifest's `callSites`)
//!   is declared under a surface or under plumbing, and every declared read has a call site in a file it names.
//! - **Window gestures** ([`window_gesture_problems`]): each row's commands have a call site in a file the row names.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use tt_executor::host::KeyResolution;
use tt_registry::{Kind, Status, validate_params};

pub const MANIFEST: &str = include_str!("../../../../desktop/src/ui-bindings/parity.json");
pub const DEFERRED: &str = include_str!("../../../../desktop/src/ui-bindings/deferred.json");
pub const READS: &str = include_str!("../../../../desktop/src/ui-bindings/reads.json");
pub const WINDOW_GESTURES: &str = include_str!("../../../../desktop/src/ui-bindings/window-gestures.json");
pub const GESTURES: &str = include_str!("../../src/gesture-table.json");
pub const TOOLBAR: &str = include_str!("../../src/toolbar-default.json");
const PLAN: &str = include_str!("../../../../plan/IMPLEMENTATION_PLAN.md");

/// The spec files, by their two-digit prefix (`A` for the command registry).
const SPEC: &[(&str, &str)] = &[
    ("00", include_str!("../../../../spec/00_overview.md")),
    ("01", include_str!("../../../../spec/01_application_shell.md")),
    ("02", include_str!("../../../../spec/02_drawing.md")),
    ("03", include_str!("../../../../spec/03_colour.md")),
    ("04", include_str!("../../../../spec/04_timeline_xsheet.md")),
    ("05", include_str!("../../../../spec/05_scene_planning.md")),
    ("06", include_str!("../../../../spec/06_effects.md")),
    ("07", include_str!("../../../../spec/07_sound_lipsync.md")),
    ("08", include_str!("../../../../spec/08_library_storyline.md")),
    ("09", include_str!("../../../../spec/09_playback_export.md")),
    ("10", include_str!("../../../../spec/10_data_model_project_format.md")),
    ("11", include_str!("../../../../spec/11_agent_interface_mcp.md")),
];

/// The sections of GUI actions (each entry: `{id, surface, commands, params?}`; the gestures: `{view, tool, commands}`).
pub const SECTIONS: &[&str] = &["dom", "menu", "keymap", "toolbar", "gestures", "windowGestures"];

pub fn manifest() -> Value {
    serde_json::from_str(MANIFEST).unwrap()
}

/// Every (command, params, where) a GUI action of `m` sends.
pub fn actions(m: &Value) -> Vec<(String, Option<Value>, String)> {
    let mut out = Vec::new();
    for sec in SECTIONS {
        for a in m[*sec].as_array().map(Vec::as_slice).unwrap_or_default() {
            let at = format!("{sec}:{}", a["id"].as_str().or(a["tool"].as_str()).unwrap_or("?"));
            for c in a["commands"].as_array().map(Vec::as_slice).unwrap_or_default() {
                out.push((c.as_str().unwrap_or_default().to_owned(), a.get("params").cloned(), at.clone()));
            }
        }
    }
    out
}

/// PA2 over a manifest: the problems (an unregistered command, an event, parameters that do not validate). Each
/// action's parameters are completed the way the GUI runs them (`complete_reference`: a toggle the reference leaves
/// open gets its next state), in a fixture with a project open; when required parameters are still missing the GUI
/// collects them with a dialog (01 §0), so the parameters given are checked against the schema without `required`.
pub fn pa2(m: &Value) -> Vec<String> {
    let r = tt_executor::registry().unwrap();
    let h = super::fixture();
    let mut out = Vec::new();
    for (id, params, at) in actions(m) {
        let Some(e) = r.get(&id) else {
            out.push(format!("{at}: `{id}` is not registered"));
            continue;
        };
        if e.spec.kind == Kind::Event {
            out.push(format!("{at}: `{id}` is an event"));
            continue;
        }
        // A gesture's parameters are the driver's (its samples), a window gesture's the window's: the battery of the
        // gesture table and the window-gesture call sites cover them.
        if at.starts_with("gestures:") || at.starts_with("windowGestures:") {
            continue;
        }
        let given = params.and_then(|p| p.as_object().cloned()).unwrap_or_default();
        let (params, dialog) = match h.complete_reference(&id, given) {
            Ok(KeyResolution::Command { params, dialog, .. }) => (params, dialog),
            other => {
                out.push(format!("{at}: `{id}` does not complete: {other:?}"));
                continue;
            }
        };
        let mut schema = e.params_schema();
        if dialog && let Some(o) = schema.as_object_mut() {
            o.remove("required");
        }
        // `validate_params` caches the compiled schema by its key: the schema without `required` has its own key, or
        // a strict check of the same command earlier in the manifest (an action that gives every parameter) is reused.
        let key = if dialog { format!("{id}#dialog") } else { id.clone() };
        if let Err(err) = validate_params(&key, &schema, &params) {
            out.push(format!("{at}: `{id}` {params}: {}", err.message));
        }
    }
    out
}

// ----- D-36: the deferred table ---------------------------------------------------------------------------------------

/// The checkpoint catalogue: every `C<m>.<k>` the table of `plan/IMPLEMENTATION_PLAN.md` §4 lists on milestone `M<m>`'s
/// row, plus M4 C4.4 (ruling D-30), each as `M<m> C<m>.<k>`.
pub fn catalogue() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let section = PLAN.split("## 4. ").nth(1).unwrap_or_default();
    let section = section.split("\n## ").next().unwrap_or_default();
    for line in section.lines() {
        let Some(rest) = line.strip_prefix("| M") else { continue };
        let Some(m) = rest.split(' ').next().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let bytes = line.as_bytes();
        for (i, _) in line.match_indices('C') {
            let digits = |from: usize| bytes[from..].iter().take_while(|b| b.is_ascii_digit()).count();
            let major = digits(i + 1);
            if major == 0 || bytes.get(i + 1 + major) != Some(&b'.') {
                continue;
            }
            let minor = digits(i + 2 + major);
            if minor == 0 || i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
                continue;
            }
            let cm: u32 = line[i + 1..i + 1 + major].parse().unwrap_or(u32::MAX);
            if cm == m {
                out.insert(format!("M{m} {}", &line[i..i + 2 + major + minor]));
            }
        }
    }
    out.insert("M4 C4.4".to_owned());
    out
}

/// Whether an owner has passed when C4.3b hands off: every checkpoint of M0-M3 and M4 C4.1-C4.3.
fn passed(m: u32, k: u32) -> bool {
    m < 4 || (m == 4 && k <= 3)
}

/// The problem with an owner, if any: its exact syntax `M<m> C<m>.<k>`, the catalogue, and not passed (D-31, D-36).
pub fn owner_problem(owner: &str, catalogue: &BTreeSet<String>) -> Option<String> {
    let syntax = || format!("owner `{owner}` is not `M<m> C<m>.<k>`");
    let rest = owner.strip_prefix('M')?;
    let Some((m, c)) = rest.split_once(" C") else { return Some(syntax()) };
    let Some((cm, k)) = c.split_once('.') else { return Some(syntax()) };
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !(all_digits(m) && all_digits(cm) && all_digits(k)) || m != cm {
        return Some(syntax());
    }
    if !catalogue.contains(owner) {
        return Some(format!("owner `{owner}` is not a checkpoint of the plan (§4) or M4 C4.4"));
    }
    let (m, k) = (m.parse().unwrap_or(0), k.parse().unwrap_or(0));
    passed(m, k).then(|| format!("owner `{owner}` has passed (D-31)"))
}

/// An owner without the `M` prefix (`5 C5.1`) is caught here: [`owner_problem`]'s first test returns `None` for it.
fn owner_check(owner: &str, catalogue: &BTreeSet<String>) -> Option<String> {
    if !owner.starts_with('M') {
        return Some(format!("owner `{owner}` is not `M<m> C<m>.<k>`"));
    }
    owner_problem(owner, catalogue)
}

/// The section references of a citation: `(file, section)` for every `§x.y`, the file the nearest two-digit number
/// before it names (`02 §6, §7.7` cites 02 §6 and 02 §7.7).
pub fn section_refs(citation: &str) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    let mut file: Option<String> = None;
    let chars: Vec<char> = citation.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch != '§' {
            continue;
        }
        // A two-digit file number before the sign (spaces between), not part of a longer number.
        let mut j = i;
        while j > 0 && chars[j - 1] == ' ' {
            j -= 1;
        }
        if j >= 2
            && chars[j - 1].is_ascii_digit()
            && chars[j - 2].is_ascii_digit()
            && (j < 3 || !(chars[j - 3].is_ascii_digit() || chars[j - 3] == '.' || chars[j - 3] == '§'))
        {
            file = Some(chars[j - 2..j].iter().collect());
        }
        let mut number = String::new();
        let mut k = i + 1;
        while k < chars.len()
            && (chars[k].is_ascii_digit() || (chars[k] == '.' && k + 1 < chars.len() && chars[k + 1].is_ascii_digit()))
        {
            number.push(chars[k]);
            k += 1;
        }
        if !number.is_empty() {
            out.push((file.clone(), number));
        }
    }
    out
}

/// Whether `spec/<file>_*.md` has a heading numbered `section` (`## 6.`, `### 6.4 Text`).
fn heading_exists(file: &str, section: &str) -> bool {
    let Some((_, text)) = SPEC.iter().find(|(f, _)| *f == file) else { return false };
    text.lines().any(|l| {
        let t = l.trim_start_matches('#');
        t.len() < l.len()
            && t.starts_with(' ')
            && t.trim_start()
                .strip_prefix(section)
                .is_some_and(|after| after.starts_with('.') || after.starts_with(' '))
    })
}

/// The problems of a citation: none cited, a reference without a file, a heading that does not exist (D-36).
pub fn section_problems(id: &str, citation: &str) -> Vec<String> {
    let refs = section_refs(citation);
    if refs.is_empty() {
        return vec![format!("deferred `{id}`: no spec section cited (D-31)")];
    }
    refs.into_iter()
        .filter_map(|(file, section)| match file {
            None => Some(format!("deferred `{id}`: §{section} names no spec file")),
            Some(f) if !heading_exists(&f, &section) => {
                Some(format!("deferred `{id}`: spec {f} has no heading §{section} (D-36)"))
            }
            Some(_) => None,
        })
        .collect()
}

/// The entries of the raw deferred table in file order, a duplicate key of the `deferred` object refused (D-36): a
/// duplicate is lost when the text is parsed into a map, so the keys are read from the text first.
pub fn deferred_entries(raw: &str) -> Result<Vec<(String, Value)>, String> {
    let keys = deferred_keys(raw)?;
    let mut seen = BTreeSet::new();
    for k in &keys {
        if !seen.insert(k.clone()) {
            return Err(format!("deferred `{k}` is listed twice (D-36)"));
        }
    }
    let doc: Value = serde_json::from_str(raw).map_err(|e| format!("deferred.json: {e}"))?;
    let map = doc["deferred"].as_object().ok_or("deferred.json: no `deferred` object")?;
    keys.into_iter()
        .map(|k| match map.get(&k) {
            Some(v) => Ok((k, v.clone())),
            None => Err(format!("deferred `{k}`: the key read from the text is not the decoded key (D-36)")),
        })
        .collect()
}

/// The keys of the top-level `deferred` object, in order, read from the text (a small JSON scanner: strings with
/// escapes, nesting depth). Ruling on C43B-R2-02: every key in the file, at any depth, is plain ASCII with no escape
/// sequence, so the text's keys are the decoded keys; a key with a backslash or a non-ASCII byte is refused here,
/// before anything is decoded.
fn deferred_keys(raw: &str) -> Result<Vec<String>, String> {
    let b = raw.as_bytes();
    let (mut i, mut depth) = (0usize, 0usize);
    let mut root_key = String::new();
    let mut inside = false;
    let mut keys = Vec::new();
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                let text = &b[start..i.min(b.len())];
                let s = String::from_utf8_lossy(text).into_owned();
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if b.get(j) == Some(&b':') {
                    if text.contains(&b'\\') || !text.is_ascii() {
                        return Err(format!(
                            "deferred.json: key `{s}` has an escape sequence or a non-ASCII character; keys are plain \
                             ASCII (D-36, C43B-R2-02)"
                        ));
                    }
                    if depth == 1 {
                        root_key = s;
                    } else if depth == 2 && inside {
                        keys.push(s);
                    }
                }
            }
            b'{' | b'[' => {
                depth += 1;
                if depth == 2 && b[i] == b'{' && root_key == "deferred" {
                    inside = true;
                }
            }
            b'}' | b']' => {
                if depth == 2 {
                    inside = false;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        return Err("deferred.json: unbalanced braces".into());
    }
    Ok(keys)
}

/// D-36 over the deferred entries: owner, sections, location, and the command registered, implemented, not an event.
pub fn deferred_problems(entries: &[(String, Value)]) -> Vec<String> {
    let r = tt_executor::registry().unwrap();
    let catalogue = catalogue();
    let mut out = Vec::new();
    for (id, v) in entries {
        let owner = v["owner"].as_str().unwrap_or_default();
        if let Some(p) = owner_check(owner, &catalogue) {
            out.push(format!("deferred `{id}`: {p}"));
        }
        out.extend(section_problems(id, v["section"].as_str().unwrap_or_default()));
        if v["location"].as_str().is_none_or(str::is_empty) {
            out.push(format!("deferred `{id}`: no GUI location"));
        }
        match r.get(id) {
            None => out.push(format!("deferred `{id}` is not registered")),
            Some(e) if e.spec.kind == Kind::Event => out.push(format!("deferred `{id}` is an event")),
            Some(e) if !matches!(e.status(), Status::Implemented) => {
                out.push(format!("deferred `{id}` is not implemented"));
            }
            Some(_) => {}
        }
    }
    out
}

// ----- D-35: declared reads -------------------------------------------------------------------------------------------

/// The declared reads: `(query, file)` pairs under the surfaces, and under plumbing; and the surface keys themselves,
/// which the gate checks against the collected surfaces (C43B-R2-01).
pub struct Reads {
    pub surfaces: BTreeSet<(String, String)>,
    pub plumbing: BTreeSet<(String, String)>,
    /// The `surfaces` object's keys, in file order.
    pub surface_names: Vec<String>,
}

pub fn reads(raw: &str) -> Reads {
    let doc: Value = serde_json::from_str(raw).unwrap();
    let mut surfaces = BTreeSet::new();
    let mut surface_names = Vec::new();
    for (name, s) in doc["surfaces"].as_object().unwrap() {
        surface_names.push(name.clone());
        for (q, files) in s["reads"].as_object().unwrap() {
            for f in files.as_array().unwrap() {
                surfaces.insert((q.clone(), f.as_str().unwrap().to_owned()));
            }
        }
    }
    let mut plumbing = BTreeSet::new();
    for (q, p) in doc["plumbing"].as_object().unwrap() {
        for f in p["files"].as_array().unwrap() {
            plumbing.insert((q.clone(), f.as_str().unwrap().to_owned()));
        }
    }
    Reads { surfaces, plumbing, surface_names }
}

/// The manifest's call sites, `(command, file)`.
pub fn call_sites(m: &Value) -> BTreeSet<(String, String)> {
    m["callSites"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|c| {
            (c["command"].as_str().unwrap_or_default().to_owned(), c["file"].as_str().unwrap_or_default().to_owned())
        })
        .collect()
}

/// D-35's two directions between the declared reads and the call sites of queries in product code.
pub fn reads_problems(m: &Value, reads_raw: &str) -> Vec<String> {
    let r = tt_executor::registry().unwrap();
    let rd = reads(reads_raw);
    let sites = call_sites(m);
    let is_query = |id: &str| r.get(id).is_some_and(|e| e.spec.kind == Kind::Query);
    let mut out = Vec::new();
    // C43B-R2S-01, C43B-R2-01: every surface key is a collected surface (the manifest's `surfaces`, or `menu` for the
    // native menu the manifest collects from `menu.json`), so a read declared under a surface that does not exist, or a
    // real surface renamed to an invented one, is refused.
    let collected: BTreeSet<&str> = m["surfaces"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .chain(std::iter::once("menu"))
        .collect();
    for name in &rd.surface_names {
        if !collected.contains(name.as_str()) {
            out.push(format!("reads.json: surface `{name}` is not a surface the collector renders"));
        }
    }
    for (q, f) in &sites {
        if is_query(q)
            && !rd.surfaces.contains(&(q.clone(), f.clone()))
            && !rd.plumbing.contains(&(q.clone(), f.clone()))
        {
            out.push(format!("`{q}` is read in {f} but no surface or plumbing entry declares it (reads.json)"));
        }
    }
    for (q, f) in rd.surfaces.iter().chain(rd.plumbing.iter()) {
        match r.get(q) {
            None => out.push(format!("reads.json: `{q}` is not registered")),
            Some(e) if e.spec.kind != Kind::Query => out.push(format!("reads.json: `{q}` is not a query")),
            Some(e) if !matches!(e.status(), Status::Implemented) => {
                out.push(format!("reads.json: `{q}` is not implemented"));
            }
            Some(_) => {}
        }
        if !sites.contains(&(q.clone(), f.clone())) {
            out.push(format!("reads.json declares `{q}` in {f}, where product code never calls it"));
        }
    }
    for pair in rd.surfaces.intersection(&rd.plumbing) {
        out.push(format!("reads.json: `{}` in {} is both a surface read and plumbing", pair.0, pair.1));
    }
    out
}

/// Each window-gesture row's commands are registered, not events, and called in a file the row names; the manifest's
/// section is the file's rows.
pub fn window_gesture_problems(m: &Value, raw: &str) -> Vec<String> {
    let r = tt_executor::registry().unwrap();
    let doc: Value = serde_json::from_str(raw).unwrap();
    let sites = call_sites(m);
    let mut out = Vec::new();
    let mut listed = Vec::new();
    for row in doc["rows"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap_or_default();
        let files: Vec<&str> = row["files"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        for c in row["commands"].as_array().unwrap().iter().filter_map(Value::as_str) {
            if r.get(c).is_none_or(|e| e.spec.kind == Kind::Event) {
                out.push(format!("window gesture {id}: `{c}` is not a registered command"));
            }
            if !files.iter().any(|f| sites.contains(&(c.to_owned(), (*f).to_owned()))) {
                out.push(format!("window gesture {id}: no file it names calls `{c}`"));
            }
        }
        listed.push(serde_json::json!({ "id": row["id"], "surface": row["surface"], "commands": row["commands"] }));
    }
    if m["windowGestures"] != Value::Array(listed) {
        out.push(
            "the manifest's windowGestures section is not window-gestures.json (pnpm --dir desktop parity:gen)".into(),
        );
    }
    out
}

// ----- PA3 ------------------------------------------------------------------------------------------------------------

/// PA3 over a manifest and a parsed deferred table (a duplicate is already lost: [`pa3_raw`] refuses it), with the
/// committed `reads.json`.
pub fn pa3(m: &Value, deferred: &Value) -> (Vec<String>, BTreeMap<&'static str, usize>) {
    let entries: Vec<(String, Value)> = deferred["deferred"]
        .as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    pa3_entries(m, &entries, READS)
}

/// PA3 over a manifest and the raw deferred table: a duplicate entry fails before anything else (D-36).
pub fn pa3_raw(m: &Value, deferred_raw: &str) -> (Vec<String>, BTreeMap<&'static str, usize>) {
    match deferred_entries(deferred_raw) {
        Ok(entries) => pa3_entries(m, &entries, READS),
        Err(e) => (vec![e], BTreeMap::new()),
    }
}

/// The three states (D-25, D-31, D-35) over deferred entries and a reads table.
pub fn pa3_entries(
    m: &Value,
    entries: &[(String, Value)],
    reads_raw: &str,
) -> (Vec<String>, BTreeMap<&'static str, usize>) {
    let r = tt_executor::registry().unwrap();
    let sent: BTreeSet<String> = actions(m).into_iter().map(|a| a.0).collect();
    let read: BTreeSet<String> = reads(reads_raw).surfaces.into_iter().map(|(q, _)| q).collect();
    let deferred: BTreeSet<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    let mut problems = Vec::new();
    let mut counts = BTreeMap::new();
    for e in r.entries() {
        if e.spec.kind == Kind::Event || !matches!(e.status(), Status::Implemented) {
            continue;
        }
        let id = e.id();
        let agent = e.command.and_then(|c| c.decl.agent_only).filter(|j| j.len() > 10);
        let bound = sent.contains(id) || (e.spec.kind == Kind::Query && read.contains(id));
        let states: Vec<&'static str> =
            [bound.then_some("bound"), agent.map(|_| "agent-only"), deferred.contains(id).then_some("deferred")]
                .into_iter()
                .flatten()
                .collect();
        match states.as_slice() {
            [one] => *counts.entry(*one).or_insert(0) += 1,
            [] => problems.push(format!("`{id}` is in no state: bind it, declare it agent-only, or defer it")),
            many => problems.push(format!("`{id}` is in several states: {many:?}")),
        }
    }
    problems.extend(deferred_problems(entries));
    (problems, counts)
}

/// The commands in each state (for the handoff's lists).
pub fn states(m: &Value, deferred_raw: &str) -> BTreeMap<&'static str, Vec<String>> {
    let r = tt_executor::registry().unwrap();
    let entries = deferred_entries(deferred_raw).unwrap();
    let sent: BTreeSet<String> = actions(m).into_iter().map(|a| a.0).collect();
    let read: BTreeSet<String> = reads(READS).surfaces.into_iter().map(|(q, _)| q).collect();
    let deferred: BTreeSet<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    let mut out: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for e in r.entries() {
        if e.spec.kind == Kind::Event || !matches!(e.status(), Status::Implemented) {
            continue;
        }
        let id = e.id();
        let state = if e.command.and_then(|c| c.decl.agent_only).is_some() {
            "agent-only"
        } else if deferred.contains(id) {
            "deferred"
        } else if sent.contains(id) {
            "bound (action)"
        } else if e.spec.kind == Kind::Query && read.contains(id) {
            "bound (read)"
        } else {
            "none"
        };
        out.entry(state).or_default().push(id.to_owned());
    }
    out
}
