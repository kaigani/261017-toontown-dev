"""Rebuild the spec-row register and a complete generated-schema field inventory.

The verdict overrides below are reviewer judgments, not inferred from field presence.
The inventory is structural evidence, not a claim that schemas enforce semantic rules.
"""
from collections import Counter
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SPEC = ROOT / "spec/10_data_model_project_format.md"
lines = SPEC.read_text(encoding="utf-8").splitlines()
schema_path = ROOT / "crates/model/schema/model-1.0.schema.json"
schema_lines = schema_path.read_text(encoding="utf-8").splitlines()
bundle = json.loads(schema_path.read_text(encoding="utf-8"))
defs = bundle["$defs"]
E = "crates/model/src/entities/"
modules = {"4.1":"project", "4.2":"pen", "4.3":"scene", "4.4":"transition",
           "4.5":"element", "4.6":"element", "4.7":"element", "4.8":"drawing",
           "4.9":"drawing", "4.10":"drawing", "4.11":"drawing", "4.12":"channel",
           "4.13":"palette", "4.14":"sound", "4.15":"sound", "4.16":"asset", "4.17":"library"}
rows=[]

def loc(file, token=None):
    text = (ROOT/file).read_text(encoding="utf-8").splitlines()
    if token:
        snake = re.sub(r"(?<!^)(?=[A-Z])", "_", token).lower()
        for i,line in enumerate(text,1):
            if re.search(r"\b"+re.escape(snake)+r"\s*:",line):
                return f"{file}:{i}"
    return file+":1"

def add(cite, requirement, status, where, note=""):
    rows.append(dict(citation=cite, requirement=requirement, status=status, implementation=where, note=note))

def field_status(section, field):
    key=(section,field)
    divergences={
      ("1.1","string"):15,("1.1","int"):14,("1.1","number"):16,
      ("1.1","frame"):14,("1.1","duration"):14,
      ("2.2","id"):12,("2.2","type"):13,("2.2","meta"):3,("2.2","ext"):3,
      ("4.3","paletteStyles"):6,("4.5","loops"):14,("4.5","note"):15,
      ("4.9","closed"):5,("4.9","text"):5,("4.14","manual"):5,
      ("4.16","link"):5,
      ("4.1","library"):5,
    }
    if key in divergences: return "divergent",f"C11-R1-{divergences[key]:02}"
    if section=="4.1" and field in ["palettes","pens","scenes","drawingBanks"]:
        return "deferred","Shape present; new-project assembly/default content: handoff gap 6, C1.3."
    if section=="4.6" and field=="image":
        return "divergent","C11-R1-05; importOptions itself is SI-008/M10, handoff named deferral."
    if section=="4.6" and field in ["params","blocks"]:
        return "deferred","Static schema present; type descriptors and remaining particle defaults/caps: handoff gap 7, M7."
    if section=="2.7": return "deferred","Markers/strict input exist; patch enforcement and derived computation: C1.2 (gaps 1,3)."
    if section.startswith(("6.","7.","8.","9","10.")):
        return "deferred","C1.3 persistence/doc commands (handoff gap 5; plan 4); command execution C1.2 and MCP transport C1.4."
    return "implemented","Field/type/default representation reviewed; cross-cutting findings remain applicable."

