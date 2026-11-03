//! Repairs on open that need colours and the planar map (C22-R1-05): colour recovery of colour IDs no palette defines
//! (03 §11.2 "loading drawings"; 10 §4.11, §7.2 `colour.recovered` / `colour.placeholderCreated`) and the repair of
//! disagreeing zone fills on stroke sides (10 §4.9 load rule, §7.2 `drawing.fillInconsistent`). They join the
//! persistence layer's repairs under the same REP rules: one "Repair on Open" step, a new document revision with the
//! repaired objects stamped (REP-01, REV-03), each repair listed in the load report, IDs from a generator seeded with
//! the document's content so the same file always repairs the same way (LOAD-03).

use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey, Severity};
use tt_model::id::IdGenerator;
use tt_persist::{LoadIssue, Loaded};
use tt_registry::CommandError;
use tt_vector::work::Work;

use crate::commands::colour_recovery;

/// One repair to report.
struct Found {
    code: &'static str,
    object_id: String,
    message: String,
    details: Value,
}

/// Applies the repairs to a loaded project (unchanged when there is nothing to repair or the repair cannot be staged).
pub(crate) fn repair(mut loaded: Loaded) -> Loaded {
    let Ok(Some((doc, found))) = repaired(&loaded) else { return loaded };
    let n0 = loaded.report.issues.len();
    for (k, f) in found.into_iter().enumerate() {
        let path = doc.canonical_path(&f.object_id);
        loaded.report.issues.push(LoadIssue {
            issue_id: format!("is_{}", n0 + k + 1),
            code: f.code.to_owned(),
            severity: Severity::Warning,
            object_id: Some(f.object_id),
            path,
            entry: None,
            message: f.message,
            details: f.details,
            fixes: Vec::new(),
            repaired: true,
        });
    }
    loaded.report.repaired_count = loaded.report.issues.iter().filter(|i| i.repaired).count();
    loaded.doc = doc;
    loaded.repaired = true;
    loaded
}

fn seed(doc: &Document, missing: &[(String, Option<Value>)]) -> u64 {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(doc.project().id().as_str().as_bytes());
    for (c, _) in missing {
        h.update(c.as_bytes());
    }
    let d: [u8; 32] = h.finalize().into();
    d.iter().take(8).fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

/// The repaired document and what was repaired; `None` when nothing needs repair.
fn repaired(loaded: &Loaded) -> Result<Option<(Document, Vec<Found>)>, CommandError> {
    let doc = &loaded.doc;
    let mut changes: Vec<(EntryKey, Vec<tt_model::change::PatchOp>)> = Vec::new();
    let mut found = Vec::new();
    // Colour recovery.
    let missing = colour_recovery::missing_with_copies(doc);
    if !missing.is_empty() {
        let mut ids = IdGenerator::seeded(seed(doc, &missing), 1_760_486_400_000);
        let (result, recovery) =
            colour_recovery::recover_with(&mut |t| ids.next(t).map_err(CommandError::from), doc, &missing)?;
        let (recovered, placeholders) = colour_recovery::recovered_colours(&result);
        for (colour, palette) in recovered {
            found.push(Found {
                code: "colour.recovered",
                message: format!("colour `{colour}` was missing and was recreated from its recovery copy"),
                details: json!({ "colourId": colour, "paletteId": palette }),
                object_id: colour,
            });
        }
        for colour in placeholders {
            found.push(Found {
                code: "colour.placeholderCreated",
                message: format!("colour `{colour}` was missing without a recovery copy; a placeholder was created"),
                details: json!({ "colourId": colour }),
                object_id: colour,
            });
        }
        changes.extend(recovery);
    }
    // Side fills that disagree.
    for d in doc.drawings() {
        let (ops, layers) = repair_drawing_sides(d);
        for (layer, zones) in layers {
            found.push(Found {
                code: "drawing.fillInconsistent",
                object_id: d.id().as_str().to_owned(),
                message: format!(
                    "{zones} zone(s) of layer `{layer}` had disagreeing fills on their bounding strokes; the fill of                      the longest bounding length was kept"
                ),
                details: json!({ "layerId": layer, "zones": zones }),
            });
        }
        if !ops.is_empty() {
            changes.push((EntryKey::Drawing(d.id().clone()), ops));
        }
    }
    if changes.is_empty() {
        return Ok(None);
    }
    let rev = if loaded.repaired {
        doc.document_rev()
    } else {
        // A repair is a new revision (REV-03); none left: no repair (the persistence layer refuses the same way).
        let Some(r) = doc.document_rev().checked_add(1) else { return Ok(None) };
        r
    };
    let (next, _) = doc.with_patched_entries_undo_only(&changes).map_err(crate::cx::model_error)?;
    let (_, keys) = crate::cx::changes_between(doc, &next, &[], true, rev)?;
    let stamps: Vec<String> = keys.into_iter().filter(|k| next.locate(k).is_some()).collect();
    let stamped = next.with_revisions(rev, &stamps)?;
    Ok(Some((stamped, found)))
}

/// The side-fill repair of one drawing (load and `doc.import.json`; 10 §4.9, §7.2): for each layer whose bounding
/// strokes disagree about a zone's fill, the patch rewriting its strokes' sides to the longest bounding length (no
/// noding: geometry and IDs stay as they are), and the layers repaired with their number of zones. A layer too large to
/// map within the budget is left as stored (its fills are read the same way).
pub(crate) fn repair_drawing_sides(
    d: &tt_model::entities::drawing::Drawing,
) -> (Vec<tt_model::change::PatchOp>, Vec<(String, usize)>) {
    let mut ops = Vec::new();
    let mut layers = Vec::new();
    for l in crate::planar::layers(d) {
        let mut work = Work::new(tt_vector::work::PLANAR_WORK_BUDGET, "repairing the zone fills of a layer");
        let Ok(Some((strokes, zones))) = tt_vector::planar::layer::repair_sides(&l.strokes, &mut work) else {
            continue;
        };
        ops.push(crate::commands::draw::op("replace", &format!("{}/strokes", l.pointer), Some(Value::Array(strokes))));
        layers.push((l.id, zones));
    }
    (ops, layers)
}
