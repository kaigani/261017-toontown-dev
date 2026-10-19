"""Closing pass of scripts/gen-command-declarations.py (C12-R2-02).

`close()` turns a schema parsed from spec A's columns into a closed, fully typed one:

- every object level is closed (`additionalProperties: false`), or is a typed map (`object (K → V)`);
- every leaf is typed: from the spec text, else from the field name (`command_types.leaf_for_name`), else a scalar
  placeholder (never an object or list) that is recorded;
- a named type resolves to the owning section's shape (`#/$defs/spec.<Name>`, from `command_types.catalogue()`), to a
  model object schema (`#/$defs/<Name>` from crates/model/schema), to a shape derived from those, or to a strict
  placeholder object (`{}` only) that is recorded;
- referenced definitions are embedded under `$defs`.

Every placeholder is returned with its pointer so the generator can list it (one spec issue per command family).
`sample()` builds an instance that uses every property (the generator stores one per schema for the negative test).
"""
import copy
import json
import pathlib
import re

import command_types as ct

ROOT = pathlib.Path(__file__).resolve().parents[1]
MODEL = json.loads((ROOT / "crates" / "model" / "schema" / "model-1.0.schema.json").read_text(encoding="utf-8"))
MODEL_DEFS: dict = MODEL["$defs"]

STRICT_OBJECT = {"type": "object", "properties": {}, "additionalProperties": False, "x-placeholder": True,
                 "description": "shape not defined by the owning section (spec issue); only {} is accepted until it is"}
WARNING = {"type": "object", "additionalProperties": False, "required": ["code", "message"],
           "properties": {"code": {"type": "string", "maxLength": 256}, "message": {"type": "string", "maxLength": 65536},
                          "objectIds": {"type": "array", "items": dict(ct.ID), "maxItems": 100000}}}

# Names the result columns use for model objects (prose or names) → model definition.
MODEL_ALIASES = {
    "element object": "ElementInfo",
    "exposure-sheet object": "ExposureSheet",
    "timeline object": "TimelineModel",
    "drawing JSON": "DrawingReadback",
    "Palette object": "PaletteObject",
    "Effect": "Element",
}


def _catalogue():
    types, _ = ct.catalogue()
    out = {}
    for name, schema in types.items():
        s = json.loads(json.dumps(schema).replace('"#/$defs/', '"#/$defs/spec.'))
        out[name] = s
    # BlockSpec (06 §11): Block without id/index, every field optional.
    if "Block" in out:
        b = copy.deepcopy(out["Block"])
        for k in ("id", "index"):
            b.get("properties", {}).pop(k, None)
        b.pop("required", None)
        b["x-derivedFrom"] = "Block"
        out["BlockSpec"] = b
    return out


SPEC_TYPES = _catalogue()
LOCAL_TYPES: dict = {}    # per command: `Range = {…}` written inline in a result cell
PARAM_GROUPS: dict = {}   # `PrintParams` = `…`: parameter groups spread into parameter lists


# Untyped fields that hold a section type of the same area (09: a capture session's `keying`, a job's `manifest`).
FIELD_TYPES = {"keying": "Keying", "manifest": "Manifest", "pen": "Pen"}


