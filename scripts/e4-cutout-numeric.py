#!/usr/bin/env python3
"""End-to-end scenario E4 "cut-out rig", numeric part (11 §14.8; M3 C3.2 readiness item 11) through the official MCP
Python SDK against the real `toontown-headless` binary.

Usage: python scripts/e4-cutout-numeric.py <path to toontown-headless> <out dir>

Chain: new project → a puppet (master peg → torso → head, arm peg with a mirror → forearm), element and peg pivots at
the joints → three posed keyframes → `anim.evaluate` at the keyframes and between them → compared within 1e-6 with this
script's own formulas (exact `fractions` where the angles allow, an independent evaluator otherwise; no product code) →
undo and redo → save → the saved project opened twice in new processes, the same requests answering the same bytes.
`evaluate.json` (IDs replaced by names, so it does not depend on the project's random ID) is byte-compared between
Windows and macOS by the `e4 cross-OS` CI job. `report.json` keeps the inputs, the outputs, the versions and the head SHA.
Exit status 0 when every check passes. Requires `pip install mcp`. Bones are M6 C6.3 (SI-097).
"""
import asyncio
import hashlib
import importlib.metadata
import json
import math
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from fractions import Fraction as Q
from pathlib import Path

from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

TOLERANCE = 1e-6
CHECKS = []
NAMES = ["Master", "Torso", "Head", "ArmPeg", "Forearm"]
PARENTS = {"Master": None, "Torso": "Master", "Head": "Torso", "ArmPeg": "Torso", "Forearm": "ArmPeg"}
KEY_FRAMES = [1, 12, 24]
BETWEEN = [6, 18.5, 3.25, 23]

# Static values (05 §1.5). Pivots: the master's between the feet, each part's at its joint.
STATIC = {
    "Master": {"offset": (0, 0, 0), "rotation": 0, "scale": (1, 1), "skew": 0, "cr": (0, -2), "cs": (0, -2)},
    "Torso": {"offset": (0, 1, Q(1, 4)), "rotation": 0, "scale": (1, 1), "skew": 0, "cr": (0, -1), "cs": (0, 0)},
    "Head": {"offset": (0, 2, Q(1, 100)), "rotation": 0, "scale": (1, 1), "skew": 0, "cr": (0, Q(-1, 2)), "cs": (0, Q(-1, 2))},
    "ArmPeg": {"offset": (1, 1, 0), "rotation": 0, "scale": (-1, 1), "skew": 0, "cr": (0, 0), "cs": (0, 0)},
    "Forearm": {"offset": (Q(3, 2), 0, Q(-1, 100)), "rotation": 0, "scale": (1, 1), "skew": 0, "cr": (Q(-1, 2), 0), "cs": (Q(-1, 2), 0)},
}
# The three poses: multiples of 90° at the keyframes where exact arithmetic applies.
POSES = {
    "Master": {"rotation": {1: 0, 12: 90, 24: 180}, "position": {1: (-4, 0, 0), 12: (0, 3, 0), 24: (4, 0, 2)}},
    "Torso": {"rotation": {1: 0, 12: -90, 24: 90}, "scale": {1: (1, 1), 12: (2, 1), 24: (2, Q(1, 2))}},
    "Head": {"rotation": {1: 90, 12: 0, 24: -90}, "skew": {1: 0, 12: 30, 24: 0}},
    "ArmPeg": {"rotation": {1: 0, 12: 180, 24: 270}},
    "Forearm": {"rotation": {1: 90, 12: 90, 24: 0}},
}


def check(name, ok, detail=""):
    CHECKS.append({"check": name, "ok": bool(ok), "detail": str(detail)[:400]})
    print(("ok   " if ok else "FAIL ") + name, flush=True)


def sc(result):
    return getattr(result, "structuredContent", None) or getattr(result, "structured_content", None) or {}


# ----- the independent oracle ---------------------------------------------------------------------------------------


def exact_trig(deg):
    """Exact sine and cosine of a multiple of 90°."""
    table = {0: (0, 1), 90: (1, 0), 180: (0, -1), 270: (-1, 0)}
    return tuple(Q(x) for x in table[int(deg) % 360])


