"""Independent real-binary protocol sweep; raw envelopes and latencies are retained, summaries are derived."""
import collections
import hashlib
import importlib.util
import json
import pathlib
import queue
import subprocess
import sys
import threading
import time

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location("regression", ROOT / "tests/adversarial/c14-r1-mcp.py")
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
m.BINARY = str(HERE / "ci-headless-Windows/toontown-headless.exe")
records = []


def save():
    (HERE / "protocol-raw.json").write_text(json.dumps(records, indent=2), encoding="utf-8")


def record(name, result, **more):
    records.append({"probe": name, "result": result, **more})
    save()


c = m.Client()
try:
    c.project()
    for raw in ["{", "[]", "null", '{"jsonrpc":"2.0","id":12,"method":true}',
                '{"jsonrpc":"2.0","id":12,"method":"notifications/initialized"}']:
        c.p.stdin.write((raw + "\n").encode())
        c.p.stdin.flush()
        response = c.q.get(timeout=5)
        record("malformed frame", response, input=raw, ping=c.request("ping", {}))
    for method, params in [
        ("no/method", {}), ("tools/call", {"name": 12}), ("tools/list", []),
        ("initialize", {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "second"}}),
        ("logging/setLevel", {"level": "nonsense"}),
        ("completion/complete", {"ref": {"type": "ref/prompt", "name": "nope"}, "argument": {"name": "commandId", "value": "app"}}),
        ("resources/subscribe", {"uri": "app://nope"}),
        ("resources/subscribe", {"uri": "app://object/{id}"}),
        ("resources/unsubscribe", {"uri": "app://object/{id}"}),
        ("resources/unsubscribe", {"uri": "app://object/{id}"}),
    ]:
        record(method, c.request(method, params), input=params, ping=c.request("ping", {}))
    # Frame limit is tested on a whole line followed by a usable stream.
    c.p.stdin.write(b' ' + b'x' * (50 * 1024 * 1024 + 1) + b'\n')
    c.p.stdin.flush()
    record("oversized-frame", c.q.get(timeout=10), ping=c.request("ping", {}))
    # Schema discovery independently drives every registered executable command and malformed params.
    listed = c.call("list_commands", {"page": {"limit": 1000}})["result"]["structuredContent"]["result"]["commands"]
    executable = [x for x in listed if x["kind"] != "event"]
    executable.sort(key=lambda x: (x["id"] in {"project.close", "project.open", "app.quit", "project.recovery.open", "project.backup.open"}, x["id"]))
    for command in executable:
        cid = command["id"]
        d = c.call("describe_command", {"id": cid})["result"]["structuredContent"]["result"]
        for variant, params in [("example", d["examples"][0].get("params", {})), ("malformed", {"reviewUnknown": True})]:
            start = time.monotonic()
            r = c.run(cid, params)
            records.append({"probe": "command", "id": cid, "variant": variant, "params": params,
                            "elapsedMs": (time.monotonic() - start) * 1000, "result": r})
    save()
    # Every JSON progress-token type, with a completed job which still sends final progress.
    if c.p.poll() is None:
        job_env = c.run("project.open", {"path": str(c.root / "P.animpkg")})
        job = job_env.get("result", {}).get("jobId")
        for token in [None, False, 1, 1.5, "token", [], {}]:
            result = c.request("tools/call", {"name": "job_wait", "arguments": {"jobId": job, "timeoutMs": 1000}, "_meta": {"progressToken": token}})
            record("progress-token", result, token=token, notifications=c.saved[:])
            c.saved.clear()
    # 10,000 rapid requests while the reader continuously drains stdout.
    start = time.monotonic()
    def flood():
        for n in range(10000):
            c.send({"jsonrpc": "2.0", "id": f"f{n}", "method": "tools/call", "params": {"name": "get_state", "arguments": {}}})
    sender = threading.Thread(target=flood, daemon=True)
    sender.start()
    responses = []
    deadline = time.monotonic() + 90
    while len(responses) < 10000 and time.monotonic() < deadline:
        msg = c.q.get(timeout=10)
        if isinstance(msg.get("id"), str) and msg["id"].startswith("f"):
            responses.append({"id": msg["id"], "error": msg.get("error"), "status": msg.get("result", {}).get("structuredContent", {}).get("status"), "elapsedMs": (time.monotonic() - start) * 1000})
    sender.join(timeout=5)
    (HERE / "flood-raw.json").write_text(json.dumps(responses), encoding="utf-8")
    record("flood", {"responses": len(responses), "uniqueIds": len({x['id'] for x in responses}),
                     "errors": sum(x["error"] is not None for x in responses), "elapsedMs": (time.monotonic() - start) * 1000}, ping=c.request("ping", {}))
finally:
    c.close()

# Roots returning structurally incorrect data must leave the server responsive.
c = m.Client({"roots": {"listChanged": True}})
try:
    request = c.q.get(timeout=5)
    c.send({"jsonrpc": "2.0", "id": request["id"], "result": {"roots": [None, 1, {}, {"uri": 2}, {"uri": "https://example.com"}]}})
    record("garbage roots", c.request("ping", {}))
finally:
    c.close()

# A valid filter occupies the single call worker while inline ping still answers.
c = m.Client()
try:
    c.send({"jsonrpc": "2.0", "id": "slow", "method": "tools/call", "params": {"name": "list_commands", "arguments": {"filter": "*" * 32 + "Z"}}})
    time.sleep(.1)
    record("filter-ping", c.request("ping", {}))
    try:
        c.call("get_state", {})
        record("filter-queued-call", "answered")
    except queue.Empty:
        record("filter-queued-call", "no response within 5 seconds; worker still occupied")
finally:
    c.close()

print("raw records", len(records))
