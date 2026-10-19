"""Independent R3 schema audit. No imports from builder or previous audit tooling."""
import copy
import json
import random
import re
from collections import Counter
from pathlib import Path

from jsonschema import Draft202012Validator

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SEED = 0xC12320261019


def write(name, value):
    (HERE / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def holes(schema, path=""):
    """Independently transcribed R2 criterion, including its exact traversal limits."""
    found = []
    if not isinstance(schema, dict):
        return found
    if not any(k in schema for k in ("type", "$ref", "enum", "const", "oneOf", "anyOf", "allOf", "not", "required")):
        found.append(path or "/")
    if "x-specType" in schema:
        found.append(path or "/")
    for key in ("properties", "$defs"):
        for name, child in schema.get(key, {}).items():
            found.extend(holes(child, f"{path}/{key}/{name}"))
    for key in ("items", "x-jobResult"):
        if isinstance(schema.get(key), dict):
            found.extend(holes(schema[key], f"{path}/{key}"))
    for key in ("oneOf", "anyOf", "allOf"):
        for i, child in enumerate(schema.get(key, [])):
            found.extend(holes(child, f"{path}/{key}/{i}"))
    return found


def walk(schema, path=""):
    if not isinstance(schema, dict):
        return
    yield path, schema
    for key in ("properties", "$defs", "patternProperties"):
        for name, child in schema.get(key, {}).items():
            yield from walk(child, f"{path}/{key}/{name}")
    for key in ("items", "additionalProperties", "x-jobResult", "not", "if", "then", "else"):
        if isinstance(schema.get(key), dict):
            yield from walk(schema[key], f"{path}/{key}")
    for key in ("oneOf", "anyOf", "allOf", "prefixItems"):
        for i, child in enumerate(schema.get(key, [])):
            yield from walk(child, f"{path}/{key}/{i}")


def leaves(value, path=()):
    if isinstance(value, dict):
        for k, v in value.items():
            yield from leaves(v, (*path, k))
    elif isinstance(value, list):
        for i, v in enumerate(value):
            yield from leaves(v, (*path, i))
    else:
        yield path


def mutate(value, path):
    out = copy.deepcopy(value)
    if not path:
        return {"__r3_bad_leaf": True}
    target = out
    for token in path[:-1]:
        target = target[token]
    target[path[-1]] = {"__r3_bad_leaf": True}
    return out


def main():
    rows = json.loads((HERE / "registry.json").read_text(encoding="utf-8"))
    generated = json.loads((ROOT / "crates/registry/src/declared.json").read_text(encoding="utf-8"))
    print("generated shape", type(generated).__name__)
    if isinstance(generated, list):
        generated = {r["id"]: r for r in generated}
    selected = sorted(random.Random(SEED).sample(sorted(r["id"] for r in rows), 80))
    spot = sorted(random.Random(SEED + 1).sample(sorted(r["id"] for r in rows if r["status"] == "implemented"), 40))
    write("samples.json", {"seed": hex(SEED), "random80": selected, "spot40Seed": hex(SEED + 1), "implemented40": spot})
    appendix = {}
    for n, line in enumerate((ROOT / "spec/A_command_registry.md").read_text(encoding="utf-8").splitlines(), 1):
        if re.match(r"\| `[^`]+` \| (action|toggle|query|job|ui|event) \|", line):
            cells = [s.strip().replace(r"\|", "|") for s in re.split(r"(?<!\\)\|", line)[1:-1]]
            appendix[cells[0].strip("`")] = dict(zip(("id", "kind", "summary", "params", "result", "undo"), cells)) | {"line": n}
    assert len(rows) == len(appendix) == 796
    assert {r["id"] for r in rows} == set(appendix)
    results = []
    generated_checks = []
    for r in rows:
        cid = r["id"]
        a = appendix[cid]
        assert r["kind"] == a["kind"]
        assert r["spec"] == {k: a[k] for k in ("params", "result", "undo")}
        record = {"id": cid, "generated": cid in generated, "holes": {}, "openObjects": [], "unresolvedRefs": [], "validExamples": 0, "mutations": []}
        for part in ("params", "result"):
            schema = r[part + "Schema"]
            Draft202012Validator.check_schema(schema)
            record["holes"][part] = holes(schema)
            for path, child in walk(schema):
                if (child.get("type") == "object" or "properties" in child) and "additionalProperties" not in child:
                    record["openObjects"].append(f"{part}{path}")
                if "$ref" in child:
                    ref = child["$ref"]
                    try:
                        assert ref.startswith("#/")
                        resolved = schema
                        for token in ref[2:].split("/"):
                            resolved = resolved[token.replace("~1", "/").replace("~0", "~")]
                    except (KeyError, AssertionError):
                        record["unresolvedRefs"].append(f"{part}{path}: {ref}")
        validator = Draft202012Validator(r["paramsSchema"])
        for ex in r["examples"]:
            assert validator.is_valid(ex["params"]), (cid, ex, list(validator.iter_errors(ex["params"])))
            record["validExamples"] += 1
            if cid in selected:
                mutations = [{"path": list(p), "rejected": not validator.is_valid(mutate(ex["params"], p))} for p in leaves(ex["params"])]
                if not mutations:
                    mutations = [{"path": ["__r3_unknown"], "rejected": not validator.is_valid({**ex["params"], "__r3_unknown": {}}), "note": "empty example has no leaf; unknown-field mutation"}]
                record["mutations"].extend(mutations)
        results.append(record)
        if cid in selected and cid in generated:
            for part, sample in generated[cid]["samples"].items():
                schema = r["paramsSchema"] if part == "params" else r["resultSchema"]
                if part == "jobResult":
                    schema = schema["x-jobResult"] | {"$defs": schema.get("$defs", {})}
                validator = Draft202012Validator(schema)
                check = {"id": cid, "part": part}
                if isinstance(sample, dict) and "x-unsampleable" in sample:
                    check["skippedModelSample"] = sample
                else:
                    assert validator.is_valid(sample), (cid, part, sample)
                    check["valid"] = True
                    check["mutations"] = [{"path": list(p), "rejected": not validator.is_valid(mutate(sample, p))} for p in leaves(sample)]
                    if not check["mutations"]:
                        bad = {"__r3_unknown": {}} if isinstance(sample, dict) else [{"__r3_unknown": {}}]
                        check["mutations"] = [{"path": [], "rejected": not validator.is_valid(bad), "note": "no scalar leaf in sample; wrong-content mutation"}]
                generated_checks.append(check)
    write("schema-audit.json", results)
    write("sample80-review.json", [r | {"audit": next(a for a in results if a["id"] == r["id"])} for r in rows if r["id"] in selected])
    write("spot40-review.json", [r for r in rows if r["id"] in spot])
    summary = {"entries": len(rows), "declaredStatus": dict(Counter(r["status"] for r in rows)), "generatedEntries": sum(r["generated"] for r in results), "generatedRowsWithR2Holes": sum(r["generated"] and any(r["holes"].values()) for r in results), "unresolvedReferences": sum(len(r["unresolvedRefs"]) for r in results), "examples": sum(r["validExamples"] for r in results), "sampleMutations": sum(len(r["mutations"]) for r in results), "sampleAcceptedMutations": [{"id": r["id"], "mutation": m} for r in results for m in r["mutations"] if not m["rejected"]]}
    write("schema-summary.json", summary)
    write("sample-generated-mutations.json", generated_checks)
    summary["generatedSampleParts"] = len(generated_checks)
    summary["generatedUnsampleableModelParts"] = sum("skippedModelSample" in c for c in generated_checks)
    summary["generatedSampleMutations"] = sum(len(c.get("mutations", [])) for c in generated_checks)
    summary["acceptedGeneratedSampleMutations"] = [{"id": c["id"], "part": c["part"], "mutation": m} for c in generated_checks for m in c.get("mutations", []) if not m["rejected"]]
    write("schema-summary.json", summary)
    issue_text = (ROOT / "plan/spec-issues.md").read_text(encoding="utf-8")
    families = {m.group(2): "SI-" + m.group(1) for m in re.finditer(r"^## SI-(\d+).*`(\w+)\.\*`", issue_text, re.M)}
    placeholders = []
    for cid, entry in generated.items():
        for placeholder in entry.get("placeholders", []):
            family = cid.split('.')[0]
            assert family in families, cid
            placeholders.append({"id": cid, "placeholder": placeholder, "issue": families[family]})
    write("placeholder-issue-audit.json", {"rows": placeholders, "families": sorted({p['id'].split('.')[0] for p in placeholders}), "qualification": "Issue presence is verified; it cannot justify placeholders for shapes actually defined by owning sections. See R3-01."})
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
