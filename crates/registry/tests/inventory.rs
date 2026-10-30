//! The generated inventory equals spec A (`scripts/gen-command-inventory.py` regenerates it).
#![allow(clippy::unwrap_used, clippy::panic, reason = "test")]

use std::collections::BTreeSet;
use tt_registry::inventory::INVENTORY;

/// Spec A rows as (id, kind, cells…), parsed independently of the generator.
fn spec_rows() -> Vec<(String, String, Vec<String>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/A_command_registry.md");
    let text = std::fs::read_to_string(path).unwrap();
    let mut rows = Vec::new();
    let mut in_namespace = false;
    for line in text.lines() {
        if line.starts_with("## Namespace `") {
            in_namespace = true;
            continue;
        }
        if !in_namespace || !line.starts_with("| `") {
            continue;
        }
        // Split on `|` not preceded by a backslash.
        let mut cells = Vec::new();
        let mut cur = String::new();
        let mut chars = line.trim().trim_start_matches('|').trim_end_matches('|').chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' && chars.peek() == Some(&'|') {
                cur.push('|');
                chars.next();
            } else if c == '|' {
                cells.push(cur.trim().to_owned());
                cur.clear();
            } else {
                cur.push(c);
            }
        }
        cells.push(cur.trim().to_owned());
        let id = cells[0].trim_matches('`').to_owned();
        let kind = cells[1].clone();
        rows.push((id, kind, cells[2..].to_vec()));
    }
    rows
}

#[test]
fn inventory_matches_spec_a_exactly() {
    let rows = spec_rows();
    assert_eq!(rows.len(), 796, "spec A states 796 entries");
    assert_eq!(INVENTORY.len(), rows.len());
    for (e, (id, kind, cells)) in INVENTORY.iter().zip(&rows) {
        assert_eq!(e.id, id);
        assert_eq!(e.kind.as_str(), kind, "{id}");
        assert_eq!([e.summary, e.params, e.result, e.undo].map(str::to_owned).to_vec(), *cells, "{id}");
        assert!(id.starts_with(&format!("{}.", e.namespace)), "{id}");
    }
    let ids: BTreeSet<&str> = INVENTORY.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), INVENTORY.len(), "ids are unique");
}
