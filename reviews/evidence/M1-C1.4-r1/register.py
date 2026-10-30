"""Build a line-addressed register; status decisions below are the reviewer's explicit adjudication."""
import collections
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
S11 = ROOT / "spec/11_agent_interface_mcp.md"
rows = []


def loc(path, needle):
    for i, line in enumerate((ROOT / path).read_text(encoding="utf-8").splitlines(), 1):
        if needle in line:
            return f"{path}:{i}"
    raise ValueError((path, needle))


source = {
    "registry": loc("crates/executor/src/lib.rs", "pub fn registry"),
    "executor": loc("crates/executor/src/run.rs", "pub(crate) fn invoke"),
    "tools": loc("crates/mcp/src/tools.rs", "pub fn plan"),
    "protocol": loc("crates/mcp/src/server.rs", "fn request("),
    "init": loc("crates/mcp/src/server.rs", "fn initialize("),
    "roots": loc("crates/executor/src/persistence.rs", "pub(crate) fn check_roots"),
    "policy": loc("crates/executor/src/agents.rs", "pub fn scopes("),
    "confirm": loc("crates/executor/src/cx.rs", "pub fn confirm("),
    "events": loc("crates/mcp/src/resources.rs", "pub fn affected("),
    "resources": loc("crates/mcp/src/resources.rs", "pub fn resolve("),
    "docs": loc("crates/mcp/src/docs.rs", "pub fn topic("),
    "prompts": loc("crates/mcp/src/prompts.rs", "pub const PROMPTS"),
    "headless": loc("bins/headless/src/cli.rs", "pub fn run("),
    "audit": loc("crates/executor/src/lib.rs", "pub fn invoke_audited("),
}
tests = {
    "protocol": "crates/mcp/tests/protocol.rs",
    "schema": "crates/executor/tests/c12_kernel_state.rs; crates/executor/tests/c12_r1_stress.rs",
    "kernel": "crates/executor/tests/c12_r1_stress.rs; crates/executor/tests/c13_r1_persistence.rs",
    "headless": "bins/headless/tests/mcp.rs; bins/headless/tests/cli.rs; scripts/mcp-conformance.py",
    "new": "tests/adversarial/c14-r1-mcp.py (cargo: bins/headless/tests/c14_r1.rs)",
    "audit": "crates/mcp/tests/audit.rs",
}


