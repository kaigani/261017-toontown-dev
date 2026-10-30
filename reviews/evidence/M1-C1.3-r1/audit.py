"""Recompute review inventories, source locations, immutable-path checks, log hashes and ZIP byte checks."""
import collections
import hashlib
import json
import pathlib
import re
import subprocess
import zipfile

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
def read(p): return (ROOT/p).read_text(encoding="utf-8-sig")
def write(n,v): (OUT/n).write_text(json.dumps(v,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
def git(*args): return subprocess.run(["git",*args],cwd=ROOT,capture_output=True,check=True).stdout.decode("utf-8",errors="replace")
def loc(path,needle):
    s=read(path); at=s.find(needle)
    assert at>=0,(path,needle)
    return f"{path}:{s[:at].count(chr(10))+1}"

files={p.as_posix():p.read_text(encoding="utf-8") for p in (ROOT/"crates/executor/src/commands").glob("*.rs")}
def handler(command):
    for path,text in files.items():
        marker=f'id = "{command}"'
        if marker in text:
            return f"{pathlib.Path(path).relative_to(ROOT).as_posix()}:{text[:text.index(marker)].count(chr(10))+1}"
    raise AssertionError(command)

# Immutable files: new reviewer suites are distinct from all inherited paths.
protected=["crates/model/tests/c11_*","crates/engine/tests/c11_*","desktop/src/ipc/c11-*","tests/adversarial","desktop/src/test","crates/model/tests/staged_commit.rs","crates/model/tests/staged_inverse.rs","crates/executor/tests/c12_r[0-9]*","crates/executor/tests/c12-r1-ui","reviews/evidence/M1-C1.2*"]
delta=git("diff","67265b8","--",*protected)
(OUT/"inherited-reviewer.diff").write_text(delta,encoding="utf-8")
write("immutable-paths.json",{"base":"67265b8","paths":protected,"diffEmpty":not delta,"specDiffEmpty":not git("diff","67265b8","--","spec"),"builderTestEdits":git("diff","67265b8","--","crates/executor/tests/c12_policy.rs","crates/executor/tests/c12_registry.rs","crates/executor/tests/c12_kernel_state.rs")})
receipt=json.loads(read("reviews/evidence/M1-C1.3/receipt.json"))
hashes={p:{"expected":h,"actual":hashlib.sha256((ROOT/"reviews/evidence/M1-C1.3"/p).read_bytes()).hexdigest()} for p,h in receipt["sha256"].items()}
write("builder-receipt-audit.json",{"base":receipt["baseCommit"],"files":hashes,"allMatch":all(v["expected"]==v["actual"] for v in hashes.values())})

changed=git("diff","--name-only","67265b8","f119948","--",":(glob)crates/*/src/**/*.rs").splitlines()
static=[]
for path in changed:
    if not path.endswith(".rs"): continue
    text=read(path).split("#[cfg(test)]")[0]
    code=re.sub(r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/',lambda m:re.sub(r"[^\n]"," ",m[0]),text,flags=re.S)
    static.append({"path":path,"prohibited":[{"line":code[:m.start()].count("\n")+1,"match":m[0]} for m in re.finditer(r"\bunsafe\b|\.unwrap\s*\(|\.expect\s*\(|\b(?:panic|unreachable|todo|unimplemented)!",code)],"allowAttributes":[{"line":text[:m.start()].count("\n")+1,"attribute":m[0]} for m in re.finditer(r"#\[allow\([^\]]+\)\]",text,re.S)]})
write("static-audit.json",{"method":"41 changed src files before inline cfg(test) modules; ordinary string literals/comments removed with line numbers retained; allowances retained for manual review; compiler/clippy gates independently passed","files":static,"dependencyDiff":git("diff","67265b8","f119948","--","Cargo.lock","Cargo.toml","crates/persist/Cargo.toml","deny.toml")})

sample=OUT/"format-samples"
if (sample/"byte-check.animpkg").exists():
    z=zipfile.ZipFile(sample/"byte-check.animpkg")
    infos=z.infolist(); mime=infos[0]; manifest=json.loads(z.read("manifest.json"))
    listed={e["path"]:e for e in manifest["entries"]}
    entries={i.filename:z.read(i) for i in infos}
    expected=set(entries)-{"mimetype","manifest.json","session.json"}
    expected={n for n in expected if not n.startswith("cache/")}
    checks={"mimetypeFirst":mime.filename=="mimetype" and mime.header_offset==0,"mimetypeStored":mime.compress_type==0,"mimetypeNoExtra":not mime.extra,"mimetypeExact":entries["mimetype"]==b"application/x-animpkg+zip","allUTF8":all(i.flag_bits&0x800 for i in infos),"noneEncrypted":all(not i.flag_bits&1 for i in infos),"manifestComplete":set(listed)==expected,"hashesMatch":all(e["size"]==len(entries[n]) and e["sha256"]==hashlib.sha256(entries[n]).hexdigest() for n,e in listed.items()),"jsonDeflated":all(i.compress_type==8 for i in infos if i.filename.endswith(".json")),"jsonUTF8NoBOMAndLF":all(b.decode("utf-8") and not b.startswith(b"\xef\xbb\xbf") and b.endswith(b"\n") and b"\r" not in b for n,b in entries.items() if n.endswith(".json"))}
    folder=sample/"byte-check-folder"
    folder_entries={p.relative_to(folder).as_posix():p.read_bytes() for p in folder.rglob("*") if p.is_file() and not p.relative_to(folder).parts[0].startswith(".")}
    diff=[n for n in sorted(set(entries)|set(folder_entries)) if n!="manifest.json" and entries.get(n)!=folder_entries.get(n)]
    fm=json.loads(folder_entries["manifest.json"]); pm=manifest.copy(); pm.pop("entries")
    write("format-byte-audit.json",{"checks":checks,"entryCount":len(entries),"folderEntryCount":len(folder_entries),"nonManifestDifferences":diff,"manifestOtherwiseIdentical":pm==fm,"folderControlFiles":[p.name for p in folder.iterdir() if p.name.startswith(".")],"packageSha256":hashlib.sha256((sample/"byte-check.animpkg").read_bytes()).hexdigest()})

# Each named §6–§8 rule is one row; additional rows cover unnamed tables/paragraphs and shell requirements.
spec=read("spec/10_data_model_project_format.md")
start=spec.index("## 6. Project format"); end=spec.index("## 9.",start)
section=spec[start:end]
family={
 "FMT":("crates/persist/src/save.rs","pub fn save(","crates/persist/tests/round_trip.rs"),
 "ZIP":("crates/persist/src/zip.rs","pub fn open(","crates/persist/tests/hostile.rs; crates/persist/tests/zip64.rs"),
 "LAY":("crates/persist/src/layout.rs","pub fn entry_path(","crates/persist/tests/round_trip.rs"),
 "JSON":("crates/model/src/canonical.rs","pub fn to_canonical_string", "crates/model/tests/conformance.rs; crates/persist/tests/round_trip.rs"),
 "VER":("crates/persist/src/manifest.rs","pub fn check(","crates/persist/tests/hostile.rs; crates/persist/tests/migrations.rs; crates/executor/tests/c13_projects.rs"),
 "MED":("crates/persist/src/load.rs","pub fn blob_refs(","crates/executor/tests/c13_doc.rs; crates/persist/tests/round_trip.rs"),
 "LOAD":("crates/persist/src/load.rs","pub fn load(","crates/persist/tests/hostile.rs; crates/executor/tests/c13_projects.rs"),
 "REP":("crates/persist/src/repair.rs","impl Work", "crates/persist/tests/hostile.rs; crates/executor/tests/c13_doc.rs"),
 "SAVE":("crates/persist/src/save.rs","pub fn save_package(","crates/persist/tests/hostile.rs; crates/executor/tests/c13_projects.rs"),
 "BAK":("crates/persist/src/backup.rs","impl BackupSink", "crates/executor/tests/c13_projects.rs"),
 "AUTO":("crates/executor/src/maintenance.rs","fn autosave(","crates/executor/tests/c13_projects.rs"),
 "REC":("crates/executor/src/commands/project.rs","fn project_recovery_open(","crates/executor/tests/c13_projects.rs"),
 "RO":("crates/executor/src/maintenance.rs","fn watch(","crates/executor/tests/c13_projects.rs"),
 "LOCK":("crates/persist/src/lock.rs","pub fn acquire(","crates/persist/src/lock.rs::exclusive_live_and_stale; crates/executor/tests/c13_projects.rs")}
divergent={"FMT-01":"C13-R1-12","ZIP-01":"C13-R1-10","VER-02":"C13-R1-05","MED-01":"C13-R1-16","MED-04":"C13-R1-18","SAVE-01":"C13-R1-07; Windows write-through ADR-0014 §3","SAVE-02":"C13-R1-03,04","REP-01":"SI-044; C13-R1-09","REC-01":"C13-R1-08","RO-02":"C13-R1-06; SI-045/C1.4","LOCK-02":"C13-R1-02"}
deferred={"LOCK-03":"M9 C9.1 library locks (ADR-0014 §6)","MED-06":"M2/M3/M8 cache generation (ADR-0014 §17); caches optional now","VER-05":"M7 C7.1/M9 C9.1 complete specialty version/migration semantics; basic readers/writers present"}
rows=[]
for match in re.finditer(r"^- \*\*([A-Z]+-\d+)\*\*([^\n]*)",section,re.M):
    ident=match[1]; fam=ident.split("-")[0]; source,needle,test=family[fam]
    rows.append({"id":ident,"requirement":match[2].strip(),"spec":f"spec/10_data_model_project_format.md:{spec[:start+match.start()].count(chr(10))+1}","status":"divergent" if ident in divergent else "deferred-with-owner" if ident in deferred else "implemented","implementation":loc(source,needle),"tests":test,"note":divergent.get(ident,deferred.get(ident,""))})

def add(ident,requirement,status,source,needle,test,note="",citation="10 §6–§8"):
    rows.append({"id":ident,"requirement":requirement,"spec":citation,"status":status,"implementation":loc(source,needle),"tests":test,"note":note})
for ident,req,needle,test in [
 ("Manifest","format, versions/features, generator, project ID, documentRev, savedAt, SHA256/size list; folder list omitted","pub fn manifest_for(","format-byte-audit.json; hostile::hash_mismatches_are_warnings_and_the_entry_is_still_read"),
 ("Interchange","self-contained full-depth bundle, blobs, remapping and conflict policies","pub fn plan(","c13_doc::bundles_import_into_a_scene_at_a_frame_with_fresh_ids_and_renames"),
 ("Thumbnail","preserve an existing thumbnail until M3 regeneration","pub fn plan(","c13_r1_format::existing_project_thumbnail_survives_save"),
 ("Session","optional UI state, invalid ignored, excluded from manifest/undo","pub fn plan(","c13_projects::create_save_close_open_round_trips_both_forms")]:
    add(ident,req,"divergent" if ident in ("Interchange","Thumbnail") else "implemented","crates/persist/src/save.rs",needle,test,{"Interchange":"C13-R1-11","Thumbnail":"C13-R1-13"}.get(ident,""))
for ident,req in [("Templates","mimetype, manifest, metadata/content and binaries"),("Palettes","optional mimetype archive or bare JSON, textures"),("LibraryLayout","shared animlib index layout, flat files, broken items")]:
    add(ident,req,"implemented","crates/persist/src/library.rs","pub fn to_animlib(","c13_doc::packages_templates_palettes_and_libraries_are_inspected_without_opening",citation="10 §6.9–§6.11")
repair_table=section[section.index("| Problem | Severity |"):section.index("- **REP-01**")]
for line in repair_table.splitlines()[2:]:
    if not line.startswith("|"):continue
    cells=re.split(r"(?<!\\)\|",line)[1:-1]
    if len(cells)<4:continue
    req=cells[0].strip(); code=cells[-1].strip()
    status="implemented"; note=""
    if "colour ID" in req: status="deferred-with-owner"; note="M2 C2.2 colour recovery (ADR-0014 §17), currently colour.unresolved"
    if "zone fills" in req:status="deferred-with-owner";note="M2 C2.2 planar map (ADR-0014 §17)"
    if "font" in req:status="deferred-with-owner";note="M2 C2.1 font discovery (ADR-0014 §16)"
    if "Unknown properties" in req:status="divergent";note="C13-R1-05"
    add(f"Repair-{len([r for r in rows if r['id'].startswith('Repair-')])+1:02}",f"{req}; {code}",status,"crates/persist/src/repair.rs","impl Work","crates/persist/tests/hostile.rs",note,citation="10 §7.2 repair table")
for i,req in enumerate(["Container identification and safe names","Manifest, versions/features, integrity warnings","Pure ordered migration chain","Parse/schema validate every entry","Resolve references, scene tree and assets","Repair, then colour recovery","Derive zones/scene lengths; schedule caches/thumbnails","Publish report, project.opened and one load-origin change"],1):
    status="deferred-with-owner" if i in (6,7) else "implemented"
    note={6:"M2 C2.2 colour recovery; deterministic basic repair implemented",7:"M2/M3/M8 derived/cache computations; eager model scene lengths available"}.get(i,"")
    add(f"LoadPhase-{i}",req,status,"crates/persist/src/load.rs","pub fn load(","crates/persist/tests/hostile.rs; crates/executor/tests/c13_projects.rs",note,citation="10 §7.1")
add("LoadReport","complete report and deterministic issues/IDs (durationMs is measured)","implemented","crates/persist/src/load.rs","let report = LoadReport", "c13_doc::a_repaired_load_is_one_non_undoable_step_and_reported",citation="10 §7.3")
add("ExecutorEffects","commit-only persistence effects; dryRun/atomic failure do not write files or move clean marker","divergent","crates/executor/src/persistence.rs","pub(crate) fn save_now(","c13_r1_persistence dry-run and failed atomic batch regressions","C13-R1-01",citation="11 §7.2–§7.3; C1.2 closure tickets")
add("InstallGate","open, migration, recovery, backup and repair installation validated before live use","implemented","crates/executor/src/run.rs","fn gate(","compile_fail; c12_policy; c13_projects::opening_bad_files_fails_the_job_and_leaves_no_project_open",citation="ADR-0013 §16; C1.2 closure tickets")
add("ReopenedHistory","persisted revisions continue through 10k-step history and exact replay","implemented","crates/executor/src/run.rs","fn commit(","oracle-r1.log; oracle-r3.log; reopened-oracles.json",citation="REV-01..06; C1.2 closure tickets")
add("RoundTrip","all command-created supported object types, canonical JSON, revisions, opaque data and binary entries","divergent","crates/executor/src/commands/bundle.rs","fn import(","c13_doc; c13_r1_format positive fixture round trip; both reopened oracles","Ordinary 1.0 passes; unknown-minor composition loses data (C13-R1-05). Library child creation remains M9.",citation="11 §14.3")
add("Examples","each declaration has schemas/examples; every C1.3 example executes on real files","implemented","crates/executor/tests/c12_kernel_state.rs","fn every_example_runs_on_the_fixture_under_the_watchdog", "c12_kernel_state; c13_r1_persistence registry schema audit; persistence-registry.json",citation="11 §14.2")
add("InspectionBound","large packages can be inspected without allocating the entire archive twice","divergent","crates/executor/src/commands/doc_io.rs","fn inspect(","c13_r1_persistence::package_inspection_reads_a_bounded_header; inspection-memory.json","C13-R1-14",citation="10 §5 ZIP64; 11 §14.6; spec A doc.package.get")
add("UndoLabels","Import JSON / Repair Project labels match the command table","divergent","crates/executor/src/commands/doc_lint.rs",'undo = "Repair"',"c13_r1_persistence::import_and_repair_undo_labels_match_the_command_table","C13-R1-17",citation="10 §10.2; plan §3.2")
for ident,req,source,needle,test,status,note in [
 ("NewDefaults","one scene, camera/peg, empty drawing element, 120-swatch palette, 8 pens; clean empty history","crates/model/src/defaults.rs","pub fn new_project(","tt-model defaults tests; c13_projects","implemented",""),
 ("Presets","all required bundled presets, mostRecent/default precedence, explicit overrides","crates/executor/src/commands/project.rs","fn presets(","c13_projects::workspaces_recent_projects_and_defaults_persist_per_user","implemented","UI custom selection belongs to M4 C4.1"),
 ("Recent","normalized deduplication, maxCount, exists status, persisted ordering and removal","crates/executor/src/store.rs","pub(crate) fn add_recent(","c13_projects::workspaces_recent_projects_and_defaults_persist_per_user","implemented",""),
 ("SettingsIO","per-user conventional directories, atomic versioned human-readable files; corrupt copy/default/warning","crates/persist/src/settings.rs","pub fn read(","settings::missing_data_and_corrupt; c13_projects","implemented","Supported 1.0 settings; no historic product migration exists"),
 ("Preferences","project default and persistence preference values stored separately","crates/executor/src/store.rs","pub(crate) fn pref_default(","c13_projects::workspaces_recent_projects_and_defaults_persist_per_user","implemented","Full preferences command/UI owner M4 C4.1"),
 ("Workspaces","user/modified builtin/fullscreen workspace layouts persist and restore","crates/executor/src/maintenance.rs","pub(crate) fn start(","c13_projects::workspaces_recent_projects_and_defaults_persist_per_user","implemented",""),
 ("RemainingSettings","keymap, toolbar, window geometry, last dialog folders, suppressed messages, first-run flags","crates/persist/src/settings.rs","pub fn write(","C1.3 generic store infrastructure only","deferred-with-owner","M4 C4.1 shell/settings; M9 C9.3 bundled first-run content; kernel host wiring C1.4"),
 ("Quit","unsaved policy, no location pathRequired, clean recovery deletion","crates/executor/src/commands/app.rs","fn app_quit(","c13_projects::unsaved_changes_policies_and_quit; c13_r1_persistence dry-run/atomic regression","divergent","C13-R1-01; native quit is M4/C1.4")]:
    if ident=="Preferences":status="divergent";note="C13-R1-15: defaults are persisted as overrides"
    add(ident,req,status,source,needle,test,note,citation="01 §3, §11, §12 / 11 §14.3")

# Lint rows preserve all 19 checks and explicit unavailable owners.
lint=read("crates/executor/src/commands/doc_lint.rs")
owners=dict(re.findall(r'"(\w+)" => \("(M\d+ C[\d.]+)"',lint))
checks=re.search(r'const CHECKS:.*?= \[(.*?)\];',lint,re.S)[1]
for check in re.findall(r'"(\w+)"',checks):
    add("Lint-"+check,check,"deferred-with-owner" if check in owners else "implemented","crates/executor/src/commands/doc_lint.rs","pub(crate) const CHECKS", "c13_doc::validation_references_and_candidates",owners.get(check,""),citation="10 §10.3")

commands=[]
runtime={r["id"]:r for r in json.loads((OUT/"persistence-registry.json").read_text(encoding="utf-8"))} if (OUT/"persistence-registry.json").exists() else {}
bad={"app.quit":"C13-R1-01","project.create":"C13-R1-01","project.close":"C13-R1-01","project.backup.open":"C13-R1-01","project.recovery.open":"C13-R1-08","project.save":"C13-R1-06","project.saveAs":"C13-R1-02","project.properties.set":"C13-R1-15","doc.package.export":"C13-R1-07","doc.import.json":"C13-R1-11,17","doc.package.get":"C13-R1-14","doc.asset.embed":"C13-R1-16","doc.repair":"C13-R1-17"}
for i,line in enumerate(read("spec/A_command_registry.md").splitlines(),1):
    m=re.match(r"\| `(project\.[^`]+|doc\.[^`]+|app.quit)` \|",line)
    if not m: continue
    ident=m[1]; r=runtime.get(ident,{})
    cells=[s.strip() for s in re.split(r"(?<!\\)\|",line)[1:-1]]
    commands.append({"id":ident,"kind":cells[1],"spec":f"spec/A_command_registry.md:{i}","verbatimSpecRow":line,"implementation":handler(ident),"status":"divergent" if ident in bad else "implemented","findings":bad.get(ident,""),"params":cells[3],"result":cells[4],"undo":cells[5],"actualScopes":r.get("scopes"),"actualEnablement":r.get("enablement"),"actualUndoLabel":r.get("undoLabel"),"examples":r.get("examples"),"tests":"c12_kernel_state::every_example_runs_on_the_fixture_under_the_watchdog; c13_projects/c13_doc; runtime schemas in persistence-registry.json"})
assert len(commands)==48 and len({c["id"] for c in commands})==48
counts={"requirements":dict(collections.Counter(r["status"] for r in rows)),"commands":dict(collections.Counter(r["status"] for r in commands)),"requirementTotal":len(rows),"commandTotal":len(commands)}
write("requirements.json",rows); write("command-register.json",commands); write("register-counts.json",counts)
md=["# C1.3 round-1 conformance register","", "Recomputed by `audit.py`. Status reflects the full rule, including listed divergences. An implemented row has source review and the named executable evidence; it is not a claim of exhaustive input-space testing. Deferred rows name the milestone. Runtime command schemas, examples, enablement, scopes and undo labels are preserved in `persistence-registry.json`; verbatim specification columns are in `command-register.json`.","",f"Counts: {json.dumps(counts)}","","## Requirements","","| Requirement | Status | Implementation | Tests / ruling |","|---|---|---|---|"]
for r in rows: md.append(f"| {r['id']} — {r['requirement'].replace('|','/')} ({r['spec']}) | {r['status']} | `{r['implementation']}` | {r['tests']}; {r['note']} |")
md += ["","## Commands and events","","Every row was reconciled with spec A. Parameters/result/undo text is stored verbatim in the JSON companion. For events, declaration examples validate; invoking an event correctly returns command.notInvocable rather than executing a handler. Actual enablement/scopes and example JSON are in the runtime dump.","","| ID / kind | Status | Implementation | Enablement; undo | Evidence / findings |","|---|---|---|---|---|"]
for c in commands: md.append(f"| `{c['id']}` / {c['kind']} | {c['status']} | `{c['implementation']}` | {c['actualEnablement']}; {c['actualUndoLabel']} | {c['tests']}; {c['findings']} |")
(OUT/"conformance.md").write_text("\n".join(md)+"\n",encoding="utf-8",newline="\n")
print(json.dumps(counts))