def mat_mul(a, b):
    """Affine (a c e / b d f) products, as 05 §2.1's column-vector convention gives them."""
    a1, b1, c1, d1, e1, f1, z1 = a
    a2, b2, c2, d2, e2, f2, z2 = b
    return (a1 * a2 + c1 * b2, b1 * a2 + d1 * b2, a1 * c2 + c1 * d2, b1 * c2 + d1 * d2, a1 * e2 + c1 * f2 + e1, b1 * e2 + d1 * f2 + f1, z1 + z2)


def local_matrix(p, trig, tan_k, s, cr, cs, one):
    """T(P) · T(cr) · R(θ) · T(−cr) · T(cs) · K(κ) · S(sx, sy) · T(−cs) (05 §2.2)."""
    zero = one - one
    sin, cos = trig
    m = (one, zero, zero, one, p[0], p[1], p[2])
    m = mat_mul(m, (one, zero, zero, one, cr[0], cr[1], zero))
    m = mat_mul(m, (cos, sin, -sin, cos, zero, zero, zero))
    m = mat_mul(m, (one, zero, zero, one, -cr[0], -cr[1], zero))
    m = mat_mul(m, (one, zero, zero, one, cs[0], cs[1], zero))
    m = mat_mul(m, (one, zero, tan_k, one, zero, zero, zero))
    m = mat_mul(m, (s[0], zero, zero, s[1], zero, zero, zero))
    m = mat_mul(m, (one, zero, zero, one, -cs[0], -cs[1], zero))
    return m


def rows(m):
    """The row-major 16 numbers of 05 §2.1."""
    a, b, c, d, e, f, z = m
    return [float(x) for x in (a, c, 0, e, b, d, 0, f, 0, 0, 1, z, 0, 0, 0, 1)]


def linear(table, f):
    """A channel with linear segments, held outside its keys (05 §2.10, §10.3); table values are numbers."""
    frames = sorted(table)
    if f <= frames[0]:
        return table[frames[0]]
    if f >= frames[-1]:
        return table[frames[-1]]
    for a, b in zip(frames, frames[1:]):
        if a <= f <= b:
            t = (Q(f) - a) / (b - a) if isinstance(f, (int, Q)) else (f - a) / (b - a)
            return table[a] + (table[b] - table[a]) * t
    raise AssertionError(f)


def tcb_path(points):
    """Catmull–Rom with reflected end points (05 §9.1, t = c = b = 0), 64-chord arc length (05 §9.2), in floats."""
    p = [tuple(float(x) for x in q) for q in points]
    n = len(p)
    def nb(i):
        if i < 0:
            return tuple(2 * p[0][a] - p[1][a] for a in range(3))
        if i >= n:
            return tuple(2 * p[n - 1][a] - p[n - 2][a] for a in range(3))
        return p[i]
    tangents = [tuple((nb(i + 1)[a] - nb(i - 1)[a]) / 2 for a in range(3)) for i in range(n)]
    def curve(seg, s):
        s2, s3 = s * s, s * s * s
        return tuple((2 * s3 - 3 * s2 + 1) * p[seg][a] + (s3 - 2 * s2 + s) * tangents[seg][a] + (-2 * s3 + 3 * s2) * p[seg + 1][a] + (s3 - s2) * tangents[seg + 1][a] for a in range(3))
    table = [0.0]
    for seg in range(n - 1):
        prev = curve(seg, 0.0)
        for i in range(1, 65):
            q = curve(seg, i / 64)
            table.append(table[-1] + math.sqrt(sum((q[a] - prev[a]) ** 2 for a in range(3))))
            prev = q
    return curve, table


def path_position(pose, f):
    """The animated position at local frame f with linear velocity between the pins (05 §10.5)."""
    frames = sorted(pose)
    if f <= frames[0]:
        return tuple(float(x) for x in pose[frames[0]])
    if f >= frames[-1]:
        return tuple(float(x) for x in pose[frames[-1]])
    if f in frames:
        return tuple(float(x) for x in pose[f])
    curve, table = tcb_path([pose[k] for k in frames])
    for j, (a, b) in enumerate(zip(frames, frames[1:])):
        if a <= f <= b:
            s0, s1 = table[j * 64], table[(j + 1) * 64]
            arc = s0 + (float(f) - a) / (b - a) * (s1 - s0)
            i = next(k for k in range(j * 64, (j + 1) * 64 + 1) if table[k] >= arc)
            if i == j * 64:
                return curve(j, 0.0)
            t = (arc - table[i - 1]) / (table[i] - table[i - 1])
            step = (i - 1 - j * 64 + t) / 64
            return curve(j, step)
    raise AssertionError(f)