def decision(section, text):
    state, src, test, note = "implemented", "executor", "kernel", "Within the implemented M1 kernel; domain operations keep their declared later owners."
    if section == "1":
        src, test = "registry", "schema"
        if "P1 |" in text or "P3 |" in text or "P6 |" in text or "P7 |" in text:
            state, note = "deferred-with-owner", "Builder: GUI parity/gestures M4, render and scene determinism M3; kernel determinism already tested."
        if "P5 |" in text: state, src, test, note = "divergent", "protocol", "new", "C14-R1-05/14: server-owned filter has no watchdog; client waits survive EOF."
        if "P8 |" in text: state, src, test, note = "divergent", "policy", "new", "C14-R1-02/06/12: confirmation validation, persisted grant defaults and attribution need correction."
    elif section == "2":
        src, test = "registry", "schema"
        if "GUI binding" in text or "Renderer." in text:
            state, note = "deferred-with-owner", "Builder: M4 GUI binding and M3 rendering (plan §4)."
        elif "Executor." in text:
            state, src, test, note = "divergent", "executor", "new", "C14-R1-03: document queue is shared, but activity export publishes before atomic commit."
    elif section == "3":
        src, test = "headless", "headless"
        if "Attached" in text or "Several MCP" in text:
            state, note = "deferred-with-owner", "Builder M4: ADR-0015 §10, attached HTTP and independent sessions."
        elif "**Headless**" in text:
            state, note = "deferred-with-owner", "Headless kernel works now; builder M3 renderer and M4 virtual UI/input remain."
        elif "Batch script" in text:
            state, test, note = "divergent", "new", "C14-R1-03/11: transactional export effects and configurable session limits are not preserved."
    elif section == "4":
        src, test = "protocol", "protocol"
        if "Protocol." in text:
            state, test, note = "divergent", "new", "Negotiates 2025-06-18/2025-11-25; SI-049 is genuine. C14-R1-01/02/13/14/17 cover malformed-input and lifecycle gaps."
        elif any(x in text for x in ["Streamable HTTP", "HTTP endpoint", "authorization flow"]):
            state, note = "deferred-with-owner", "Builder M4, ADR-0015 §10: localhost, Origin, bearer auth and discovery are unimplemented."
        elif "Server info." in text:
            src, note = "init", "Metadata under _meta.toontown; instructions exactly match app://docs/agent-guide."
        elif "Off by default" in text:
            src, note = "headless", "Explicit --mcp-stdio flag; no desktop MCP listener in this checkpoint."
    elif section.startswith("5"):
        src, test = "executor", "kernel"
        if section == "5.3" or (section == "5.2" and ("Angles" in text or "Time |" in text)):
            state, note = "deferred-with-owner", "Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now."
    elif section == "6.1":
        src, test = "policy", "protocol"
        if "Policy presets" in text or "preset" in text.lower():
            state, test, note = "divergent", "new", "Presets/scopes work in memory; C14-R1-06: persisted default Observe becomes Assist on restart."
    elif section == "6.2":
        src, test = "roots", "protocol"
        if "Media can" in text:
            state, note = "deferred-with-owner", "Builder M2/M8/M10 media ingestion; existing doc.import.json respects roots and scopes."
        if "Writes never" in text:
            state, test, note = "divergent", "new", "Existing handlers ask on overwrite, but malformed elicitation accepts and atomic activity export writes on rollback (02/03)."
    elif section == "6.3":
        src, test = "confirm", "protocol"
        if "client supports" in text:
            state, test, note = "divergent", "new", "C14-R1-02/04/12/14: invalid accept, requestId retry, audit phases and EOF lifetime."
        elif "Otherwise" in text:
            note = "Fallback tokens are single-use, session/params/revision bound, five-minute TTL; attached askInApp portion deferred to builder M4."
    elif section == "6.4":
        src, test = "policy", "protocol"
        state, note = "deferred-with-owner", "Builder M4 attached coexistence/indicator; programmatic pause/resume/disconnect controls exist and are tested now."
    elif section == "6.5":
        state, src, test, note = "divergent", "protocol", "new", "C14-R1-11: wrapper-only limits; batch scripts ignore configured maxBatchSteps. Input frame limit is applied after allocation."
    elif section == "7":
        src, test = "tools", "protocol"
        if "Every tool result" in text:
            state, test, note = "divergent", "new", "Envelope schema validates the exercised outputs, but generic get_state differs from app.state.get (16); accuracy of deferred render annotations is not certified."
    elif section == "7.1":
        src, test = "tools", "protocol"
        if "list_commands" in text:
            state, test, note = "divergent", "new", "C14-R1-05: valid glob can occupy call worker indefinitely."
        if "get_state" in text:
            state, test, note = "divergent", "new", "C14-R1-16: wrapper adds/projections differ from app.state.get; virtual-view fields belong to M4."
    elif section == "7.2":
        src, test = "tools", "protocol"
        if any(x in text for x in ["run_command", "dryRun:", "requestId"]):
            state, test, note = "divergent", "new", "C14-R1-01/04: mistyped safety options disappear; accepted elicitation retries do not return the first successful envelope."
    elif section == "7.3":
        src, test = "executor", "protocol"
        if "atomic: true" in text or "| `batch`" in text:
            state, test, note = "divergent", "new", "C14-R1-03: activity export escapes a failed atomic batch; document rollback itself works."
    elif section == "7.4":
        src, test = "tools", "protocol"
        note = "doc.* convenience mappings; registry validates the retained command parameters. C14-R1-01 covers wrapper safety-option validation."
    elif section in {"7.5", "7.6"}:
        state, src, test, note = "deferred-with-owner", "tools", "schema", "Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared."
    elif section == "7.7":
        src, test = "tools", "protocol"
    elif section == "7.8":
        src, test = "tools", "protocol"
        if "get_state" in text:
            state, test, note = "divergent", "new", "C14-R1-16: convenience result is augmented, not identical to app.state.get."
        elif any(x in text for x in ["render_", "inspect_ui", "describe_frame", "pointer"]):
            state, note = "deferred-with-owner", "Builder M2/M3/M4 according to mapped command ownership; wrappers for rendering still need full parameter parity."
    elif section == "8":
        src, test = "executor", "schema"
        if "Validation." in text:
            state, src, test, note = "divergent", "tools", "new", "C14-R1-01: command JSON Schema validation works, generic wrapper inputSchema is not enforced."
        elif "Defaults." in text:
            state, note = "divergent", "P2 ticket 16: no universal result.effectiveParams map; effective values are returned inconsistently. Builder M2 command-contract follow-up."
    elif section.startswith("9"):
        src, test = "resources", "protocol"
        if section == "9.2":
            src = "events"
            if any(x in text for x in ["state.frameChanged", "state.selectionChanged", "state.toolChanged", "playback.started", "ui.dialogOpened", "Area events"]):
                state, note = "deferred-with-owner", "Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now."
            if "doc.undone" in text:
                state, note = "missing", "No doc.undone/doc.redone producer in executor; undo/redo emit doc.changed. M2 kernel event-contract follow-up required."
        elif any(x in text for x in ["xsheet", "Rendered drawing", "Rendered frame"]):
            state, note = "deferred-with-owner", "Builder M3 render/xsheet commands; template is discoverable now."
    elif section == "10":
        src, test = "docs", "protocol"
        note = "Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory."
    elif section == "11":
        src, test = "audit", "audit"
        if "Every agent call" in text:
            state, test, note = "divergent", "new", "C14-R1-12: accepting elicitation creates two records; declining logs pending status instead of final outcome."
        elif "Activity" in text:
            state, note = "deferred-with-owner", "Builder M4 live panel. Existing batch export is divergent (C14-R1-15: drops batches, turns dry runs into edits)."
    elif section == "12":
        src, test = "prompts", "protocol"
        note = "All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones."
    elif section == "13":
        src, test = "docs", "protocol"
        if "Projections" in text or "Compact defaults" in text:
            src, note = "tools", "Projection/pagination through doc.* plus get_state/list_commands; see 01 for malformed wrapper page/options."
    elif section == "14":
        src, test = "registry", "schema"
        if any(x in text for x in ["Gesture equivalence", "Determinism tests", "E1 Bouncing", "E2 Paint", "E3 Lip", "E4 Cut", "E5 Story", "E6 Coexistence"]):
            state, note = "deferred-with-owner", "Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4)."
        elif "Never-block" in text:
            state, src, test, note = "divergent", "protocol", "new", "719 examples and 719 malformed params answer; valid filter still blocks worker (05), restart gap remains ADR-0015 known gap 3."
        elif "Protocol tests" in text:
            state, src, test, note = "divergent", "protocol", "new", "SDK 24 checks pass on all OSes, while reviewer protocol regressions fail."
        elif "E7 Safety" in text:
            state, src, test, note = "divergent", "confirm", "new", "Malformed elicitation bypasses confirmation (02); network half awaits first network command (builder owner)."
        elif "Round-trip" in text:
            state, note = "deferred-with-owner", "M1 kernel roundtrips covered; create-via-domain-command for every type follows implementing milestones."
        elif "Parity test" in text:
            state, note = "deferred-with-owner", "Registry bindings/agentOnly gate exists now; exhaustive real GUI action coverage is builder M4."
    return state, source[src], tests[test], note