section=""
for i,line in enumerate(lines,1):
    match=re.match(r"#{2,4} (\d+(?:\.\d+)*)\.? ",line)
    if match: section=match[1]
    if not line.startswith("| ") or line.startswith("|---"): continue
    cells=re.split(r"(?<!\\)\|",line)[1:-1]
    cells=[c.strip() for c in cells]
    if not cells or cells[0] in ["Type","Space","Entity","Field","Class","elementType","Block","kind","Rule","Form","File","Code","Command ID","Check","#","Problem"]: continue
    if section=="11": continue
    names=re.findall(r"`([^`]+)`",cells[0]) or [cells[0]]
    # Split combined fields such as createdAt/modifiedAt into their own rows.
    for name in names:
        detail=" | ".join(cells[1:])
        status,note=field_status(section,name)
        if section in modules:
            where=loc(E+modules[section]+".rs", name)
        elif section=="1.1": where=loc("crates/model/src/value.rs")
        elif section=="1.2": where=loc("crates/model/src/value.rs")
        elif section=="1.3": where="crates/model/src/name.rs:30; crates/model/src/validate.rs:103"
        elif section=="2.1": where="crates/model/src/types.rs:1; crates/model/src/document/index.rs:1"
        elif section=="2.2": where="crates/model/src/entities/mod.rs:1; crates/model/src/object.rs:37"
        elif section=="2.7": where="crates/model/src/schema.rs:1; crates/model/src/object.rs:37"
        elif section=="5":
            where="crates/model/src/document/mod.rs:398; crates/model/examples/scale.rs:1"
            if name in ["Elements created by one command"]: status,note="deferred","C1.2 command schemas/executor; handoff gap 1."
            elif name=="Package size": status,note="deferred","u64 byte counts represented now; ZIP64 I/O C1.3, handoff gap 5."
            elif name=="Names": where="crates/model/src/name.rs:30; crates/model/src/validate.rs:103"
            elif name=="IDs": status,note="divergent","C11-R1-01 (duplicate split entries); inline/local checks exist."
        else: where="plan/IMPLEMENTATION_PLAN.md:164 (checkpoint ownership; not an implemented command)"
        add(f"10 §{section}, L{i}",f"{name}: {detail}",status,where,note)

# Requirements expressed as named prose rules across the WHOLE specification.
special={
 "ID-01":("divergent","crates/model/src/document/mod.rs:157","C11-R1-01; allocation/immutability also depend on C1.2."),
 "ID-02":("divergent","crates/model/src/id.rs:194","1,000,000 sequential IDs pass; cloning the system generator duplicates IDs (C11-R1-20). Opaque input IDs intentionally allow lowercase/non-ULID forms."),
 "ID-03":("implemented","crates/model/src/document/index.rs:1","Scoped nodes/strokes/keys and compound refs; motionPoint added per SI-006."),
 "ID-04":("deferred","plan/IMPLEMENTATION_PLAN.md:164","C1.2 undo/redo/moves, C1.3 save/load (handoff gaps 1,5)."),
 "ID-05":("deferred","plan/IMPLEMENTATION_PLAN.md:164","C1.2 edit commands and C1.3 import (gaps 1,5)."),
 "ID-06":("deferred","crates/model/src/id.rs:194","Generator/type representation present; rename/move/library copy policy belongs to commands C1.2/C9.1."),
 "ID-07":("implemented","crates/model/src/entities/palette.rs:1","Option<Id>, no reserved colour IDs."),
 "REV-01":("deferred","crates/model/src/document/mod.rs:437","documentRev and <= integrity present; commit stamping C1.2 gap 1."),
 "REV-02":("deferred","crates/model/src/change.rs:152","Own-view mechanism present; C11-R1-03/04; stamping C1.2 gap 1."),
 "REV-03":("deferred","plan/IMPLEMENTATION_PLAN.md:164","C1.2 undo/redo stamping, gap 1."),
 "REV-04":("deferred","crates/model/src/document/mod.rs:437","Fields and integrity present; persisted manifest/C1.3 and expectRev/C1.2."),
 "REV-05":("deferred","plan/IMPLEMENTATION_PLAN.md:164","C1.2 expectRev, gap 1."),
 "REV-06":("deferred","plan/IMPLEMENTATION_PLAN.md:164","C1.2 transaction queue, gap 1."),
 "JSON-01":("implemented","crates/model/src/canonical.rs:171","UTF-8, LF, two spaces, newline; golden/property controls pass."),
 "JSON-02":("divergent","crates/model/src/canonical.rs:1","C11-R1-06; structs ordered, free-form maps/scene.paletteStyles not canonical."),
 "JSON-03":("divergent","crates/model/src/canonical.rs:182","Finite vectors/tuples/-0 pass; generic serialization admits nonfinite -> null, C11-R1-16."),
 "JSON-04":("implemented","crates/model/src/schema.rs:1","x-omitDefault rules match serializer omissions."),
 "JSON-05":("divergent","crates/model/src/canonical.rs:171","Same constructed document stable; not all accepted objects serializable, C11-R1-09, or equal documents identical, C11-R1-06."),
 "AG-01":("deferred","crates/model/src/schema.rs:67","Shared types/schemas present; C1.2 registry, C1.3 doc commands, C1.4 MCP. C11-R1-08/11/12/13."),
 "AG-02":("deferred","crates/model/src/change.rs:1","C1.2 command/executor, gap 1; current inversion blockers C11-R1-03/04."),
 "AG-03":("divergent","crates/model/src/path.rs:26","C11-R1-10; special-ref context deferred C1.2 gap 4."),
 "AG-04":("deferred","crates/model/src/document/render.rs:1","Persisted readback present; derived values C1.2 gap 3; C11-R1-11 stub conformance."),
 "AG-05":("deferred","plan/IMPLEMENTATION_PLAN.md:164","doc.object.get paging is C1.3 doc command surface."),
 "AG-06":("divergent","crates/model/src/entities/scene.rs:54","C11-R1-06; IndexMap insertion order is not semantic document state."),
}
for i,line in enumerate(lines,1):
    m=re.match(r"- \*\*([A-Z]+-\d+)\b",line)
    if not m: continue
    name=m[1]
    paragraph=[line]
    j=i
    while j<len(lines) and lines[j] and not lines[j].startswith(("- **","##","|")):
        paragraph.append(lines[j]); j+=1
    status,where,note=special.get(name,("deferred","plan/IMPLEMENTATION_PLAN.md:164","C1.3 persistence/load/save/versions; C1.4 protocol where applicable; handoff gaps 1,5."))
    add(f"10 {name}, L{i}"," ".join(paragraph).removeprefix("- "),status,where,note)

