#!/usr/bin/env python3
"""End-to-end scenario E2 "paint by id" (11 §14.8; M2 C2.4 readiness item 3) through the official MCP Python SDK
against the real `toontown-headless` binary.

Usage: python scripts/e2-paint-by-id.py <path to toontown-headless> <out dir>

Chain: new project → vectorize the line art (job, progress) → `doc.validate` lists the unpainted zones → paint every
zone by ID (one revision each) → validation clean → recolour → `render_drawing` (generic tool) twice with identical
hashes and known pixels → a stale zone ID and a stale revision are refused → undo / redo → save, reopen, identical
`doc.export.json` → the agent activity exported as a batch and replayed with `toontown-headless --run` from the saved
base, reproducing the same document. Seven PNGs and their SHA-256 hashes, and `report.json`, are written to the out
dir. Exit status 0 when every check passes. Requires `pip install mcp`.
"""
import asyncio
import base64
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "tests" / "fixtures" / "vectorize" / "e2-lineart.png"
CHECKS = []


def check(name, ok, detail=""):
    CHECKS.append({"check": name, "ok": bool(ok), "detail": str(detail)[:400]})
    print(("ok   " if ok else "FAIL ") + name, flush=True)


def sc(result):
    return getattr(result, "structuredContent", None) or getattr(result, "structured_content", None) or {}