def add(section, line, text, kind="requirement"):
    status, implementation, test, note = decision(section, text)
    rows.append({"id": f"S11-L{line}", "section": section, "line": line, "requirement": text,
                 "status": status, "implementation": implementation, "test": test, "note": note, "kind": kind})


lines = S11.read_text(encoding="utf-8").splitlines()
section = "0"
fenced = False
i = 0
while i < len(lines):
    line = lines[i]
    if line.startswith("```"):
        fenced = not fenced
        i += 1
        continue
    heading = re.match(r"^#{2,3} (\d+(?:\.\d+)?)\.? ", line)
    if heading:
        section = heading[1]
    if fenced or section == "0" or section.startswith("15") or section == "16":
        i += 1
        continue
    if line.startswith("|") and not re.match(r"^\|[-:| ]+\|$", line) and i and lines[i-1].startswith("|") and not lines[i+1:i+2] == []:
        # Header rows are followed by separators; only data rows are units.
        if i+1 < len(lines) and re.match(r"^\|[-:| ]+\|$", lines[i+1]):
            i += 1
            continue
        add(section, i+1, line.strip(), "table row")
    elif re.match(r"^\s*(?:[-*]|\d+\.) ", line):
        start = i
        value = line.strip()
        while i+1 < len(lines) and lines[i+1].startswith("  ") and not re.match(r"^\s*(?:[-*]|\d+\.) ", lines[i+1]):
            i += 1
            value += " " + lines[i].strip()
        add(section, start+1, value, "list rule")
    elif line and not line.startswith(("#", "|", "---", " ")) and line != "Requirements:":
        start = i
        value = line
        while i+1 < len(lines) and lines[i+1] and not lines[i+1].startswith(("#", "|", "-", "```")):
            i += 1
            value += " " + lines[i]
        add(section, start+1, value, "normative prose")
    i += 1

