"""Generates the typed declarations of every spec A command that has no hand-written declaration yet (C12-R1-12).

Run from the repository root: python scripts/gen-command-declarations.py

Inputs: spec/A_command_registry.md (read-only; parsed by scripts/gen-command-inventory.py) and the hand-written
`#[command(...)]` declarations in crates/executor/src (their ids are excluded). Outputs:

- crates/registry/src/declared.rs: one `Command` per remaining entry: kind, label, scopes (11 §6.1), undo label,
  confirmation conditions (11 §6.3), needsProject (01 §1.3), enablement text, owning milestone/checkpoint (plan §4),
  GUI location or agent-only justification (11 §14.1), one example, and a handler that is never reached (the executor
  answers `command.notImplemented` after validating the parameters).
- crates/registry/src/declared.json: the parameter and result JSON Schemas (draft 2020-12) of those entries, derived
  from spec A's parameter and result columns: types, ranges, enums, defaults, required parameters, unknown parameters
  rejected. Jobs carry the schema of their final result under `x-jobResult`.

When a later checkpoint implements a command with a hand-written declaration, re-running the generator drops it from
these files (the registry refuses an id declared twice). `crates/registry/tests/declared.rs` checks the outputs are
current, every example validates and every owner is the plan's.
"""
import importlib.util
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT_RS = ROOT / "crates" / "registry" / "src" / "declared.rs"
OUT_JSON = ROOT / "crates" / "registry" / "src" / "declared.json"
SCHEMA = "https://json-schema.org/draft/2020-12/schema"

_spec = importlib.util.spec_from_file_location("inventory", ROOT / "scripts" / "gen-command-inventory.py")
inventory = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(inventory)