def node_local(name, f, exact):
    """M_local of a node at a scene frame (the bars start at frame 1)."""
    st = STATIC[name]
    pose = POSES[name]
    one = Q(1) if exact else 1.0
    num = (lambda x: Q(x)) if exact else (lambda x: float(x))
    if "position" in pose:
        if exact:
            ap = tuple(Q(x) for x in pose["position"][f])
        else:
            ap = path_position(pose["position"], f)
    else:
        ap = (one - one,) * 3
    p = tuple(num(st["offset"][a]) + ap[a] for a in range(3))
    frame = Q(f) if exact else float(f)
    rot = num(st["rotation"]) + (linear({k: num(v) for k, v in pose["rotation"].items()}, frame) if "rotation" in pose else 0)
    if "scale" in pose:
        sx = linear({k: num(v[0]) for k, v in pose["scale"].items()}, frame)
        sy = linear({k: num(v[1]) for k, v in pose["scale"].items()}, frame)
    else:
        sx, sy = one, one
    s = (num(st["scale"][0]) * sx, num(st["scale"][1]) * sy)
    skew = linear({k: num(v) for k, v in pose["skew"].items()}, frame) if "skew" in pose else one - one
    if exact:
        trig = exact_trig(rot)
        if skew != 0:
            raise ValueError("exact poses have no skew")
        tan_k = Q(0)
    else:
        trig = (math.sin(math.radians(rot)), math.cos(math.radians(rot)))
        tan_k = math.tan(math.radians(skew))
    cr = tuple(num(x) for x in st["cr"])
    cs = tuple(num(x) for x in st["cs"])
    return local_matrix(p, trig, tan_k, s, cr, cs, one)


def node_world(name, f, exact):
    m = node_local(name, f, exact)
    parent = PARENTS[name]
    return mat_mul(node_world(parent, f, exact), m) if parent else m


def exact_possible(name, f):
    """Exact arithmetic where every angle in the chain is a multiple of 90° with no skew."""
    if f not in KEY_FRAMES:
        return False
    n = name
    while n:
        pose = POSES[n]
        if "rotation" in pose and pose["rotation"][f] % 90 != 0:
            return False
        if "skew" in pose and pose["skew"][f] != 0:
            return False
        n = PARENTS[n]
    return True


# ----- the scenario -------------------------------------------------------------------------------------------------


def normalised(result, ids):
    """`anim.evaluate` with element IDs replaced by names, as stable bytes."""
    text = json.dumps(result, sort_keys=True, separators=(",", ":"))
    for name, i in ids.items():
        text = text.replace(i, name)
    return text


def session_params(binary, work):
    return StdioServerParameters(
        command=binary,
        args=["--headless", "--mcp-stdio", "--user-dir", str(work / "user"), "--policy", "autonomous"],
    )