def png_pixels(png):
    """RGBA rows of an 8-bit RGBA, non-interlaced PNG (what the renderer writes)."""
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, width, height = 8, b"", 0, 0
    while pos < len(png):
        n = int.from_bytes(png[pos:pos + 4], "big")
        kind, data = png[pos + 4:pos + 8], png[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            width, height = int.from_bytes(data[:4], "big"), int.from_bytes(data[4:8], "big")
            assert data[8] == 8 and data[9] == 6, "8-bit RGBA"
        elif kind == b"IDAT":
            idat += data
        pos += 12 + n
    raw, stride, rows, prev = zlib.decompress(idat), width * 4, [], bytearray(width * 4)
    for y in range(height):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        prev = line
    return width, height, rows


def content(export_result):
    """A `doc.export.json` bundle without its export time."""
    b = dict((export_result or {}).get("bundle") or {})
    b.pop("exportedAt", None)
    return json.dumps(b, sort_keys=True)


def pixel(png, x, y):
    _, _, rows = png_pixels(png)
    return tuple(rows[y][x * 4:x * 4 + 4])


async def run(binary, out, work):
    notes = []

    async def on_message(msg):
        notes.append(msg)

    async def on_roots(context):
        return types.ListRootsResult(roots=[types.Root(uri=work.resolve().as_uri(), name="work")])

    params = StdioServerParameters(command=binary, args=["--headless", "--mcp-stdio", "--user-dir", str(work / "user"), "--policy", "autonomous"])
    hashes = {}

    async with stdio_client(params) as (read, write):
        async with ClientSession(read, write, message_handler=on_message, list_roots_callback=on_roots) as s:
            await s.initialize()
            await asyncio.sleep(0.5)

            async def cmd(cid, p=None, **extra):
                args = {"id": cid, "params": p or {}}
                args.update(extra)
                return sc(await s.call_tool("run_command", args))

            async def done(r):
                """Waits for a job command's job; the final record (or the envelope of a command that is no job)."""
                job = (r.get("result") or {}).get("jobId")
                if not job or job == "dryRun":
                    return r
                w = sc(await s.call_tool("job_wait", {"jobId": job, "timeoutMs": 60000}))
                rec = w.get("result") or w
                return rec.get("record", rec)

            def keep(name, png):
                (out / f"{name}.png").write_bytes(png)
                hashes[name] = hashlib.sha256(png).hexdigest()

            pkg = work / "E2.animpkg"
            r = await cmd("project.create", {"name": "E2", "path": str(pkg)})
            check("project.create", r.get("status") == "ok", r)
            shutil.copy(ART, work / "lineart.png")
            lineart = str(work / "lineart.png")

            r = await cmd("draw.vectorize.preview", {"file": lineart, "width": 256})
            check("vectorize preview", r.get("status") == "ok" and r["result"]["strokeCount"] == 3, r.get("result", {}).get("strokeCount"))
            keep("1-preview", base64.b64decode(r["result"]["png"]))

            # A new project's current element is the camera peg; the line art goes to its drawing element.
            r = await cmd("draw.vectorize.run", {"files": [lineart], "targetElementId": 'scene:#1/element:"Drawing"', "startFrame": 1})
            check("vectorize run started", r.get("status") == "ok", r)
            job = (r.get("result") or {}).get("jobId")
            progress = []

            async def on_progress(p, total, message):
                progress.append((p, total, message))

            w = sc(await s.call_tool("job_wait", {"jobId": job, "timeoutMs": 60000}, progress_callback=on_progress))
            rec = w.get("result") or w
            rec = rec.get("record", rec)
            check("vectorize job succeeded", rec.get("state") == "succeeded", rec)
            drawing = rec["result"]["drawingIds"][0]
            check("job progress", bool(progress) or rec.get("progress") == 1.0, progress[:2])

            region = {"x": -600, "y": -450, "width": 1200, "height": 900}

            async def render(name=None):
                r = await cmd("draw.drawing.render", {"drawingId": drawing, "width": 256, "region": region, "background": "#FFFFFFFF"})
                png = base64.b64decode(r["result"]["png"])
                if name:
                    keep(name, png)
                return png

            png = await render("2-vectorized")
            check("known pixel: the frame line is ink", pixel(png, 18, 96)[:3] == (0, 0, 0), pixel(png, 18, 96))
            check("known pixel: inside the ring is paper", pixel(png, 70, 96)[:3] == (255, 255, 255), pixel(png, 70, 96))

            # Base for the replay: the project as saved now.
            r = await done(await cmd("project.save", {}))
            check("save the base", r.get("state") == "succeeded" or r.get("status") == "ok", r)
            base = work / "base.animpkg"
            shutil.copy(pkg, base)
            r = await cmd("agent.activity.export", {"path": str(work / "activity-base.json"), "format": "batch", "onExists": "overwrite"})
            base_steps = len(json.loads((work / "activity-base.json").read_text(encoding="utf-8"))["steps"])

            v = await cmd("doc.validate", {"checks": ["unpaintedZones"]})
            zones = [i["details"]["zoneId"] for i in v["result"]["issues"] if i["details"].get("drawingId") == drawing]
            check("doc.validate reports the unpainted zones", len(zones) >= 4, len(zones))

            export = await cmd("doc.export.json", {"scope": "project", "binaries": "omit"})
            swatches = {}
            for pal in export["result"]["bundle"]["palettes"]:
                for sw in pal["swatches"]:
                    val = next(iter(sw.get("values", {}).values()), {})
                    if isinstance(val, dict) and "rgba" in val:
                        swatches.setdefault(val["rgba"], sw["colourId"])
            red, green = swatches["#FF0000FF"], swatches["#00FF00FF"]

            hit = await cmd("draw.hitTest.get", {"drawingId": drawing, "point": {"x": (70 - 128) * 1200 / 256, "y": 0}})
            ring = next((h["objectId"] for h in hit["result"]["hits"] if str(h.get("objectId", "")).startswith("zn_")), None)
            check("hit test finds the ring's inside zone", ring in zones, (ring, zones))

            revs_ok = True
            for z in zones:
                before = (await cmd("doc.state.get"))["result"]["documentRev"]
                r = await cmd("colour.paint.apply", {"drawingId": drawing, "objectIds": [z], "colourId": red})
                after = r.get("documentRev")
                revs_ok &= r.get("status") == "ok" and after == before + 1
            check("each zone painted by ID in one revision", revs_ok, "")
            v = await cmd("doc.validate", {"checks": ["unpaintedZones"]})
            left = [i for i in v["result"]["issues"] if i["details"].get("drawingId") == drawing]
            check("validation clean after painting", not left, left[:2])

            png = await render("3-painted")
            check("known pixel: the ring's inside is red", pixel(png, 70, 96)[:3] == (255, 0, 0), pixel(png, 70, 96))
            generic = [sc(await s.call_tool("render_drawing", {"drawing": drawing, "width": 256})) for _ in range(2)]
            h = [hashlib.sha256(base64.b64decode(g["result"]["png"])).hexdigest() for g in generic]
            check("render_drawing (generic tool) twice: identical hashes", h[0] == h[1], h)

            r = await cmd("colour.recolour.apply", {"fromColourId": red, "toColourId": green})
            check("recolour", r.get("status") == "ok", r)
            png = await render("4-recoloured")
            check("known pixel: recoloured green", pixel(png, 70, 96)[:3] == (0, 255, 0), pixel(png, 70, 96))

            r = await cmd("colour.paint.apply", {"drawingId": drawing, "objectIds": ["zn_00000000000000000000000000"], "colourId": red})
            check("a stale zone ID is refused", r.get("error", {}).get("code") == "colour.staleZoneId", r.get("error"))
            r = await cmd("colour.paint.apply", {"drawingId": drawing, "objectIds": [ring], "colourId": red}, expectRev={drawing: 1})
            check("a stale revision is refused", r.get("error", {}).get("code") == "rev.conflict", r.get("error"))

            r = sc(await s.call_tool("undo", {}))
            png = await render("5-undone")
            check("undo restores red", pixel(png, 70, 96)[:3] == (255, 0, 0), pixel(png, 70, 96))
            r = sc(await s.call_tool("redo", {}))
            png_redo = await render("6-redone")
            check("redo restores green", pixel(png_redo, 70, 96)[:3] == (0, 255, 0), pixel(png_redo, 70, 96))

            r = await done(await cmd("project.save", {}))
            check("save", r.get("state") == "succeeded" or r.get("status") == "ok", r)
            live = await cmd("doc.export.json", {"scope": "project", "binaries": "omit"})
            r = await cmd("agent.activity.export", {"path": str(work / "activity.json"), "format": "batch", "onExists": "overwrite"})
            r = await done(await cmd("project.open", {"path": str(pkg), "onUnsaved": "discard"}))
            check("reopen", r.get("state") == "succeeded" or r.get("status") == "ok", r)
            reopened = await cmd("doc.export.json", {"scope": "project", "binaries": "omit"})
            check("save/open: identical doc.export.json", content(live["result"]) == content(reopened["result"]), "")
            png = await render("7-reopened")
            check("reopened render equals the redone render", hashlib.sha256(png).hexdigest() == hashlib.sha256(png_redo).hexdigest(), "")

    # Replay: the edits after the base, exported as a batch, from the identical saved base.
    steps = json.loads((work / "activity.json").read_text(encoding="utf-8"))["steps"][base_steps:]
    steps = [st for st in steps if not str(st.get("id", "")).startswith(("agent.", "project."))]
    # The live document was saved before its export, and a save writes the drawings' colour-recovery copies (03 §11.1)
    # as a revision: the replay saves too, beside the base, then exports.
    steps.append({"id": "project.saveAs", "params": {"path": str(work / "replayed.animpkg"), "onExists": "overwrite"}})
    steps.append({"id": "doc.export.json", "params": {"scope": "project", "binaries": "omit"}})
    batch = work / "replay.json"
    batch.write_text(json.dumps({"label": "E2 replay", "atomic": False, "stopOnError": True, "steps": steps}), encoding="utf-8")
    log = work / "replay.jsonl"
    p = subprocess.run([str(Path(binary).resolve()), "--headless", "--user-dir", str(work / "user2"), "--project", str(base), "--run", str(batch), "--out", str(log)], capture_output=True, text=True, timeout=300)
    check("--run replay exits 0", p.returncode == 0, p.stderr[-400:])
    replayed = None
    for line in log.read_text(encoding="utf-8").splitlines():
        e = json.loads(line)
        # The last step is the export appended above.
        if e.get("record") == "step" and e.get("index") == len(steps) - 1:
            replayed = (e.get("envelope") or {}).get("result")
    (out / "live-export.json").write_text(content(live["result"]), encoding="utf-8")
    (out / "replayed-export.json").write_text(content(replayed), encoding="utf-8")
    check("replay from the saved base reproduces the document", replayed is not None and content(live["result"]) == content(replayed), "" if replayed else log.read_text(encoding="utf-8")[-400:])
    (out / "hashes.json").write_text(json.dumps(hashes, indent=2) + "\n", encoding="utf-8")


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    binary, out = sys.argv[1], Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="tt-e2-"))
    t = time.time()
    try:
        asyncio.run(run(binary, out, work))
    except Exception as e:  # noqa: BLE001 - every failure is a failed check in the report
        check("scenario ran to the end", False, repr(e))
    failed = [c for c in CHECKS if not c["ok"]]
    (out / "report.json").write_text(json.dumps({"seconds": round(time.time() - t, 3), "passed": len(CHECKS) - len(failed), "failed": len(failed), "checks": CHECKS}, indent=2) + "\n", encoding="utf-8")
    shutil.rmtree(work, ignore_errors=True)
    print(f"{len(CHECKS) - len(failed)}/{len(CHECKS)} checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