# Prose-only field sets, boundaries, principles and decisions that a table parser cannot discover.
extra=[
 ("2.2 L140","Full objects and stubs both validate; correct type per child slot","divergent","crates/model/src/schema.rs:250","C11-R1-11"),
 ("2.4 L172","Quoted-name escapes, aliases, numeric/index and ID selectors; canonical paths","divergent","crates/model/src/path.rs:26","C11-R1-10; malformed/10k-segment controls pass; normal fixture paths pass."),
 ("2.6 L248","created/deleted/modified, parentId/index, oldValue, invertibility, array moves and range edits","divergent","crates/model/src/change.rs:489","C11-R1-03/04; generated ordinary edits/inverses pass seeded property tests."),
 ("2.6 L254","64 KiB elision and consumer reread","divergent","crates/model/src/change.rs:618","C11-R1-04: applying an elided replace writes null."),
 ("2.6 L256","origin user/agent/system/load; derived data absent","deferred","crates/model/src/change.rs:121","Own views omit derived fields; transaction origin/event enforcement C1.2."),
 ("2.7 L271","DM-1 same serializer for persistence and query","implemented","crates/model/src/document/render.rs:139","Golden split-depth comparison passes."),
 ("2.8 L281","Property x-since and persisted immutable/readOnly annotations","missing","crates/model/src/schema.rs:1","C11-R1-13: object-level x-since is not property-level x-since; type lacks readOnly."),
 ("2.8 L284","Draft 2020-12, format-versioned schema $id and generated publication","implemented","crates/model/src/schema.rs:40","1.0 bundle and generated TS modules; generation checked twice."),
 ("10.2 L1131","Property-schema lookup through child unions and numeric tuple indices","divergent","crates/model/src/schema.rs:581","C11-R1-19; implementation already published in model API; command registration itself remains C1.3."),
 ("3 L293","Containment tree, references, repeated nodes, orphan elements and parameter ownership","divergent","crates/model/src/document/mod.rs:437","Cycle/orphan/dangling controls pass; duplicate split entry disappears C11-R1-01; image invariant C11-R1-05."),
 ("4.4 L381","transitionType (SI-004), durationLeft=12, blankDuration=0, durationRight=12 (0..1000), colour=#000000FF, angle=0 (0..360), direction type-dependent, size=25 (5..100 step 5); preserve inactive fields","implemented",E+"transition.rs:1","Envelope collision resolved explicitly; unreadable-load fallback C1.3."),
 ("4.7 L466","exposure [drawingId|null,count>=1], merge equal neighbours, omit trailing blanks, bank membership","implemented","crates/model/src/object.rs:83; crates/model/src/document/mod.rs:781","Seeded exposure properties and fixtures pass."),
 ("4.7 L470","cellNotes decimal frame keys and <=10,000-character values; cell {frame,drawingId,drawingName,note}","implemented",E+"element.rs:1; crates/model/src/document/render.rs:1","Persisted map/cell view present; NFC global defect C11-R1-15 applies."),
 ("4.8 L477","drawingBank.drawings unordered; natural display order; derived elementIds; delete with last user","deferred",E+"drawing.rs:1","Representation/order present; transactional deletion C1.2/M3; derived gap 3."),
 ("4.10 L527","Each image drawing contains exactly one bitmap","divergent","crates/model/src/document/mod.rs:781","C11-R1-05"),
 ("4.11 L532","colourRecovery {colourId,paletteId,paletteName,styles[{styleId,styleName}],swatch}; exactly referenced colours at save; recovery load/paste/import","deferred",E+"drawing.rs:1","Shape present. Save/load C1.3 gap 5; colour recovery geometry/commands C2.2 per plan 4."),
 ("4.12 L543","channel map keys/owners, no position.x/y/z, channel exists only with keys","implemented","crates/model/src/validate.rs:475","Owner-specific allowed channels and empty/duplicate frame checks present."),
 ("4.12 L555","key id/frame/value/constantSegment/handles {left/right dt,dv,mode}/easing; handle clamping","divergent",E+"channel.rs:1; crates/model/src/validate.rs:449","C11-R1-05 and C11-R1-14"),
 ("4.12 L565","motionPoint id, frame|null, position, tension/continuity/bias [-1,1], constantSegment, lockDepth","implemented",E+"channel.rs:1","SI-006 adds addressable envelope; first/last keyframe, order and uniqueness checked. Generic int bound finding applies."),
 ("4.13 L579","swatch kind; values {rgba}, {stops[{position,rgba}]}, {image,tile}; one value/style","implemented",E+"palette.rs:1; crates/model/src/validate.rs:149","Style coverage and gradient checks present; textures separately divergent."),
 ("4.13 L581","Texture image paths under textures/","divergent","crates/model/src/validate.rs:149","C11-R1-05"),
 ("4.13 L582 / 03 §1.2","At least one visible palette, not merely one palette","divergent","crates/model/src/document/mod.rs:437","C11-R1-05: all-hidden palettes accepted."),
 ("4.14 L589","streamed=false, showMouthShapes=false, non-overlapping placements; mappings drawingElementId/A-G-X/autoReapply unique per target","implemented",E+"sound.rs:1; crates/model/src/validate.rs:704","Shape/defaults and placement overlap/cross-reference checks present."),
 ("4.14 L595","stopFrame <= natural end; in/out/envelope <= source duration","deferred","crates/model/src/validate.rs:611","Ordering present. Source media info is derived (handoff gap 3); decoding/source length owner M8. Recheck at C1.2/C1.3 API boundary."),
 ("4.15 L622","mediaSequence id/assetId/startFrame/stopFrame/linkOffset>=1, non-overlap, linked source, derived sourceLength/missing","implemented",E+"sound.rs:1; crates/model/src/document/mod.rs:781","Persisted structure/reference checks present; sourceLength/missing computation gap 3."),
 ("4 L348/376/492/617/641","All listed derived/transient project, scene, element, drawing, sound, asset and library properties","deferred","crates/model/src/schema.rs:1","Declarations present; computation handoff gap 3 C1.2; transient session state C1.2/C1.4."),
 ("1/5","Input depth must preserve serializability and boundary guarantees","divergent","crates/model/src/object.rs:37","C11-R1-09: accepts depth 140 while canonical writer is capped at 128; JS boundary 64."),
]
for section,req,status,where,note in extra: add("10 §"+section,req,status,where,note)
for i,line in enumerate(lines,1):
    if not re.match(r"\| D\d+ \|",line): continue
    cells=[c.strip() for c in re.split(r"(?<!\\)\|",line)[1:-1]]
    n=int(cells[0][1:]); status="implemented"; where="docs/decisions/ADR-0010-object-model-and-identifiers.md:1"; note="Model choice matches recommendation."
    if n in [7,10,11,12,13,14,16]: status="deferred"; where="plan/IMPLEMENTATION_PLAN.md:164"; note="C1.2 history (D11), C1.3 persistence/default import policy (handoff gaps 1,5); library/media commands later owning milestones."
    if n in [2,8,9]: where="crates/model/src/entities/drawing.rs:1"; note="Sides/JSON arrays/f64 representation present; topology fill consistency is C2.2 and load repair gap 5."
    if n==15: where="crates/model/src/document/mod.rs:118"; note="Arc per entry, shallow snapshots; whole-entry clone/diff cost remains C1.2 gap 2."
    add(f"10 {cells[0]}, L{i}"," | ".join(cells[1:]),status,where,note)