async def build(binary, work):
    """Creates the rig; returns the package, the element IDs, the requests made and the evaluation."""
    inputs = []

    async def on_roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    async with stdio_client(session_params(binary, work)) as (read, write):
        async with ClientSession(read, write, list_roots_callback=on_roots) as s:
            await s.initialize()
            await asyncio.sleep(0.5)

            async def cmd(cid, p=None):
                inputs.append({"id": cid, "params": p or {}})
                r = sc(await s.call_tool("run_command", {"id": cid, "params": p or {}}))
                if r.get("status") != "ok":
                    raise RuntimeError(f"{cid} {p}: {r}")
                return r["result"]

            async def done(r):
                job = (r or {}).get("jobId")
                if not job:
                    return r
                w = sc(await s.call_tool("job_wait", {"jobId": job, "timeoutMs": 60000}))
                rec = w.get("result") or w
                return rec.get("record", rec)

            pkg = work / "E4.animpkg"
            await done(await cmd("project.create", {"name": "E4", "path": str(pkg)}))
            ids = {}
            for name in NAMES:
                kind = "peg" if name in ("Master", "ArmPeg") else "drawing"
                created = await cmd("element.create", {"type": kind, "name": name})
                ids[name] = created["elementIds"][0]
                if kind == "peg":
                    await cmd("element.timing.set", {"elementIds": [ids[name]], "duration": 24})
            for name, parent in PARENTS.items():
                if parent:
                    await cmd("element.parent.set", {"elementIds": [ids[name]], "parentId": ids[parent]})
            # Content, so bounds and screen boxes exist: a 2 × 1 field rectangle on each drawing part.
            for name in ("Torso", "Head", "Forearm"):
                d = await cmd("element.drawing.create", {"elementId": ids[name], "name": "1", "exposeFrom": 1, "exposeTo": 24})
                await cmd("draw.rectangle.add", {"drawingId": d["drawingId"], "from": {"x": -100, "y": -50}, "to": {"x": 100, "y": 50}})
            # Static values and pivots.
            for name, st in STATIC.items():
                params = {
                    "nodeIds": [ids[name]],
                    "target": "static",
                    "position": dict(zip("xyz", (float(x) for x in st["offset"]))),
                    "rotation": float(st["rotation"]),
                    "scale": dict(zip("xy", (float(x) for x in st["scale"]))),
                }
                peg = name in ("Master", "ArmPeg")
                if peg:
                    params["pivot"] = dict(zip("xy", (float(x) for x in st["cr"])))
                else:
                    params["rotationPivot"] = dict(zip("xy", (float(x) for x in st["cr"])))
                    params["scalePivot"] = dict(zip("xy", (float(x) for x in st["cs"])))
                await cmd("anim.transform.set", params)
                options = {"applyPivotTo": "both"} if peg else {"rotateAboutPivot": True}
                await cmd("doc.object.patch", {"targets": [ids[name]], "props": {"transform": options}})
            # The three poses.
            for frame in KEY_FRAMES:
                for name, pose in POSES.items():
                    params = {"nodeIds": [ids[name]], "frame": frame}
                    if "rotation" in pose:
                        params["rotation"] = float(pose["rotation"][frame])
                    if "position" in pose:
                        params["position"] = dict(zip("xyz", (float(x) for x in pose["position"][frame])))
                    if "scale" in pose:
                        params["scale"] = dict(zip("xy", (float(x) for x in pose["scale"][frame])))
                    if "skew" in pose:
                        params["skew"] = float(pose["skew"][frame])
                    await cmd("anim.transform.set", params)
            request = {"nodeIds": [ids[n] for n in NAMES], "frames": KEY_FRAMES + BETWEEN}
            first = await cmd("anim.evaluate", request)
            # Undo the last key, read the difference, redo: the same result.
            await cmd("edit.undo", {})
            undone = await cmd("anim.evaluate", request)
            await cmd("edit.redo", {})
            again = await cmd("anim.evaluate", request)
            check("undo changes the pose, redo restores the same bytes", undone != first and again == first)
            await done(await cmd("project.save", {}))
            return pkg, ids, request, first, inputs


async def reopen(binary, work, pkg, request):
    """Opens the saved base in a new process and asks the same question."""

    async def on_roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    async with stdio_client(session_params(binary, work)) as (read, write):
        async with ClientSession(read, write, list_roots_callback=on_roots) as s:
            await s.initialize()
            await asyncio.sleep(0.5)
            r = sc(await s.call_tool("run_command", {"id": "project.open", "params": {"path": str(pkg), "onUnsaved": "discard"}}))
            job = (r.get("result") or {}).get("jobId")
            if job:
                await s.call_tool("job_wait", {"jobId": job, "timeoutMs": 60000})
            r = sc(await s.call_tool("run_command", {"id": "anim.evaluate", "params": request}))
            if r.get("status") != "ok":
                raise RuntimeError(f"evaluate after open: {r}")
            return r["result"]