class Closer:
    def __init__(self, cid: str, fields: dict | None = None):
        self.cid = cid
        self.placeholders: list = []
        self.defs: set = set()
        # Typed fields of the command's context, by name (C12-R3-01): what an untyped field of the same name is.
        self.fields = fields or {}

    def note(self, pointer: str, what: str):
        self.placeholders.append({"pointer": pointer or "/", "kind": what})

    def extend(self, base: str, s: dict, at: str):
        """`Base & {…}` / `Base + {…}`: one closed object with the base's members and the extra ones."""
        raw = SPEC_TYPES.get(base) or PARAM_GROUPS.get(base)
        if raw is None or "properties" not in raw:
            self.note(at, f"named type `{base}`")
            return dict(STRICT_OBJECT, **{"x-undefinedType": base})
        merged = copy.deepcopy(raw)
        merged.pop("allOf", None)
        merged.pop("x-extends", None)
        merged["properties"] = {**merged.get("properties", {}), **s.get("properties", {})}
        merged["required"] = list(dict.fromkeys(merged.get("required", []) + s.get("required", [])))
        return self.close(merged, at, base)

    def named(self, name: str, at: str):
        if name in PARAM_GROUPS:
            return self.close(copy.deepcopy(PARAM_GROUPS[name]), at, name)
        if name in SPEC_TYPES and SPEC_TYPES[name].get("x-extends"):
            return self.extend(SPEC_TYPES[name]["x-extends"], SPEC_TYPES[name], at)
        if name in LOCAL_TYPES:
            return self.close(copy.deepcopy(LOCAL_TYPES[name]), at, name)
        if name in SPEC_TYPES:
            self.defs.add("spec." + name)
            return {"$ref": f"#/$defs/spec.{name}"}
        alias = MODEL_ALIASES.get(name)
        if alias and (alias in SPEC_TYPES or alias in LOCAL_TYPES or alias in PARAM_GROUPS):
            return self.named(alias, at)
        model = alias or (name if name in MODEL_DEFS and MODEL_DEFS[name].get("x-objectType") else None)
        if model:
            self.defs.add(model)
            return {"$ref": f"#/$defs/{model}"}
        self.note(at, f"named type `{name}`")
        s = dict(STRICT_OBJECT)
        s["x-undefinedType"] = name
        return s

    def close(self, s, at: str = "", name: str = ""):
        if not isinstance(s, dict):
            return s
        if s and set(s) <= {"required", "not", "minProperties", "maxProperties"}:
            return dict(s)  # a constraint-only alternative (`anyOf: [{required: [...]}, ...]`)
        if s.get("x-placeholder"):
            # A strict placeholder from a section's example (a member whose example value is only `null`).
            self.note(at, f"leaf `{name or '(value)'}`")
            return dict(s)
        if s.get("x-open"):
            return dict(s)  # an object the section leaves open on purpose (a bound command's parameters)
        s = dict(s)
        meta = {k: s[k] for k in ("$schema", "title", "description", "x-spec", "x-jobResult") if k in s}
        if "x-specType" in s:
            out = self.named(s.pop("x-specType"), at)
            return {**out, **{k: v for k, v in meta.items() if k != "x-jobResult"}} if meta else out
        if "x-extend" in s:
            out = self.extend(s.pop("x-extend"), s, at)
            return {**out, **{k: v for k, v in meta.items() if k != "x-jobResult"}} if meta else out
        if isinstance(s.get("$ref"), str):
            self.defs.add(s["$ref"].split("/")[-1])
            return s
        for k in ("anyOf", "oneOf", "allOf"):
            if k in s:
                s[k] = [self.close(x, at, name) for x in s[k]]
        if "x-jobResult" in s:
            s["x-jobResult"] = self.close(s["x-jobResult"], at + "/x-jobResult")
        t = s.get("type")
        if t == "object" or "properties" in s:
            props = s.get("properties")
            desc = s.get("description", "")
            m = re.search(r"\((\w+)\s*→\s*(\w+)\)", desc)
            if not props and m and "additionalProperties" not in s:
                value = ct.leaf_for_name(m.group(2)) or _primitive(m.group(2))
                if value is None:
                    self.note(at + "/*", f"map value `{m.group(2)}`")
                    value = dict(ct.SCALAR_PLACEHOLDER)
                s["additionalProperties"] = value
                s["properties"] = {}
                s["propertyNames"] = {"minLength": 1, "maxLength": 4096}
            elif not props and s.get("additionalProperties") is False:
                s["properties"] = {}
            elif not props and s.get("additionalProperties") in (None, True):
                if desc and not name:
                    pass
                self.note(at, "object without fields")
                return dict(STRICT_OBJECT, **{"description": (desc + " — " if desc else "") + STRICT_OBJECT["description"]})
            else:
                closed = {}
                for k, v in (props or {}).items():
                    if k in ("left", "right") and "time" in props and not (isinstance(v, dict) and _typed(v)):
                        closed[k] = dict(ct.NUMBER)  # an envelope point's channel gains (07 §9.5), not key handles
                        continue
                    closed[k] = self.leaf_or(v, f"{at}/{k}", k)
                s["properties"] = closed
                if not isinstance(s.get("additionalProperties"), dict):
                    s["additionalProperties"] = False
                s["type"] = "object"
        elif t == "array":
            s["items"] = self.leaf_or(s.get("items", {}), at + "/*", ct._singular(name) if name else name)
        elif not t and not any(k in s for k in ("$ref", "anyOf", "oneOf", "allOf", "const", "enum")):
            return self.untyped(s, at, name)
        return s

    def leaf_or(self, v, at: str, name: str):
        untyped_list = empty_list(v) or bare_object(v)
        if name in self.fields and (not isinstance(v, dict) or not _typed(v) or untyped_list):
            # The same field, typed by a sibling command or the command's own parameters.
            return self.close(copy.deepcopy(self.fields[name]), at, name)
        if not isinstance(v, dict) or not _typed(v):
            return self.untyped(v if isinstance(v, dict) else {}, at, name)
        return self.close(v, at, name)

    def untyped(self, s: dict, at: str, name: str):
        """A position the spec gives no type: from its name, else a recorded scalar placeholder."""
        desc = s.get("description", "")
        if name in FIELD_TYPES:
            return self.named(FIELD_TYPES[name], at)
        if name in ("warnings", "errors", "conflicts"):
            return {"type": "array", "items": dict(WARNING), "maxItems": 100000}
        if name in ("warning", "error"):
            return dict(WARNING)
        leaf = ct.leaf_for_name(name) if name else None
        if leaf is None and name.endswith("s"):
            single = ct.leaf_for_name(ct._singular(name))
            if single is not None and single.get("type") != "array":
                leaf = {"type": "array", "items": single, "maxItems": 100000}
        if leaf is None:
            self.note(at, f"leaf `{name or '(value)'}`")
            leaf = dict(ct.SCALAR_PLACEHOLDER)
        out = dict(leaf)
        if out.get("type") == "integer":
            # Only the name says it is numeric: no integer domain is inferred from a name (C12-R3-01).
            out["type"] = "number"
        for k in ("default", "nullable"):
            if k in s:
                out[k] = s[k]
        if desc and not desc.startswith("type not stated"):
            out["description"] = desc
        if s.get("x-nullable"):
            return {"anyOf": [out, {"type": "null"}]}
        return out

    def finish(self, root: dict) -> dict:
        """Embeds the referenced definitions (transitively) under `$defs`."""
        defs = {}
        todo = list(self.defs)
        seen = set()
        while todo:
            n = todo.pop()
            if n in seen:
                continue
            seen.add(n)
            if n.startswith("spec."):
                raw = SPEC_TYPES.get(n[5:])
                if raw is None:
                    continue
                c = Closer(self.cid)
                d = c.close(copy.deepcopy(raw), f"/$defs/{n}", n[5:])
                self.placeholders += c.placeholders
                todo += [x for x in c.defs if x not in seen]
            else:
                d = MODEL_DEFS.get(n)
                if d is None:
                    continue
            defs[n] = d
            for r in re.findall(r'"\$ref": "#/\$defs/([^"]+)"', json.dumps(d)):
                if r not in seen:
                    todo.append(r)
        if defs:
            root["$defs"] = dict(sorted(defs.items()))
        return root