counts=Counter(r["status"] for r in rows)
(HERE/"conformance.json").write_text(json.dumps({"counts":dict(counts),"rows":rows},ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
def esc(s): return str(s).replace("|","\\|").replace("\n"," ")
md=["# C1.1 round 1 conformance register", "", "Counts are requirement rows, not a percentage of product completion: "+json.dumps(dict(counts))+".", "",
    "Every table row in spec 10 is retained, combined named fields are split, named normative rules and D1–D16 are enumerated, and prose-only model field sets are added. The second inventory lists every generated property, including nested value-object fields. Requirements expressed by a block row retain their defaults/constraints verbatim in that row; its nested fields are individually discoverable in the inventory.", "",
    "`implemented` means the model representation/default and the cited checks were found and no local divergence was identified. Cross-cutting findings (NFC, integer bounds, metadata, validated construction, canonicalization) apply to otherwise implemented fields. Deferred command, renderer, media and persistence behavior is not counted as implemented. A present field alone does not prove semantic validation. Read the finding and deferral notes with the status.", "",
    "The requirements are the reviewed spec; generated properties are implementation evidence only. Map ordering uses Unicode scalar lexicographic order in Rust BTreeMap/overlay (UTF-8 byte order agrees for valid scalar strings); JS UTF-16 sort would differ for supplementary characters. No cross-language sorting assumption is needed to reproduce C11-R1-06.", "",
    "| Spec | Requirement (type / default / constraint) | Status | Implementation / owner | Review note |", "|---|---|---|---|---|"]
for r in rows: md.append("| "+" | ".join(esc(r[k]) for k in ["citation","requirement","status","implementation","note"])+" |")
md += ["", "## Complete generated property inventory", "", "This includes all 22 object envelopes and every value object. Schema locations give exact definition-level file:line references; JSON pointers identify individual fields. The spec column links each nested field to its owning entity/block requirement, whose defaults and constraints are retained above. The requirement register owns verdicts, so these structural rows are not double-counted.", "", "| Definition.property | Spec requirement | Schema definition location | Declared type / bounds / default / markers |", "|---|---|---|---|"]
definition_sections={}
source_sections={"project":"4.1","pen":"4.2","scene":"4.3","transition":"4.4","element":"4.5–4.7","drawing":"4.8–4.11","channel":"4.12","palette":"4.13","sound":"4.14–4.15","asset":"4.16","library":"4.17","effect":"4.6 (06 owns effect parameters)","mod":"2.2"}
for source in (ROOT/E).glob("*.rs"):
    for name in re.findall(r"\b(?:struct|enum|type)\s+(\w+)",source.read_text(encoding="utf-8")):
        definition_sections[name]="10 §"+source_sections[source.stem]
for name in re.findall(r"\b(?:struct|enum|type)\s+(\w+)",(ROOT/"crates/model/src/value.rs").read_text(encoding="utf-8")):
    definition_sections[name]="10 §1.1"
inventory_count=0
for name,s in defs.items():
    line=next((i for i,l in enumerate(schema_lines,1) if l==f'    "{name}": {{'),1)
    for field,p in s.get("properties",{}).items():
        annotations={k:v for k,v in p.items() if k not in ["description","title"]}
        annotations["required"]=field in s.get("required",[])
        requirement=definition_sections.get(name,"10 §2.2/2.8 (generated child/stub helper)")
        if name in [ref.rsplit('/',1)[-1] for ref in bundle['x-objectTypes'].values()] and field in ['id','colourId','type','rev','name','meta','ext']:
            requirement="10 §2.2; "+requirement
        if p.get('x-derived') or p.get('x-transient'): requirement+="; 10 §2.7 (query-only)"
        md.append(f"| {name}.{field} | {requirement} | crates/model/schema/model-1.0.schema.json:{line}; /$defs/{name}/properties/{field} | {esc(json.dumps(annotations,ensure_ascii=False,separators=(',',':')))} |")
        inventory_count+=1
md += ["", f"Inventory: {len(bundle['x-objectTypes'])} object types; {inventory_count} properties across object/value schemas.", ""]
(HERE/"conformance.md").write_text("\n".join(md),encoding="utf-8")
print(json.dumps({"requirements":len(rows),"counts":dict(counts),"inventoryProperties":inventory_count}))