def compare(evaluated):
    """Every node at every frame against the oracle."""
    worst = 0.0
    compared = 0
    exact = 0
    for frame_result in evaluated["frames"]:
        f = frame_result["frame"]
        f = int(f) if float(f).is_integer() else f
        for name, node in zip(NAMES, frame_result["nodes"]):
            use_exact = exact_possible(name, f)
            want = rows(node_world(name, f, use_exact))
            exact += 1 if use_exact else 0
            got = node["world"]["matrix"]
            for a, b in zip(got, want):
                worst = max(worst, abs(a - b))
                compared += 1
    return worst, compared, exact


async def run(binary, out, work):
    pkg, ids, request, first, inputs = await build(binary, work)
    worst, compared, exact = compare(first)
    check(f"world matrices within {TOLERANCE} of the independent values", worst <= TOLERANCE, f"worst {worst}, {compared} numbers, {exact} node-frames exact")
    check("the comparison covered every node at every frame", compared == len(NAMES) * len(request["frames"]) * 16, compared)
    # The requested angles: a chain of rotations with no mirror or scale reads back as their sum (E4's own wording).
    by_frame = {r["frame"]: dict(zip(NAMES, r["nodes"])) for r in first["frames"]}
    wrong = []
    for f in KEY_FRAMES:
        nodes = by_frame[f]
        want = POSES["Master"]["rotation"][f]
        if abs((nodes["Master"]["world"]["rotation"] - want + 180) % 360 - 180) > TOLERANCE:
            wrong.append(("Master", f, nodes["Master"]["world"]["rotation"], want))
        for name in ("Master", "Torso", "Head", "ArmPeg", "Forearm"):
            if abs(nodes[name]["animated"]["rotation"] - POSES[name]["rotation"][f]) > TOLERANCE:
                wrong.append((name, f, nodes[name]["animated"]["rotation"]))
    check("the requested angles read back", not wrong, wrong)
    # A mirrored ancestor reports a negative sy at the arm (05 §2.5) when nothing else mirrors it.
    arm = by_frame[1]["ArmPeg"]["world"]["scale"]["y"]
    check("the mirror reads back as a negative sy", arm < 0, arm)
    check("positive frames between keys are reported as asked", all(f in by_frame for f in BETWEEN), list(by_frame))
    stable = normalised(first, ids)
    (out / "evaluate.json").write_text(stable, encoding="utf-8", newline="\n")
    # The same base opened twice, in new processes.
    base = work / "base.animpkg"
    shutil.copy(pkg, base)
    one = await reopen(binary, work, base, request)
    two = await reopen(binary, work, base, request)
    a, b = json.dumps(one, sort_keys=True), json.dumps(two, sort_keys=True)
    check("the same requests from the same base answer the same bytes", a == b, hashlib.sha256(a.encode()).hexdigest())
    check("save and open keep the evaluation", normalised(one, ids) == stable)
    (out / "inputs.json").write_text(json.dumps(inputs, indent=1), encoding="utf-8", newline="\n")
    return worst


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    binary, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="tt-e4-"))
    t = time.time()
    worst = None
    try:
        worst = asyncio.run(run(binary, out, work))
    except Exception as e:  # noqa: BLE001 - every failure is a failed check in the report
        check("scenario ran to the end", False, repr(e))
    failed = [c for c in CHECKS if not c["ok"]]
    sha = os.environ.get("GITHUB_SHA")
    if not sha:
        try:
            sha = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            sha = "unknown"
    evaluate = out / "evaluate.json"
    report = {
        "seconds": round(time.time() - t, 3),
        "passed": len(CHECKS) - len(failed),
        "failed": len(failed),
        "checks": CHECKS,
        "worstAbsoluteError": worst,
        "tolerance": TOLERANCE,
        "headSha": sha,
        "platform": platform.platform(),
        "python": platform.python_version(),
        "mcp": importlib.metadata.version("mcp"),
        "evaluateSha256": hashlib.sha256(evaluate.read_bytes()).hexdigest() if evaluate.exists() else None,
    }
    (out / "report.json").write_text(json.dumps(report, indent=1), encoding="utf-8", newline="\n")
    shutil.rmtree(work, ignore_errors=True)
    print(f"{len(CHECKS) - len(failed)}/{len(CHECKS)} checks passed; worst error {worst}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