def _primitive(word: str):
    w = word.lower()
    if w in ("int", "integer"):
        return dict(ct.INTEGER)
    if w in ("number", "float"):
        return dict(ct.NUMBER)
    if w in ("bool", "boolean"):
        return dict(ct.BOOLEAN)
    if w in ("string", "value"):
        return None if w == "value" else dict(ct.STRING)
    if w == "id" or w.endswith("id"):
        return dict(ct.ID)
    if w == "colour":
        return dict(ct.COLOUR)
    return None


def bare_object(s) -> bool:
    """An object the spec leaves without fields (`object`)."""
    return isinstance(s, dict) and s.get("type") == "object" and not s.get("properties") and         not isinstance(s.get("additionalProperties"), dict) and not s.get("x-open")


def empty_list(s) -> bool:
    """A list whose items the spec does not type (`x[]`, `[object]`)."""
    if not (isinstance(s, dict) and s.get("type") == "array"):
        return False
    items = s.get("items") or {}
    return not _typed(items) or bare_object(items)


def _typed(s: dict) -> bool:
    return any(k in s for k in ("type", "$ref", "anyOf", "oneOf", "allOf", "const", "enum", "x-specType"))


# ----- samples -------------------------------------------------------------------------------------------------------

class Unsampleable(Exception):
    pass