MAX_STRING = 65_536
MAX_ITEMS = 100_000
ID = {"type": "string", "minLength": 1, "maxLength": 4096, "description": "object ID or path reference (10 §3)"}
PATH = {"type": "string", "minLength": 1, "maxLength": 4096, "description": "file-system path (11 §6.2)"}
NUMBER = {"type": "number"}
POINT = {
    "type": "object",
    "properties": {"x": NUMBER, "y": NUMBER},
    "required": ["x", "y"],
    "additionalProperties": False,
}
COLOUR = {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$", "description": "#RRGGBB or #RRGGBBAA"}
PRIMITIVES = {
    "id": ID,
    "path": PATH,
    "int": {"type": "integer"},
    "integer": {"type": "integer"},
    "number": NUMBER,
    "float": NUMBER,
    "bool": {"type": "boolean"},
    "boolean": {"type": "boolean"},
    "string": {"type": "string", "maxLength": MAX_STRING},
    "code": {"type": "string", "maxLength": 256},
    "base64": {"type": "string", "maxLength": 256 * 1024 * 1024, "description": "base64 text"},
    "point": POINT,
    "colour": COLOUR,
    "color": COLOUR,
    "object": {"type": "object"},
    "any": {},
    "null": {"type": "null"},
    "value": {"description": "any JSON value of the property's type"},
    "enum": {"type": "string", "description": "one of the values the owning section lists"},
}


# ----- owners (plan §4; C12-R1-21) -----------------------------------------------------------------------------------

ANIM_C32 = (
    # C3.2: transforms, pivots, TCB paths, velocity, channels, easing, cameras, projection, anim.evaluate.
    "anim.evaluate", "anim.changed", "anim.transform.set", "anim.projection.get", "anim.loop.set",
    "anim.camera.create", "anim.camera.delete", "anim.camera.get", "anim.camera.list", "anim.camera.set",
    "anim.camera.current.get", "anim.camera.current.set", "anim.camera.currentChanged",
    "anim.function.curveType.set", "anim.function.get", "anim.function.key.add", "anim.function.key.duplicate",
    "anim.function.key.remove", "anim.function.key.set", "anim.function.list", "anim.function.preset.apply",
    "anim.function.sample", "anim.function.set",
    "anim.key.add", "anim.key.list", "anim.key.move", "anim.key.remove",
    "anim.path.get", "anim.path.lockDepth.set", "anim.path.point.add", "anim.path.point.remove",
    "anim.path.point.set", "anim.path.sample", "anim.path.set",
    "anim.segment.set", "anim.segment.uniform.apply",
)
ANIM_C62 = (
    # C6.2: motion path UI, keyframes, function editor, velocity, easing, cameras UI, motion recording.
    "anim.camera.select", "anim.path.point.select", "anim.channel.copy", "anim.channel.cut", "anim.channel.paste",
    "anim.point.copy", "anim.point.paste",
)


def owner_of(cid: str) -> str:
    """Owning milestone/checkpoint of a command (plan §4 milestone table)."""
    exact = {
        "render.frame.run": "M3 C3.3",
        "render.limits.get": "M3 C3.3",
        "render.range.run": "M3 C3.3",
        "import.movie.run": "M8 C8.3",  # moved with the decoder to M8 C8.3 (plan §4, 2026-10-18; ADR-0008)
        "library.starter.open": "M9 C9.3",
    }
    if cid in exact:
        return exact[cid]
    ns, rest = cid.split(".", 1)
    if ns == "anim":
        if cid in ANIM_C32:
            return "M3 C3.2"
        if cid in ANIM_C62 or rest.startswith(("functionEditor.", "record.")):
            return "M6 C6.2"
        if rest.startswith(("bone.", "skeleton.")):
            return "M6 C6.3"
        return "M6 C6.1"  # views, display options, animation tools, peg-only mode, flips, hierarchy commands
    if ns == "colour":
        if rest.startswith(("paint", "gap")):
            return "M2 C2.3"  # `colour.paint.*` with `draw.*`; gap closing is a paint operation (C2.2 engine)
        return "M7 C7.1"
    if ns == "draw":
        return "M2 C2.4" if rest.startswith(("vectorize", "vectorFile")) else "M2 C2.3"
    if ns == "drawAid":
        return "M5 C5.2" if rest.startswith("staticLightTable") else "M4 C4.2"
    if ns in ("element",):
        return "M3 C3.1"
    if ns in ("cell", "xsheet"):
        return "M5 C5.2" if KIND[cid] in ("ui", "toggle") else "M3 C3.1"  # model commands C3.1; view commands C5.2
    if ns == "timeline":
        return "M5 C5.1"
    if ns == "fx":
        if rest.startswith(("block.", "particles", "simulation", "spriteSet", "preview.")):
            return "M7 C7.3"
        return "M7 C7.2"
    if ns == "sound":
        return "M8 C8.1"
    if ns == "lipsync":
        return "M8 C8.2"
    if ns == "media":
        return "M8 C8.3"
    if ns == "library":
        return "M9 C9.1"
    if ns == "storyline":
        return "M9 C9.2"
    if ns == "playback":
        return "M10 C10.1" if rest.startswith(("preview", "player")) else "M5 C5.1"
    if ns == "export":
        return "M10 C10.2" if rest.startswith(("drawingPdf", "publish")) else "M10 C10.1"
    if ns in ("import", "print"):
        return "M10 C10.2"
    if ns == "capture":
        return "M10 C10.3"
    if ns == "tool":
        return "M4 C4.2"
    if ns == "view":
        return "M4 C4.2"
    if ns in ("app", "edit", "window", "workspace", "prefs", "keymap", "ui"):
        return "M4 C4.1"
    if ns in ("project", "doc"):
        return "M1 C1.3"
    if ns == "agent":
        return "M1 C1.4"
    if ns in ("checkpoint", "job"):
        return "M1 C1.2"
    if ns == "inspect":
        return "M3 C3.3"
    raise SystemExit(f"no owner for {cid}")


# ----- scopes, project, confirmation, GUI (11 §6.1, 01 §1.3, 11 §6.3, 11 §14.1) ------------------------------------

READ_VERBS = ("import", "open", "probe", "relink", "addFromFile", "keyedImage.get", "file.get", "vectorize.preview",
              "swatch.set", "sequence.add")
WRITE_VERBS = ("export", "save", "snapshot", "render", "print.run", "library.create", "publish.run", "preview.run")
PREFS = ("prefs.set", "prefs.reset", "keymap.set", "keymap.remove", "keymap.reset", "keymap.import", "anim.prefs.set",
         "fx.preset.create", "fx.preset.delete", "fx.preset.rename", "fx.preset.import", "export.movie.defaults.set")
NETWORK = ("export.publish.run", "export.publish.account.open")
NO_PROJECT_PREFIXES = ("prefs.", "keymap.", "window.", "workspace.", "ui.theme", "ui.help", "ui.contextHelpMode",
                       "ui.preferences", "ui.startScreen", "ui.tips", "ui.statusBar")
NO_PROJECT = ("library.save", "library.open", "library.close", "library.create", "library.list", "library.tree.get")

GUI = {
    "anim": "Camera, Top and Side views, the Function editor and the Animation menu (05)",
    "fx": "effect elements, their parameter rows and the Effect preview panel (06)",
    "draw": "the Drawing view, the Drawing menu and tool properties (02)",
    "drawAid": "the Drawing view toolbar and the View menu (02)",
    "colour": "the Palette panel and the colour editor (03)",
    "library": "the Library panel (08)",
    "storyline": "the Storyline panel (08)",
    "element": "the Timeline and Exposure Sheet layer menus and the Properties panel (04)",
    "cell": "the Exposure Sheet and Timeline cell menus (04)",
    "xsheet": "the Exposure Sheet panel (04)",
    "timeline": "the Timeline panel (04)",
    "sound": "the Sound editor, Timeline sound layers and the Sound menu (07)",
    "lipsync": "the Lip-sync panel (07)",
    "media": "media elements in the Timeline and File > Import (07, 09)",
    "tool": "the Tools toolbar and tool properties (02, 05)",
    "capture": "the Capture window (09)",
    "export": "File > Export dialogs (09)",
    "import": "File > Import dialogs (09)",
    "print": "File > Print and Page Setup (09)",
    "playback": "the Playback toolbar and the Play menu (09)",
    "render": "File > Export render actions (09)",
    "ui": "the Help menu, the status bar and Preferences (01)",
    "view": "the View menu and view toolbars (01)",
    "keymap": "Preferences > Keyboard Shortcuts (01)",
    "prefs": "the Preferences dialog (01)",
}


def scopes_of(e: dict, params_text: str) -> list[str]:
    cid, kind, undo = e["id"], e["kind"], e["undo"]
    if kind in ("query", "event"):
        out = ["read"]
    elif kind == "ui":
        out = ["ui"]
    elif kind == "toggle":
        out = ["edit"] if undo == "yes" else ["ui"]
    elif kind == "job":
        out = ["edit"] if undo == "yes" else ["read"]
    else:  # action
        out = ["edit"] if undo == "yes" else ["ui"]
    # Parameters of the file-system `path` type (`path: path`, `paths: [path]`), not geometric paths of points.
    has_path = re.search(r"\b\w+\??\s*:\s*\[?path\b", params_text) is not None
    if cid.endswith(".validate"):
        return out + (["files.read"] if has_path else [])  # validation reads; it writes nothing
    if has_path or cid in ("library.save",):
        if any(v in cid for v in WRITE_VERBS):
            out.append("files.write")
        if any(v in cid for v in READ_VERBS) or cid == "export.publish.run":
            out.append("files.read")
        if not any(s.startswith("files.") for s in out):
            out.append("files.read")
    if cid in PREFS:
        out = [s for s in out if s != "ui"] + ["prefs"]
    if cid in NETWORK:
        out.append("network")
    if cid == "print.run":
        out.append("print")
    if cid in ("library.save", "library.writeAccess.set"):
        out.append("library.global")
    order = ["read", "edit", "ui", "input", "files.read", "files.write", "prefs", "library.global", "network",
             "print", "app"]
    return sorted(set(out), key=order.index)


def needs_project(e: dict) -> bool:
    cid = e["id"]
    if e["kind"] == "event":
        return False
    if cid in NO_PROJECT or cid.startswith(NO_PROJECT_PREFIXES):
        return False
    return True


def confirm_of(e: dict, params_text: str, result_text: str, scopes: list[str]) -> list[str]:
    out = []
    if re.search(r"\b(overwrite|onExists)\b", params_text):
        out.append("overwriting an existing file")
    if "needs_confirmation" in result_text or re.search(r"\bconfirm\??\s*:", params_text):
        out.append("a destructive change the owning section lists (returns needs_confirmation)")
    if re.search(r"\bdiscard(Changes)?\??\s*:", params_text):
        out.append("discarding unsaved changes")
    if "network" in scopes:
        out.append("an outward network action")
    if "print" in scopes:
        out.append("printing to a physical printer")
    return out


# ----- spec type text → JSON Schema ----------------------------------------------------------------------------------

def split_top(s: str, sep: str) -> list[str]:
    """Splits on `sep` outside brackets, braces, parentheses and quotes."""
    out, depth, cur, q, i = [], 0, [], False, 0
    while i < len(s):
        ch = s[i]
        if ch == '"':
            q = not q
        elif not q and ch in "([{":
            depth += 1
        elif not q and ch in ")]}":
            depth -= 1
        if not q and depth == 0 and s.startswith(sep, i):
            out.append("".join(cur))
            cur = []
            i += len(sep)
            continue
        cur.append(ch)
        i += 1
    out.append("".join(cur))
    return [x.strip() for x in out]


NUM = r"[−-]?\d+(?:\.\d+)?"


def num(t: str):
    t = t.replace("−", "-")
    v = float(t)
    return int(v) if v.is_integer() else v


def apply_range(schema: dict, ann: str) -> None:
    """Applies `(1–10000)`, `(≥1)`, `(1..179)`, `(0–100 %)`, `(1–scene length)` to a numeric schema."""
    if schema.get("type") not in ("integer", "number"):
        return
    a = ann.strip()
    m = re.match(rf"^(?:≥|>=)\s*({NUM})", a)
    if m:
        schema["minimum"] = num(m.group(1))
        return
    m = re.match(rf"^>\s*({NUM})", a)
    if m:
        schema["exclusiveMinimum"] = num(m.group(1))
        return
    m = re.match(rf"^(?:≤|<=)\s*({NUM})", a)
    if m:
        schema["maximum"] = num(m.group(1))
        return
    m = re.match(rf"^({NUM})\s*(?:–|\.\.|-|to)\s*({NUM})?", a)
    if m:
        schema["minimum"] = num(m.group(1))
        if m.group(2):
            schema["maximum"] = num(m.group(2))


def strip_annotation(t: str) -> tuple[str, str | None]:
    """`int (1–10)` → (`int`, `1–10`); only a trailing top-level parenthesis is an annotation."""
    t = t.strip()
    if t.endswith(")"):
        depth = 0
        for i in range(len(t) - 1, -1, -1):
            if t[i] == ")":
                depth += 1
            elif t[i] == "(":
                depth -= 1
                if depth == 0:
                    head = t[:i].strip()
                    if head:
                        return head, t[i + 1:-1]
                    break
    return t, None


UNPARSED: list[str] = []
COORDINATES = ("x", "y", "z", "w", "h", "width", "height", "minX", "minY", "maxX", "maxY", "dx", "dy", "dz")


def parse_type(t: str, ctx: str) -> dict:
    t = t.strip()
    if not t:
        return {}
    t, ann = strip_annotation(t)
    alts = split_top(t, "|")
    if len(alts) > 1:
        consts, schemas = [], []
        for a in alts:
            a = re.sub(r"^enum\s+", "", a.strip())
            if re.fullmatch(r'"[^"]*"', a):
                consts.append(json.loads(a))
            else:
                schemas.append(parse_single(a, ctx))
        free = [x for x in schemas if x.get("type") == "string" and "enum" not in x and "pattern" not in x]
        if consts and free:
            # `enum "auto"|"new" | id`: an ID string already admits the keywords; they are documented on it.
            free[0]["description"] = (free[0].get("description", "") + " or " + " | ".join(map(json.dumps, consts)))
        elif consts:
            schemas.insert(0, {"type": "string", "enum": consts})
        out = schemas[0] if len(schemas) == 1 else {"anyOf": schemas}
    else:
        out = parse_single(t, ctx)
    if ann:
        apply_range(out, ann)
        if not any(k in out for k in ("minimum", "maximum", "exclusiveMinimum")):
            out = dict(out)
            out["description"] = (out.get("description", "") + f" ({ann})").strip()
    return out


def parse_single(t: str, ctx: str) -> dict:
    t = t.strip()
    t, ann = strip_annotation(t)
    t = re.sub(r"^enum\s+", "", t)
    if re.fullmatch(r'"[^"]*"', t):
        out = {"type": "string", "enum": [json.loads(t)]}
    elif t in ("true", "false"):
        out = {"const": t == "true"}
    elif re.fullmatch(r'"[A-Z]"\.\."[A-Z]"', t):
        out = {"type": "string", "pattern": f"^[{t[1]}-{t[-2]}]$"}
    elif re.fullmatch(NUM, t):
        out = {"const": num(t)}
    elif t.startswith("[") and t.endswith("]"):
        out = {"type": "array", "items": parse_type(t[1:-1], ctx), "maxItems": MAX_ITEMS}
    elif t.endswith("[]"):
        out = {"type": "array", "items": parse_single(t[:-2], ctx), "maxItems": MAX_ITEMS}
    elif t.startswith("{") and t.endswith("}"):
        out = parse_fields(t[1:-1], ctx, params=True)
    elif re.match(r"^object\b", t):
        rest = t[len("object"):].strip()
        if rest.startswith("{") and "}" in rest:
            out = parse_fields(rest[1:rest.index("}")], ctx, params=True)
        else:
            out = {"type": "object"}
            if rest:
                out["description"] = rest
    elif t in PRIMITIVES:
        out = dict(PRIMITIVES[t])
    elif re.fullmatch(r"[A-Z]\w*", t):
        out = {"type": "object", "description": f"`{t}` (owning section)", "x-specType": t}
    elif re.fullmatch(r"[a-z]\w*", t) and t.endswith("Id"):
        out = dict(ID)
    else:
        UNPARSED.append(f"{ctx}: {t}")
        out = {"description": t}
    if ann:
        apply_range(out, ann)
    return out


def parse_default(text: str):
    text = text.strip()
    try:
        return True, json.loads(text)
    except (ValueError, TypeError):
        return False, text


def parse_fields(text: str, ctx: str, params: bool) -> dict:
    """`a: int; b?: bool = true` (parameters) or `a, b: bool, c?` (results) → an object schema."""
    sep = ";" if (";" in text or not params) and ";" in text else ","
    if params and ";" not in text and "," in text:
        sep = ","
    props, required = {}, []
    for f in split_top(text, sep):
        f = f.strip()
        if not f or f in ("…", "..."):
            continue
        default = None
        parts = split_top(f, " = ")
        if len(parts) == 1:
            parts = split_top(f, "=")
        head = parts[0].strip()
        if len(parts) > 1:
            default = "=".join(parts[1:]).strip()
        m = re.match(r"^([A-Za-z_$][\w.$]*)(\[\])?(\|null)?(\?)?\s*(?::\s*(.*))?$", head, re.S)
        if not m:
            # `viewMatrix[16]`, `pan {x,y}`, `text (Markdown)`: fixed-length list, inline object, annotated field.
            g = re.match(r"^([A-Za-z_$][\w.$]*)(\?)?\s*(?:\[(\d+)\]|(\{.*\})|\((.*)\))$", head, re.S)
            if g:
                name, opt, n, obj, note = g.groups()
                if n:
                    schema = {"type": "array", "items": NUMBER, "minItems": int(n), "maxItems": int(n)}
                elif obj:
                    schema = parse_fields(obj[1:-1], f"{ctx}.{name}", params=params)
                else:
                    schema = {"description": note}
                props[name] = schema
                if not opt:
                    required.append(name)
                continue
            # e.g. `PrintParams` (a named parameter group): accept its fields.
            if re.fullmatch(r"[A-Z]\w*", head):
                continue
            UNPARSED.append(f"{ctx}: field `{f}`")
            continue
        name, arr, nullable, opt, ty = m.groups()
        if ty:
            schema = parse_type(ty, f"{ctx}.{name}")
        elif name in COORDINATES:
            schema = dict(NUMBER)  # `{x,y}`, `{x,y,width,height}`: geometry fields are numbers
        else:
            schema = {}
        if arr:
            schema = {"type": "array", "items": schema, "maxItems": MAX_ITEMS}
        if nullable:
            schema = {"anyOf": [schema, {"type": "null"}]} if schema else {}
        if default is not None:
            default, ann = strip_annotation(default)
            if ann:
                apply_range(schema, ann)
            ok, value = parse_default(default)
            if ok:
                schema = dict(schema)
                schema["default"] = value
            else:
                schema = dict(schema)
                schema["description"] = (schema.get("description", "") + f" Default: {value}.").strip()
        props[name] = schema
        if not opt and default is None:
            required.append(name)
    out = {"type": "object", "properties": props}
    if required:
        out["required"] = required
    if params:
        out["additionalProperties"] = False
    return out


def backticks(cell: str) -> list[str]:
    return re.findall(r"`([^`]*)`", cell)


def params_schema(e: dict, by_id: dict) -> dict:
    cell = e["params"].strip()
    if e["kind"] == "event":
        out = {"type": "object", "properties": {}, "additionalProperties": False,
               "description": "Events take no parameters; subscribe to them (11 §9)."}
    elif cell in ("—", "-", ""):
        out = {"type": "object", "properties": {}, "additionalProperties": False}
    elif cell.startswith("same as"):
        other = backticks(cell)[0]
        out = params_schema(by_id[other], by_id)
        out = {k: v for k, v in out.items() if k not in ("$schema", "title")}
    else:
        groups = backticks(cell)
        if len(groups) > 1 and " or " in cell:
            alts = [parse_fields(g, e["id"], params=True) for g in groups]
            props = {}
            for a in alts:
                props.update(a["properties"])
            out = {"type": "object", "properties": props, "additionalProperties": False,
                   "anyOf": [{"required": a.get("required", [])} for a in alts]}
        else:
            out = parse_fields(groups[0] if groups else cell, e["id"], params=True)
    return {"$schema": SCHEMA, "title": f"{e['id']} parameters", **out, "x-spec": e["params"]}


def result_part(text: str, ctx: str) -> dict:
    """One result expression: `{a, b: T}`, `[T]`, `Name`, `x[]`, prose."""
    t = re.sub(r"\(§[^)]*\)", "", text).strip()
    t = re.sub(r"\s*\((?:nested|fields|top-most first)[^)]*\)", "", t).strip()
    if t.startswith("{") and t.endswith("}"):
        return parse_fields(t[1:-1], ctx, params=False)
    if t.startswith("[") and t.endswith("]"):
        return {"type": "array", "items": parse_type(t[1:-1], ctx)}
    if re.fullmatch(r"[A-Z]\w*", t):
        return {"type": "object", "description": f"`{t}` (owning section)", "x-specType": t}
    if t in ("—", "-", ""):
        return {"type": "object", "properties": {}}
    return {"description": t}


def result_schema(e: dict, results: dict) -> dict:
    cell = e["result"].strip()
    if e["kind"] == "event":
        payload = e["params"] if e["params"].strip() not in ("—", "-", "") else cell
        groups = backticks(payload)
        out = result_part(groups[0] if groups else payload, e["id"])
        out["description"] = "Event payload (delivered to subscribers)."
    elif cell.startswith("same as"):
        target = e["id"].rsplit(".", 1)[0] + ".get"
        out = {k: v for k, v in results.get(target, {"description": cell}).items() if k not in ("$schema", "title")}
    else:
        groups = backticks(cell)
        prose = re.sub(r"`[^`]*`", "", cell)
        job = [g for g in groups if g.strip() == "{jobId}"]
        rest = [g for g in groups if g.strip() not in ("{jobId}", "needs_confirmation") and
                not g.strip().startswith("needs_confirmation")]
        if job:
            final = [result_part(g, e["id"]) for g in rest]
            out = {"type": "object", "properties": {"jobId": dict(ID)}, "required": ["jobId"]}
            if final:
                out["x-jobResult"] = final[0] if len(final) == 1 else {"anyOf": final}
        elif rest:
            parts = [result_part(g, e["id"]) for g in rest]
            out = parts[0] if len(parts) == 1 else {"anyOf": parts}
            if "Range = " in cell:
                out["description"] = prose.strip()
        else:
            if re.search(r"\b(object|JSON|state|settings)\b", cell):
                out = {"type": "object", "description": cell}
            else:
                out = {"description": cell}
    return {"$schema": SCHEMA, "title": f"{e['id']} result", **out, "x-spec": e["result"]}


# ----- examples -----------------------------------------------------------------------------------------------------

def sample(s: dict, name: str = ""):
    if "default" in s:
        return s["default"]
    if "const" in s:
        return s["const"]
    if "enum" in s:
        return s["enum"][0]
    if "anyOf" in s and "type" not in s:
        return sample(s["anyOf"][0], name)
    t = s.get("type")
    if t == "string":
        if s is PATH or s.get("description", "").startswith("file-system path"):
            return "review/example.out"
        if "pattern" in s:
            return "#FF8000FF"
        if s.get("description", "").startswith("object ID"):
            return "scene:#1"
        return "example"
    if t == "integer":
        lo = s.get("minimum", 1)
        hi = s.get("maximum")
        v = max(lo, 1)
        return v if hi is None or v <= hi else lo
    if t == "number":
        lo = s.get("minimum", 1)
        hi = s.get("maximum")
        v = max(lo, 1)
        return v if hi is None or v <= hi else lo
    if t == "boolean":
        return True
    if t == "null":
        return None
    if t == "array":
        return [sample(s.get("items", {}))] if s.get("minItems", 0) > 0 else []
    if t == "object":
        props = s.get("properties", {})
        req = list(s.get("required", []))
        if "anyOf" in s and s["anyOf"] and "required" in s["anyOf"][0]:
            req += s["anyOf"][0]["required"]
        return {k: sample(props[k], k) for k in req if k in props}
    return {}


def example_of(e: dict, ps: dict) -> tuple[str, dict]:
    params = sample({k: v for k, v in ps.items() if k not in ("$schema", "title", "x-spec")})
    if e["kind"] == "event":
        return "Subscribe (events take no parameters)", {}
    title = "Required parameters only" if params else "No parameters"
    return f"{title}: {e['summary'][:80]}", params


# ----- output ---------------------------------------------------------------------------------------------------------

KIND: dict[str, str] = {}


def rust_str(s: str) -> str:
    return inventory.rust_str(s)


def undo_label(e: dict) -> str | None:
    if e["undo"] != "yes":
        return None
    m = re.search(r'\("([^"]+)"\)', e["summary"])
    if m:
        return m.group(1)
    label = re.split(r"[;(]| — ", e["summary"])[0].strip().rstrip(".")
    return label[:60]


def label_of(e: dict) -> str:
    m = re.search(r'\("([^"]+)"\)', e["summary"])
    return m.group(1) if m else re.split(r"[;(]| — ", e["summary"])[0].strip().rstrip(".")[:80]


def typed_ids() -> set[str]:
    ids = set()
    for f in (ROOT / "crates" / "executor" / "src").rglob("*.rs"):
        ids |= set(re.findall(r'#\[command\(\s*id = "([^"]+)"', f.read_text(encoding="utf-8")))
    return ids


SCOPE_RS = {"read": "Read", "edit": "Edit", "ui": "Ui", "input": "Input", "files.read": "FilesRead",
            "files.write": "FilesWrite", "prefs": "Prefs", "library.global": "LibraryGlobal", "network": "Network",
            "print": "Print", "app": "App"}
KIND_RS = {"action": "Action", "toggle": "Toggle", "query": "Query", "job": "Job", "ui": "Ui", "event": "Event"}


def main() -> int:
    entries = inventory.parse()
    by_id = {e["id"]: e for e in entries}
    for e in entries:
        KIND[e["id"]] = e["kind"]
    typed = typed_ids()
    todo = [e for e in entries if e["id"] not in typed]
    schemas, results, rows = [], {}, []
    for e in todo:
        results[e["id"]] = result_schema(e, results) if e["result"].strip().startswith("same as") is False else None
    for e in todo:
        ps = params_schema(e, by_id)
        rs = results[e["id"]] or result_schema(e, results)
        results[e["id"]] = rs
        scopes = scopes_of(e, e["params"])
        title, ex = example_of(e, ps)
        rows.append((e, ps, rs, scopes, title, ex))
        schemas.append({"id": e["id"], "params": ps, "result": rs})
    if UNPARSED:
        print("unparsed fragments (kept as described any-values):", file=sys.stderr)
        for u in UNPARSED:
            print("  " + u, file=sys.stderr)

    OUT_JSON.write_text(json.dumps(schemas, ensure_ascii=False, indent=1) + "\n", encoding="utf-8", newline="\n")
    lines = [
        "// @generated by scripts/gen-command-declarations.py from spec/A_command_registry.md. Do not edit.",
        f"//! Typed declarations of the {len(rows)} spec A commands whose handlers belong to later checkpoints",
        "//! (C12-R1-12): schemas in `declared.json`, metadata here. See the generator for the derivation rules.",
        "",
        "use crate::{Command, Decl, Enablement, Example, Kind, Scope, Status};",
        "",
        "/// Every generated declaration, in spec order; `params_schema`/`result_schema` index `declared.json`.",
        "pub static DECLARED: &[Command] = &[",
    ]
    for i, (e, ps, rs, scopes, title, ex) in enumerate(rows):
        np = needs_project(e)
        confirm = confirm_of(e, e["params"], e["result"], scopes)
        owner = owner_of(e["id"])
        undo = undo_label(e)
        if e["kind"] == "event":
            agent_only, gui = "an event delivered to subscribers", None
        else:
            agent_only, gui = None, GUI.get(e["id"].split(".")[0], "the owning section's GUI") + f"; wired in {owner}"
        rule = ("needs an open project (01 §1.3); " if np else "") + f"see spec {e['section']} for the full rule"
        lines += [
            "    Command {",
            "        decl: Decl {",
            f"            id: {rust_str(e['id'])},",
            f"            kind: Kind::{KIND_RS[e['kind']]},",
            f"            label: {rust_str(label_of(e))},",
            f"            scopes: &[{', '.join('Scope::' + SCOPE_RS[s] for s in scopes)}],",
            f"            undo: {('Some(' + rust_str(undo) + ')') if undo else 'None'},",
            f"            confirm: &[{', '.join(rust_str(c) for c in confirm)}],",
            f"            enablement: Enablement {{ rule: {rust_str(rule)}, check: None }},",
            f"            needs_project: {'true' if np else 'false'},",
            f"            examples: &[Example {{ title: {rust_str(title)}, params: {rust_str(json.dumps(ex, ensure_ascii=False))} }}],",
            f"            params_schema: || crate::declared_schema({i}, false),",
            f"            result_schema: || crate::declared_schema({i}, true),",
            f"            status: Status::NotImplemented {{ owner: {rust_str(owner)} }},",
            f"            agent_only: {('Some(' + rust_str(agent_only) + ')') if agent_only else 'None'},",
            f"            gui: {('Some(' + rust_str(gui) + ')') if gui else 'None'},",
            "        },",
            "        handler: crate::not_implemented_handler,",
            "    },",
        ]
    lines.append("];")
    OUT_RS.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    # Formatted as `cargo fmt` would, so regenerating and formatting agree.
    subprocess.run(["rustfmt", "--edition", "2024", "--config-path", str(ROOT / "rustfmt.toml"), str(OUT_RS)],
                   check=True)
    print(f"{len(rows)} declarations -> {OUT_RS.relative_to(ROOT)}, {OUT_JSON.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