# One separate declaration register covers every §15 and spec A declaration, including app.info.get.
declarations = []
for line_no, line in enumerate((ROOT / "spec/A_command_registry.md").read_text(encoding="utf-8").splitlines(), 1):
    match = re.match(r"^\| `((?:agent|inspect|checkpoint|job)\.[^`]+|app\.info\.get)`", line)
    if not match:
        continue
    cid = match[1]
    path = next(p for p in (ROOT / "crates/executor/src/commands").glob("*.rs") if f'id = "{cid}"' in p.read_text(encoding="utf-8"))
    rel = path.relative_to(ROOT).as_posix()
    status, note = "implemented", "Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable."
    if cid.startswith("inspect."):
        status, note = "deferred-with-owner", "Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present."
    elif cid in {"agent.server.set", "agent.server.status", "agent.token.regenerate", "agent.confirmation.respond", "agent.confirmation.requested"}:
        status, note = "deferred-with-owner", "Builder M4: HTTP/attached user confirmation remains deferred; implemented in-memory parts do not complete this declaration's contract."
    elif cid == "agent.activity.export":
        status, note = "divergent", "03/15: file effects survive rollback; batch replay loses committed batches and executes dry-run edits."
    elif cid == "agent.policy.defaults.set":
        status, note = "divergent", "06/11: persisted defaults are not rehydrated; configured limits are wrapper-only."
    declarations.append({"id": cid, "spec": f"spec/A_command_registry.md:{line_no}", "declaration": line,
                         "status": status, "implementation": loc(rel, f'id = "{cid}"'), "note": note,
                         "test": "crates/executor/tests/c12_kernel_state.rs::every_example_runs_on_the_fixture_under_the_watchdog; crates/executor/tests/c12_registry.rs"})

counts = dict(collections.Counter(r["status"] for r in rows))
decl_counts = dict(collections.Counter(r["status"] for r in declarations))
md = ["# M1-C1.4 round 1 conformance register", "", "Baseline: `3906001`; reviewed specification is unchanged.", "",
      "Units: each data table row, list rule (with continuation lines), and prose paragraph in spec 11 §§1–14; §15 and every matching spec A row are covered by the separate declaration register. A compound unit is divergent if any implemented M1 part fails; domain-owned work is deferred with its owner. This includes the nine principles, beyond the requested §§2–15.", "",
      f"Requirements: **{len(rows)}**; {counts}. Declarations: **{len(declarations)}**; {decl_counts}.", "",
      "`spec/01_application_shell.md` §1.2 is **Quit**, and contains no headless rules. The requested headless rules are spec 11 §3 and are registered above; shell §1.2 remains the inherited C1.3 quit contract (app.quit, scopes app, saved workspace/recovery), covered by c13_projects and contracts. This corrects the citation without inventing a missing requirement.", "",
      "Implementation and test references identify evidence, not a claim that one suite proves all deferred behavior. Error code inventory and obligation/evidence limitations are in the report and verification.json.", "",
      "| Unit | Requirement | Status | Implementation | Test / evidence | Ruling |", "|---|---|---|---|---|---|"]
def esc(text): return text.replace("|", "\\|").replace("\n", " ")
for row in rows:
    md.append(f"| {row['id']} (§{row['section']}) | {esc(row['requirement'])} | {row['status']} | `{row['implementation']}` | {row['test']} | {row['note']} |")
md += ["", "## Declarations: params, result, kind, undo, enablement, scopes and examples", "",
       "The spec A row is retained verbatim below. Source locations anchor the matching typed declaration (nearby structs hold params/results; attributes hold scopes/undo/needs_project/enablement). Registry assembly and typed examples verify schema admission; protocol regressions identify boundary divergences. Several map-valued result schemas remain permissive rather than declaring each field; this is not evidence of strong output-shape coverage.", "",
       "| Command / spec A | Complete declaration | Status | Implementation | Test | Ruling |", "|---|---|---|---|---|---|"]
for row in declarations:
    md.append(f"| `{row['id']}` ({row['spec']}) | {esc(row['declaration'])} | {row['status']} | `{row['implementation']}` | {row['test']} | {row['note']} |")
md += ["", "## Remaining event producer gap", "", "`doc.undone` / `doc.redone` are specified but have no producer; undo/redo emit `doc.changed`. This is a carried kernel event-contract gap (P2 ticket C14-R1-16), distinct from the now-working subscribed doc.changed path. State/playback/UI event names are tracked under their implementing domain milestones.", ""]
(HERE / "conformance.md").write_text("\n".join(md), encoding="utf-8")
(HERE / "conformance.json").write_text(json.dumps({"counts": counts, "declarationCounts": decl_counts, "requirements": rows, "declarations": declarations}, indent=2), encoding="utf-8")
print(len(rows), counts, len(declarations), decl_counts)
