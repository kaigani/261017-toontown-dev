"""Writes the C4.3 manual scripts' fixture project (reviews/manual/M4/C4.3-*.md): Bob, a black rectangle at the centre,
and Ann, a black ellipse to the left and behind him (the scene of scripts/c43-camera-e2e.py).

Usage: python scripts/make-c43-manual-fixture.py <toontown-headless binary> [out path]

The default out path is reviews/manual/M4/fixtures/c43-camera.animpkg. The project is built by the headless engine
over MCP stdio with the official SDK, through commands only, so it can be rebuilt from the repository.
"""

from __future__ import annotations

import asyncio
import json
import shutil
import sys
import tempfile
from pathlib import Path

from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

ROOT = Path(__file__).resolve().parents[1]


def sc(r) -> dict:
    return getattr(r, "structuredContent", None) or getattr(r, "structured_content", None) or {}


async def build(headless: str, out: Path, work: Path) -> None:
    async def roots(_ctx):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work"),
                                            types.Root(uri=out.parent.resolve().as_uri(), name="out")])

    params = StdioServerParameters(command=headless, args=["--headless", "--mcp-stdio", "--user-dir", str(work / "user"), "--policy", "autonomous"])
    async with stdio_client(params) as (r, w):
        async with ClientSession(r, w, list_roots_callback=roots) as s:
            await s.initialize()

            async def run(cid: str, p: dict | None = None) -> dict:
                env = sc(await s.call_tool("run_command", {"id": cid, "params": p or {}}))
                if env.get("status") != "ok":
                    raise RuntimeError(f"{cid}: {json.dumps(env.get('error'))}")
                return env.get("result") or {}

            await run("project.create", {"name": "C4.3 camera fixture", "onUnsaved": "fail"})
            found = await run("doc.object.find", {"type": "swatch", "name": "Colour 2"})
            items = found.get("items") or found.get("matches") or []
            black = next((it.get("id") or it.get("key") or it.get("colourId") for it in items if isinstance(it, dict)), None)
            ids = {}
            for name, shape in (("Bob", ("draw.rectangle.add", {"from": {"x": -120, "y": -90}, "to": {"x": 120, "y": 90}})),
                                ("Ann", ("draw.ellipse.add", {"from": {"x": -80, "y": -80}, "to": {"x": 80, "y": 80}}))):
                ids[name] = ((await run("element.create", {"type": "drawing", "name": name})).get("elementIds") or [None])[0]
                await run("element.drawing.create", {"elementId": ids[name], "name": "1", "exposeFrom": 1, "exposeTo": 24})
                await run("app.context.set", {"elementId": ids[name], "frame": 1})
                await run("colour.current.set", {"colourId": black})
                await run(shape[0], shape[1])
                await run("colour.paint.apply", {"points": [{"x": 0, "y": 0}]})
            await run("anim.transform.set", {"nodeIds": [ids["Ann"]], "target": "static", "position": {"x": -1.5, "y": 0.5, "z": -1}})
            await run("app.context.set", {"elementId": ids["Bob"], "frame": 1})
            saved = work / "c43-camera.animpkg"
            env = sc(await s.call_tool("run_command", {"id": "project.saveAs", "params": {"path": str(saved), "onExists": "overwrite"}}))
            job = (env.get("result") or {}).get("jobId")
            if job:
                env = sc(await s.call_tool("job_wait", {"jobId": job, "timeoutMs": 60000}))
            if env.get("status") != "ok":
                raise RuntimeError(f"project.saveAs: {json.dumps(env)[:400]}")
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(saved, out)
            print(json.dumps({"out": str(out), "elements": ids}, indent=1))


def main() -> int:
    if len(sys.argv) not in (2, 3):
        print(__doc__)
        return 2
    out = Path(sys.argv[2]) if len(sys.argv) == 3 else ROOT / "reviews/manual/M4/fixtures/c43-camera.animpkg"
    work = Path(tempfile.mkdtemp(prefix="tt-c43-fixture-"))
    try:
        asyncio.run(build(sys.argv[1], out, work))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
