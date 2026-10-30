#!/usr/bin/env python3
"""MCP protocol conformance of `toontown-headless --mcp-stdio` against the official Python SDK client (11 §14.7).

Usage: python scripts/mcp-conformance.py <path to toontown-headless> [report.json]

The SDK client launches the binary over stdio and exercises: initialize, ping, tools/list (and the SDK's own check of
structuredContent against each tool's outputSchema), tools/call, resources/list, resources/templates/list,
resources/read, resources/subscribe with an update notification, prompts/list, prompts/get, completion/complete,
logging/setLevel with log notifications, progress notifications, elicitation (the confirmation protocol) and roots.
Exit status 0 when every check passes. Requires `pip install mcp`.
"""
import asyncio
import json
import os
import re
import shutil
import sys
import tempfile
import time
from pathlib import Path

from mcp import ClientSession, StdioServerParameters, types
from mcp.client.stdio import stdio_client

CHECKS = []


def check(name, ok, detail=""):
    CHECKS.append({"check": name, "ok": bool(ok), "detail": str(detail)[:400]})


def g(o, camel):
    """An attribute by its protocol (camelCase, SDK 1.x) or Python (snake_case, SDK 2.x) name."""
    if hasattr(o, camel):
        return getattr(o, camel)
    snake = re.sub(r"(?<!^)(?=[A-Z])", "_", camel).lower()
    return getattr(o, snake)


def sc(result):
    """The structured content of a tool result."""
    return getattr(result, "structuredContent", None) or getattr(result, "structured_content", None) or {}


