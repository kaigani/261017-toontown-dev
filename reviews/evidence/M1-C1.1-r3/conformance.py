"""Preserve the 428 stable requirements/193 deferrals; update only evidenced R3 decisions."""
from pathlib import Path
from collections import Counter
import json
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
old = json.loads((HERE.parent / "M1-C1.1-r2/conformance.json").read_text(encoding="utf-8"))
rows = old["rows"]
M = "crates/model/tests/c11_r3_adversarial.rs"
R = "crates/model/tests/c11_r3_register.rs"
W = "crates/engine/tests/c11_r3_wire.rs"

def location(file, token):
    lines = (ROOT / file).read_text(encoding="utf-8").splitlines()
    return file + ":" + str(next(i for i,l in enumerate(lines,1) if token in l))

def test(name, file=M):
    return dict(name=name, location=location(file, "fn " + name + "("), command=f"cargo test -p {'tt-engine' if file==W else 'tt-model'} --test {Path(file).stem} {name} --locked -- --exact --nocapture")

decisions = {
    2: ("implemented", "input.rs", "fn child_schemas", "frame_map_keys_and_nfc_values_at_fresh_boundaries", "R2-10 closed: declared pattern-map values normalize; nondecimal Unicode frame keys reject.", M),
    3: ("implemented", "schema.rs", "pub const FRAME_KEY_PATTERN", "frame_map_keys_and_nfc_values_at_fresh_boundaries", "R2-08 closed: signed bounds on frame-map keys.", M),
    11: ("implemented", "validate.rs", "fn is_frame", "frame_map_keys_and_nfc_values_at_fresh_boundaries", "R2-08 closed: frame keys positive, within signed-32-bit range.", M),
    177: ("implemented", "validate.rs", "for frame in t.labels.keys()", "frame_map_keys_and_nfc_values_at_fresh_boundaries", "R2-08/10 closed: labels are bounded placement-relative frames; values NFC.", M),
    388: ("divergent", "change.rs", "pub fn apply_with_inverse", "resolved_stateless_move_inverse_does_not_shift_numeric_object_keys", "Pre-state RFC property passes 10,000 cases, but C11-R3-02 corrupts numeric object keys via public stateless invert and C11-R3-03 accepts malformed exposure ranges. R3-01 breaks opaque diffs.", M),
    392: ("divergent", "schema.rs", "fn annotate_since", "anonymous_variant_property_declarations_have_since", "R2-11 fixes MouthTrack.stale and Stub; C11-R3-06: DynamicParam anonymous value/keyframes declarations still lack x-since.", M),
    394: ("divergent", "schema.rs", "pub fn property_schema", "property_schema_preserves_nullable_value_domains", "Nested traversal and ref annotations fixed; C11-R3-04: flat merge intersects alternatives, rejecting null and non-first enum values.", M),
    398: ("implemented", "schema.rs", "pub const FRAME_KEY_PATTERN", "frame_map_keys_and_nfc_values_at_fresh_boundaries", "R2-08/10 closed: frame map bounds and NFC values; prior 10,000-character controls pass.", M),
    412: ("implemented", "canonical.rs", "pub fn to_canonical_string", "mixed_depth_boundary_writer_output_is_readable", "R2-07 closed: text depth 128, including root and scalar, agrees in Rust and TypeScript.", W),
    56: ("divergent", "change.rs", 'if path.ends_with("/mouthTrack/mouths")', "opaque_mouth_track_lookalikes_are_ordinary_json_changes", "Reopened C11-R3-01: diff applies mouth-letter semantics to opaque metadata lookalikes; valid pen edits cannot be applied.", M),
    57: ("divergent", "change.rs", 'if path.ends_with("/mouthTrack/mouths")', "opaque_mouth_track_lookalikes_are_ordinary_json_changes", "Reopened C11-R3-01: same regression under reverse-DNS ext namespaces.", M),
}
closure = []
for row,(status,source,anchor,name,note,file) in decisions.items():
    r = rows[row-1]
    r["r2_status"] = r["status"]
    r["r2_note"] = r["note"]
    r.update(status=status, note=note, implementation=location("crates/model/src/"+source,anchor), r3_test=test(name,file))
    closure.append(dict(register_row=row, **r))

