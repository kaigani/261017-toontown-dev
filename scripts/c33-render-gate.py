#!/usr/bin/env python3
"""M3 C3.3 render gate (ADR-0020; readiness items 4, 9 and 12) through the official MCP Python SDK against the real
`toontown-headless` binary.

Usage: python scripts/c33-render-gate.py <path to toontown-headless> <out dir>

Replays `tests/fixtures/c33/gate-scene.json` on a new project (colours `$swatch:N` resolved through
`doc.object.get palette:#1/swatch:#N`), then:
- renders its outputs and checks each PNG's SHA-256 against `crates/executor/tests/golden/c33/gate-index.json`, which
  the in-process executor recorded (`c33_goldens::gate_scene_images_match_the_gate_index`): the headless binary and the
  library the desktop host links render the same bytes;
- writes the non-horizontal path's `anim.path.get` read-back (passage frames included) with IDs replaced by stable
  names, for the `c33 cross-OS` byte comparison;
- renders a PNG sequence with `render.range.run` and keeps the frames and the manifest (paths relative) for the same
  comparison.
`report.json` keeps the checks, versions and head SHA. Exit status 0 when every check passes. Requires `pip install mcp`.
"""
import asyncio
import hashlib
import importlib.metadata
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

ROOT = Path(__file__).resolve().parents[1]
SCENE = ROOT / "tests" / "fixtures" / "c33" / "gate-scene.json"
INDEX = ROOT / "crates" / "executor" / "tests" / "golden" / "c33" / "gate-index.json"
CHECKS = []


def check(name, ok, detail=""):
    CHECKS.append({"check": name, "ok": bool(ok), "detail": str(detail)[:400]})
    print(("ok   " if ok else "FAIL ") + name, flush=True)


def sc(result):
    return getattr(result, "structuredContent", None) or getattr(result, "structured_content", None) or {}


def stable(value):
    """IDs replaced by names in order of first appearance, so the bytes do not depend on the project's random IDs."""
    names = {}

    def walk(v, key=None):
        if isinstance(v, dict):
            return {k: walk(x, k) for k, x in v.items()}
        if isinstance(v, list):
            return [walk(x, key) for x in v]
        if isinstance(v, str) and key is not None and (key == "id" or key.endswith("Id") or key.endswith("Ids")):
            return names.setdefault(v, f"<id{len(names) + 1}>")
        return v

    return json.dumps(walk(value), sort_keys=True, indent=1) + "\n"


async def run(binary, out, work):
    gate = json.loads(SCENE.read_text(encoding="utf-8"))
    index = json.loads(INDEX.read_text(encoding="utf-8"))

    async def on_roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    params = StdioServerParameters(
        command=binary,
        args=["--headless", "--mcp-stdio", "--user-dir", str(work / "user"), "--policy", "autonomous"],
    )
    async with stdio_client(params) as (read, write):
        async with ClientSession(read, write, list_roots_callback=on_roots) as s:
            await s.initialize()
            await asyncio.sleep(0.5)

            async def cmd(cid, p=None):
                r = sc(await s.call_tool("run_command", {"id": cid, "params": p or {}}))
                if r.get("status") != "ok":
                    raise RuntimeError(f"{cid} {p}: {r}")
                return r["result"]

            async def resolve(v):
                if isinstance(v, str) and v.startswith("$swatch:"):
                    got = await cmd("doc.object.get", {"ref": f"palette:#1/swatch:#{v[len('$swatch:'):]}"})
                    return got["object"]["colourId"]
                if isinstance(v, list):
                    return [await resolve(x) for x in v]
                if isinstance(v, dict):
                    return {k: await resolve(x) for k, x in v.items()}
                return v

            await cmd("project.create", {"name": "Gate", "path": str(work / "Gate.animpkg")})
            for step in gate["steps"]:
                await cmd(step["id"], await resolve(step["params"]))
            for o in gate["outputs"]:
                r = await cmd(o["id"], o["params"])
                data = r.get("data") or r.get("png")
                if data is None:
                    (out / f"{o['name']}.json").write_text(stable(r), encoding="utf-8", newline="\n")
                    passage = [p.get("frame") for p in r.get("points", []) if p.get("kind") == "control"]
                    check(f"{o['name']}: passage frames of the control points", len(passage) == 2 and all(f is not None for f in passage), passage)
                    continue
                import base64

                png = base64.b64decode(data)
                (out / f"{o['name']}.png").write_bytes(png)
                digest = hashlib.sha256(png).hexdigest()
                want = index.get(o["name"], {}).get("sha256")
                check(f"{o['name']}: the headless binary renders the in-process bytes", digest == want, f"{digest} vs {want}")
            # A PNG sequence, delivered with its manifest.
            seq = work / "sequence"
            started = await cmd("render.range.run", {"from": 1, "to": 24, "step": 8, "scale": 0.1, "directory": str(seq), "createDirs": True})
            done = sc(await s.call_tool("job_wait", {"jobId": started["jobId"], "timeoutMs": 120000}))
            record = done.get("result", {}).get("record", done.get("result", {}))
            check("render.range.run succeeded", record.get("state") == "succeeded", record)
            manifest = record.get("result") or {}
            names = sorted(p.name for p in seq.iterdir()) if seq.exists() else []
            check("frames and manifest are published together", names == ["Scene-1_0001-0024.manifest.json", "Scene-1_0001.png", "Scene-1_0009.png", "Scene-1_0017.png"], names)
            (out / "sequence").mkdir(exist_ok=True)
            for f in manifest.get("files", []):
                p = Path(f["path"])
                shutil.copy(p, out / "sequence" / p.name)
                ok = hashlib.sha256(p.read_bytes()).hexdigest() == f["sha256"]
                check(f"sequence {p.name}: manifest SHA-256", ok)
            for key in ("outputPath", "manifestPath"):
                manifest.pop(key, None)
            for f in manifest.get("files", []):
                f["path"] = Path(f["path"]).name
            (out / "sequence" / "manifest.json").write_text(stable(manifest), encoding="utf-8", newline="\n")


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    binary, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="tt-c33-gate-"))
    try:
        asyncio.run(run(binary, out, work))
    except Exception as e:  # noqa: BLE001 - every failure is a failed check in the report
        check("gate ran to the end", False, repr(e))
    failed = [c for c in CHECKS if not c["ok"]]
    sha = os.environ.get("GITHUB_SHA")
    if not sha:
        try:
            sha = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            sha = "unknown"
    report = {
        "passed": len(CHECKS) - len(failed),
        "failed": len(failed),
        "checks": CHECKS,
        "headSha": sha,
        "platform": platform.platform(),
        "python": platform.python_version(),
        "mcp": importlib.metadata.version("mcp"),
    }
    (out / "report.json").write_text(json.dumps(report, indent=1), encoding="utf-8", newline="\n")
    shutil.rmtree(work, ignore_errors=True)
    print(f"{len(CHECKS) - len(failed)}/{len(CHECKS)} checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
