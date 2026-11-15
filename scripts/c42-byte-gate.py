"""M4 C4.2b part G: the attached-versus-headless byte gate (M3 merge note §8 item 2 and §3 readiness item 12; 11 §14
item 5; ADR-0006 decision 3).

Usage: python scripts/c42-byte-gate.py <toontown binary> <toontown-headless binary> <out dir>

The same saved fixture (the committed E1 project, one copy per process so neither locks the other's file), opened at
the same document revision, renders the same integer frames through the running application (attached mode, the MCP
SDK over Streamable HTTP) and through the headless binary (the MCP SDK over stdio). For every frame the PNG bytes must
be identical and the result's metadata identical except the named volatile fields (`VOLATILE`: how long the render
took and the job that ran it). Receipts in <out dir>: `attached/` and `headless/` PNGs (the retained goldens of this
run), `manifest.json` (kind `toontown.c42.byte-gate`: hashes, revisions, the volatile fields and every difference).
Exit 0 when the gate holds.
"""

from __future__ import annotations

import asyncio
import base64
import hashlib
import importlib.util
import json
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("c41e2e", ROOT / "scripts" / "c41-gui-e2e.py")
e2e = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(e2e)

from mcp import ClientSession, StdioServerParameters, types  # noqa: E402
from mcp.client.stdio import stdio_client  # noqa: E402

FIXTURE = ROOT / "reviews/evidence/M3-C3.4-r1/ci-Windows/projects/E1-replay.animpkg"
FRAMES = [1, 6, 12, 18, 24]
#: Result fields that legitimately differ between two runs: how long the render took, the job that ran it.
VOLATILE = {"renderMs", "jobId"}
OPEN_BOUND_S = 60.0


def sc(r) -> dict:
    return getattr(r, "structuredContent", None) or getattr(r, "structured_content", None) or {}


async def render_all(call) -> dict:
    """Opens nothing: renders FRAMES through `call(command, params) -> envelope` and returns per-frame results."""
    out = {}
    for f in FRAMES:
        env = await call("render.frame.run", {"frame": f})
        out[f] = env
    return out


async def open_project(call, path: Path) -> dict:
    env = await call("project.open", {"path": str(path), "onUnsaved": "fail"})
    if env.get("status") != "ok":
        raise RuntimeError(f"project.open: {json.dumps(env)[:600]}")
    start = time.monotonic()
    while time.monotonic() - start < OPEN_BOUND_S:
        st = await call("doc.state.get", {})
        if st.get("status") == "ok":
            return st
        await asyncio.sleep(0.25)
    raise RuntimeError("the project did not open within the bound")


async def headless_side(binary: str, work: Path, fixture: Path) -> tuple[dict, dict]:
    async def roots(_ctx):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    params = StdioServerParameters(command=binary, args=["--headless", "--mcp-stdio", "--user-dir", str(work / "user-h"), "--policy", "autonomous"])
    async with stdio_client(params) as (read, write):
        async with ClientSession(read, write, list_roots_callback=roots) as s:
            await s.initialize()

            async def call(cid, p):
                return sc(await s.call_tool("run_command", {"id": cid, "params": p}))

            state = await open_project(call, fixture)
            return state, await render_all(call)