async def run(binary, workdir):
    notes = []
    elicited = []
    roots_asked = []

    async def on_message(msg):
        notes.append(msg)

    async def on_log(params):
        notes.append(("log", params))

    async def on_elicit(context, params):
        elicited.append(params)
        return types.ElicitResult(action="accept", content={"confirm": True})

    async def on_roots(context):
        roots_asked.append(True)
        uri = Path(workdir).resolve().as_uri()
        return types.ListRootsResult(roots=[types.Root(uri=uri, name="work")])

    params = StdioServerParameters(
        command=binary,
        args=["--headless", "--mcp-stdio", "--user-dir", str(Path(workdir) / "user")],
    )
    async with stdio_client(params) as (read, write):
        async with ClientSession(
            read,
            write,
            elicitation_callback=on_elicit,
            list_roots_callback=on_roots,
            logging_callback=on_log,
            message_handler=on_message,
        ) as s:
            init = await s.initialize()
            check("initialize.protocolVersion", g(init, "protocolVersion") in ("2025-06-18", "2025-11-25"), g(init, "protocolVersion"))
            caps = init.capabilities
            check("initialize.capabilities", caps.tools and caps.resources and caps.prompts and caps.logging is not None, caps)
            check("initialize.instructions", bool(g(init, "instructions")) and "agent guide" in g(init, "instructions").lower(), "")
            await s.send_ping()
            check("ping", True)
            await asyncio.sleep(0.5)
            check("roots/list asked after initialized", bool(roots_asked), roots_asked)

            tools = (await s.list_tools()).tools
            names = {t.name for t in tools}
            check("tools/list 28 generic tools", len(tools) == 28, sorted(names))
            check("tools carry outputSchema", all(g(t, "outputSchema") for t in tools), "")

            pkg = str(Path(workdir) / "Conformance.animpkg")
            r = await s.call_tool("run_command", {"id": "project.create", "params": {"name": "Conformance", "path": pkg}})
            env = sc(r)
            check("tools/call run_command project.create (inside client roots)", env.get("status") == "ok", env)
            scene = (env.get("result") or {}).get("sceneId", "")
            r = await s.call_tool("get_state", {})
            check("tools/call get_state", sc(r).get("result", {}).get("project", {}).get("name") == "Conformance", sc(r))
            r = await s.call_tool("get_object", {"ref": scene})
            check("tools/call get_object", sc(r).get("status") == "ok", sc(r))

            res = (await s.list_resources()).resources
            check("resources/list", len(res) == 8, [str(x.uri) for x in res])
            tpl = g(await s.list_resource_templates(), "resourceTemplates")
            check("resources/templates/list", len(tpl) == 10, len(tpl))
            guide = await s.read_resource("app://docs/agent-guide")
            check("resources/read docs equals instructions", guide.contents[0].text == g(init, "instructions"), "")
            obj = await s.read_resource(f"app://object/{scene}")
            check("resources/read object", scene in obj.contents[0].text, "")

            await s.subscribe_resource(f"app://object/{scene}")
            await s.set_logging_level("info")
            await s.call_tool("set_properties", {"targets": [scene], "props": {"meta": {"conformance": 1}}})
            await asyncio.sleep(0.6)
            updated = [n for n in notes if "ResourceUpdated" in type(getattr(n, "root", n)).__name__ or "resources/updated" in repr(n)]
            check("resources/subscribe -> notifications/resources/updated", bool(updated), len(notes))
            logs = [n for n in notes if isinstance(n, tuple) and n[0] == "log"]
            check("logging/setLevel -> notifications/message", bool(logs), len(logs))

            prompts = (await s.list_prompts()).prompts
            check("prompts/list", len(prompts) == 10, [p.name for p in prompts])
            p = await s.get_prompt("orient", {})
            check("prompts/get", bool(p.messages), "")
            comp = await s.complete(types.PromptReference(type="ref/prompt", name="orient"), {"name": "commandId", "value": "doc.object.g"})
            check("completion/complete", "doc.object.get" in comp.completion.values, comp.completion.values)

            # Confirmation through elicitation: overwriting an existing file under Assist.
            target = Path(workdir) / "Existing.animpkg"
            target.write_bytes(b"existing")
            r = await s.call_tool("run_command", {"id": "project.saveAs", "params": {"path": str(target), "onExists": "overwrite"}})
            check("elicitation/create for a confirmation", bool(elicited), elicited[:1])
            check("accepted elicitation proceeds", sc(r).get("status") == "ok", sc(r))

            # Progress on a job wait.
            r = await s.call_tool("run_command", {"id": "doc.package.export", "params": {"path": str(Path(workdir) / "Export.animpkg")}})
            job = (sc(r).get("result") or {}).get("jobId")
            progress = []

            async def on_progress(p, total, message):
                progress.append((p, total, message))

            r = await s.call_tool("job_wait", {"jobId": job, "timeoutMs": 30000}, progress_callback=on_progress)
            check("job_wait ok", sc(r).get("status") == "ok", sc(r))
            check("notifications/progress", bool(progress), progress)

            r = await s.call_tool("pointer", {})
            check("pointer answers notImplemented with its milestone", sc(r).get("error", {}).get("details", {}).get("milestone") == "M2 C2.3", sc(r))


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    binary = sys.argv[1]
    report_path = sys.argv[2] if len(sys.argv) > 2 else None
    workdir = tempfile.mkdtemp(prefix="tt-c14-sdk-")
    started = time.time()
    error = None
    try:
        asyncio.run(asyncio.wait_for(run(binary, workdir), timeout=180))
    except Exception as e:  # noqa: BLE001 - reported as a failed check
        error = f"{type(e).__name__}: {e}"
        check("session completed", False, error)
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
    import importlib.metadata as md

    report = {
        "client": f"mcp (Python SDK) {md.version('mcp')}",
        "server": binary,
        "platform": sys.platform,
        "seconds": round(time.time() - started, 2),
        "passed": sum(c["ok"] for c in CHECKS),
        "failed": sum(not c["ok"] for c in CHECKS),
        "checks": CHECKS,
    }
    text = json.dumps(report, indent=2)
    print(text)
    if report_path:
        os.makedirs(os.path.dirname(report_path) or ".", exist_ok=True)
        Path(report_path).write_text(text + "\n", encoding="utf-8")
    return 0 if report["failed"] == 0 and CHECKS else 1


if __name__ == "__main__":
    sys.exit(main())