spot = list(range(111,125)) + list(range(128,142)) + [146,148]
assert len(spot)==30 and not set(spot).intersection(old["implemented_spotchecks"])
for row in spot:
    r=rows[row-1]
    assert r["status"]=="implemented"
    if row<=124:
        name="rows_111_to_124_element_variant_blocks_are_required_and_typed"
        source=location("crates/model/src/schema.rs", "fn add_element_rules")
    elif row<=137:
        name="rows_128_to_137_transform_constraints_and_pivots"
        source=location("crates/model/src/entities/element.rs", "pub struct Transform")
    elif row<=141:
        name="rows_138_to_141_camera_export_skeleton_and_bone_blocks"
        source=location("crates/model/src/entities/element.rs", "pub struct Camera") if row==138 else location("crates/model/src/validate.rs", "impl Check for Element")
    else:
        name="rows_146_and_148_drawing_pivot_and_ordered_nodes"
        source=location("crates/model/src/entities/drawing.rs", "pub struct Drawing {")
    r["r3_spotcheck"]=dict(**test(name,R), implementation=source, scope="Persisted model shape/constraints; accepted command/rendering deferrals retain their owners.")
counts=dict(Counter(r["status"] for r in rows)); counts.setdefault("missing",0)
assert counts["deferred"]==193
out=dict(base="eb54b55b44d6e744ab5f2b0d74b4213b604798f3", counts=counts, rows=rows, implemented_spotchecks=spot, r2_spotchecks=old["implemented_spotchecks"])
(HERE/"conformance.json").write_text(json.dumps(out,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
def esc(value):
    return str(value).replace("|", "\\|").replace("\n", " ")
md=["# C1.1 round 3 conformance register", "", f"428 stable requirements: {counts}. All 193 deferrals retain their original owners.", "", "R3 updates eleven rows (nine closure claims plus two regressions) and spot-checks 30 other implemented rows. Other rows retain R2 evidence; historical R1/R2 notes are superseded by the final review closure tables. Future command behavior is not claimed from model representation tests.", "", "| Row | Citation | Requirement | Status | Source / owner | Note / R3 test |", "|---|---|---|---|---|---|"]
for r in rows:
    t=r.get("r3_test",r.get("r3_spotcheck"))
    note=r["note"] + (f"; {t['location']} `{t['name']}`" if t else "")
    md.append("| " + " | ".join(esc(x) for x in [r["row"],r["citation"],r["requirement"],r["status"],r["implementation"],note]) + " |")
(HERE/"conformance.md").write_text("\n".join(md)+"\n",encoding="utf-8")
(HERE/"register-closure.json").write_text(json.dumps(closure,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
(HERE/"spotchecks.json").write_text(json.dumps([rows[i-1] for i in spot],ensure_ascii=False,indent=2)+"\n",encoding="utf-8")

bundlepath=ROOT/"crates/model/schema/model-1.0.schema.json"
bundle=json.loads(bundlepath.read_text(encoding="utf-8"))
inventory=[]
for name,s in bundle["$defs"].items():
    for field,p in s.get("properties",{}).items():
        inventory.append(dict(definition=name,property=field,pointer=f"/$defs/{name}/properties/{field}",schema=p,kind="named-definition property"))
    # Anonymous untagged object alternatives introduce real properties, unlike predicates that merely constrain
    # existing fields. R1/R2's 518-entry inventory omitted these two DynamicParam declarations.
    for keyword in ["anyOf","oneOf"]:
        for i,alt in enumerate(s.get(keyword,[])):
            if alt.get("type")=="object":
                for field,p in alt.get("properties",{}).items():
                    inventory.append(dict(definition=name,property=field,pointer=f"/$defs/{name}/{keyword}/{i}/properties/{field}",schema=p,kind="anonymous variant declaration"))
(HERE/"property-inventory.json").write_text(json.dumps(inventory,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print(json.dumps(dict(requirements=len(rows),counts=counts,spotchecks=spot,properties=len(inventory),missing_since=[p["pointer"] for p in inventory if p["schema"].get("x-since")!="1.0"]),indent=2))