async def attached_side(app: str, work: Path, fixture: Path, run) -> tuple[dict, dict]:
    user = work / "user-a"
    (user / "Settings").mkdir(parents=True)
    seed = {"agent.policy.defaults": {"policy": "autonomous", "maxJobs": 2, "maxBatchSteps": 1000, "maxInlineImagePx": 2048,
                                      "maxInlineBytes": 52428800, "askInApp": True, "letAgentMoveView": True, "highlightAgentChanges": True}}
    (user / "Settings" / "preferences.json").write_text(json.dumps({"format": "toontown-settings", "kind": "preferences", "formatVersion": "1.0", "data": seed}), encoding="utf-8")
    discovery = work / "discovery.json"
    proc = e2e.launch(app, user, discovery)
    try:
        d = e2e.wait_discovery(discovery, proc)
        client = e2e.httpx2.AsyncClient(headers={"Authorization": f"Bearer {d['token']}"}, timeout=e2e.httpx2.Timeout(60.0, read=300.0))

        async def roots(_ctx):
            return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

        async with client:
            async with e2e.streamable_http_client(d["endpoint"], http_client=client) as streams:
                async with ClientSession(streams[0], streams[1], list_roots_callback=roots) as s:
                    await s.initialize()
                    await e2e.gui_tree(s, run, lambda r: e2e.by_id(r, "startScreen") is not None, "the attached GUI is up")

                    async def call(cid, p):
                        return sc(await s.call_tool("run_command", {"id": cid, "params": p}))

                    state = await open_project(call, fixture)
                    return state, await render_all(call)
    finally:
        e2e.stop(proc)


def png_of(env: dict) -> bytes | None:
    r = env.get("result") or {}
    data = r.get("png") or r.get("data")
    return base64.b64decode(data) if data else None


def metadata(env: dict) -> dict:
    r = dict(env.get("result") or {})
    r.pop("png", None)
    r.pop("data", None)
    return r


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    app, headless, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    run = e2e.Run(out)
    work = Path(tempfile.mkdtemp(prefix="tt-c42-gate-"))
    manifest: dict = {"kind": "toontown.c42.byte-gate", "platform": sys.platform, "fixture": FIXTURE.relative_to(ROOT).as_posix(),
                      "fixtureSha256": hashlib.sha256(FIXTURE.read_bytes()).hexdigest(), "frames": FRAMES,
                      "volatile": sorted(VOLATILE)}
    try:
        fh, fa = work / "fixture-headless.animpkg", work / "fixture-attached.animpkg"
        shutil.copyfile(FIXTURE, fh)
        shutil.copyfile(FIXTURE, fa)
        h_state, h = asyncio.run(headless_side(headless, work, fh))
        a_state, a = asyncio.run(attached_side(app, work, fa, run))
        rev_h, rev_a = h_state.get("documentRev"), a_state.get("documentRev")
        manifest["documentRev"] = {"headless": rev_h, "attached": rev_a}
        run.check(rev_h is not None and rev_h == rev_a, "the same document revision on both sides", manifest["documentRev"])
        frames = {}
        for f in FRAMES:
            ph, pa = png_of(h[f]), png_of(a[f])
            mh, ma = metadata(h[f]), metadata(a[f])
            diff = sorted(k for k in set(mh) | set(ma) if mh.get(k) != ma.get(k))
            unexpected = [k for k in diff if k not in VOLATILE]
            for side, png in (("headless", ph), ("attached", pa)):
                if png:
                    (out / side).mkdir(exist_ok=True)
                    (out / side / f"frame-{f:02d}.png").write_bytes(png)
            frames[f] = {
                "headless": hashlib.sha256(ph).hexdigest() if ph else None,
                "attached": hashlib.sha256(pa).hexdigest() if pa else None,
                "metadataDiff": diff,
                "status": [h[f].get("status"), a[f].get("status")],
            }
            run.check(ph is not None and ph == pa, f"frame {f}: identical PNG bytes attached and headless", frames[f])
            run.check(not unexpected, f"frame {f}: identical metadata except the volatile fields", {"unexpected": unexpected, "headless": mh, "attached": ma} if unexpected else diff)
        manifest["frames"] = frames
    except Exception as ex:  # noqa: BLE001 - reported as a failure with its text
        run.check(False, "the gate ran", repr(ex))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    manifest["failures"] = run.failures
    run.save("transcript.json", run.transcript)
    run.save("manifest.json", manifest)
    print(json.dumps({k: manifest.get(k) for k in ("kind", "documentRev", "failures")}, indent=1))
    return 1 if run.failures else 0


if __name__ == "__main__":
    sys.exit(main())