def sample(s, defs: dict, full: bool = True, depth: int = 0, name: str = ""):
    """An instance of `s` using every property (`full`) or only the required ones."""
    if depth > 24:
        raise Unsampleable("depth")
    if not isinstance(s, dict):
        raise Unsampleable("schema")
    if "$ref" in s:
        ref = s["$ref"].split("/")[-1]
        if ref not in defs:
            raise Unsampleable(ref)
        if not ref.startswith("spec."):
            raise Unsampleable(f"model type {ref}")
        return sample(defs[ref], defs, full, depth + 1, name)
    if s.get("x-open"):
        return {"x-open": True}  # an open object (any members); the negative test does not probe inside it
    if "default" in s:
        return s["default"]
    if "const" in s:
        return s["const"]
    if "enum" in s:
        return s["enum"][0]
    for k in ("anyOf", "oneOf"):
        if k in s and "type" not in s:
            for alt in s[k]:
                try:
                    return sample(alt, defs, full, depth + 1, name)
                except Unsampleable:
                    continue
            raise Unsampleable(k)
    if "allOf" in s:
        out = {}
        for part in s["allOf"]:
            v = sample(part, defs, full, depth + 1, name)
            if isinstance(v, dict):
                out.update(v)
        rest = {k: v for k, v in s.items() if k != "allOf"}
        if rest.get("properties"):
            out.update(sample(rest, defs, full, depth + 1, name))
        return out
    t = s.get("type")
    if isinstance(t, list):
        t = t[0]
    if t == "string":
        if "pattern" in s:
            if s["pattern"].startswith("^#"):
                return "#FF8000FF"
            m = re.match(r"\^\[(\w)-(\w)\]\$", s["pattern"])
            if m:
                return m.group(1)
            raise Unsampleable("pattern")
        d = s.get("description", "")
        if d.startswith("file-system path"):
            return "review/example.out"
        if d.startswith("object ID"):
            return "scene:#1"
        return "example"
    if t in ("integer", "number"):
        lo = s.get("minimum", s.get("exclusiveMinimum", 0) + 1 if "exclusiveMinimum" in s else 1)
        hi = s.get("maximum")
        v = max(lo, 1) if hi is None or max(lo, 1) <= hi else lo
        return int(v) if t == "integer" else v
    if t == "boolean":
        return True
    if t == "null":
        return None
    if t == "array":
        if s.get("maxItems", 1) == 0:
            return []
        n = max(s.get("minItems", 0), 1 if full and depth < 8 else 0)
        return [sample(s.get("items", {}), defs, full, depth + 1, ct._singular(name)) for _ in range(n)]
    if t == "object":
        props = s.get("properties", {})
        req = list(s.get("required", []))
        for alt in s.get("anyOf", []):
            if "required" in alt:
                req += [r for r in alt["required"] if r not in req]
                break
        keys = list(props) if full else req
        out = {}
        for k in keys:
            if k not in props:
                continue
            try:
                out[k] = sample(props[k], defs, full, depth + 1, k)
            except Unsampleable:
                if k in req:
                    raise
        if not keys and isinstance(s.get("additionalProperties"), dict):
            out["key1"] = sample(s["additionalProperties"], defs, full, depth + 1)
        return out
    raise Unsampleable(f"type {t}")
