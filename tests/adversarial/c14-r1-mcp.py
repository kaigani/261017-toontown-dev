"""M1-C1.4 round-1 protocol regressions. cargo test -p tt-headless --test c14_r1 runs the real binary.

Standalone reviewer-suite discovery runs the release-feature check; runtime cases need TT_REVIEW_BINARY.
Every subprocess is isolated, every wait is bounded, and teardown kills a stalled server.
"""
import json
import os
import pathlib
import queue
import subprocess
import tempfile
import threading
import time
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
BINARY = os.environ.get("TT_REVIEW_BINARY")


class Client:
    def __init__(self, caps=None, extra=()):
        self.tmp = tempfile.TemporaryDirectory(prefix="tt-c14-r1-")
        self.root = pathlib.Path(self.tmp.name)
        self.p = subprocess.Popen([BINARY, "--mcp-stdio", "--user-dir", str(self.root / "user"),
                                   "--allow-path", str(self.root), *extra],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.q = queue.Queue()
        self.saved = []
        self.n = 100
        def reader():
            for line in self.p.stdout:
                self.q.put(json.loads(line))
        self.reader = threading.Thread(target=reader, daemon=True)
        self.reader.start()
        self.init = self.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": caps or {},
                                                "clientInfo": {"name": "reviewer", "version": "1"}})
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, msg):
        self.p.stdin.write((json.dumps(msg, ensure_ascii=True) + "\n").encode())
        self.p.stdin.flush()

    def request(self, method, params, timeout=30):
        self.n += 1
        rid = self.n
        self.send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        return self.wait(rid, timeout)

    def wait(self, rid, timeout=5):
        deadline = time.monotonic() + timeout
        while True:
            for i, msg in enumerate(self.saved):
                if msg.get("id") == rid and "method" not in msg:
                    return self.saved.pop(i)
            msg = self.q.get(timeout=max(.001, deadline - time.monotonic()))
            self.saved.append(msg)
            if time.monotonic() >= deadline:
                raise TimeoutError(f"no response for {rid}")

    def call(self, name, args):
        return self.request("tools/call", {"name": name, "arguments": args})

    def run(self, command, params):
        return self.call("run_command", {"id": command, "params": params})["result"]["structuredContent"]

    def project(self):
        e = self.run("project.create", {"name": "P", "path": str(self.root / "P.animpkg")})
        assert e["status"] == "ok", e
        return e["result"]["sceneId"]

    def close(self):
        self.p.kill()
        self.p.communicate(timeout=5)
        self.reader.join(timeout=2)
        self.tmp.cleanup()


class ReleaseFeature(unittest.TestCase):
    def test_suspension_hooks_are_not_a_normal_dependency_feature(self):
        import tomllib
        manifest = tomllib.loads((ROOT / "crates/executor/Cargo.toml").read_text())
        dep = manifest["dependencies"]["tt-persist"]
        self.assertNotIn("test-hooks", dep.get("features", []),
                         "test-hooks is enabled in the ordinary dependency graph, including release products")

    def test_percentile_evidence_retains_observations(self):
        for name in ["autosave-windows.json", "fingerprint-windows.json", "never-block-mcp-windows.json"]:
            value = json.loads((ROOT / "reviews/evidence/M1-C1.4/measurements" / name).read_text())
            def has_observations(item):
                if isinstance(item, dict):
                    return any((key.lower().endswith(("samples", "raw", "observations")) and isinstance(v, list)
                                and len(v) > 1) or has_observations(v) for key, v in item.items())
                if isinstance(item, list):
                    return any(has_observations(v) for v in item)
                return False
            self.assertTrue(has_observations(value), f"{name} retains percentiles but no recomputable observations")


@unittest.skipUnless(BINARY, "runtime cases are wired through cargo test -p tt-headless --test c14_r1")
class Protocol(unittest.TestCase):
    def setUp(self):
        self.c = Client()

    def tearDown(self):
        self.c.close()

    def test_atomic_failure_has_no_activity_export_effect(self):
        c = self.c
        c.project()
        out = c.root / "should-not-exist.jsonl"
        r = c.call("batch", {"label": "rollback export", "steps": [
            {"id": "agent.activity.export", "params": {"path": str(out)}},
            {"id": "no.such.command", "params": {}}]})
        print("atomic_export", json.dumps(r), "file_exists", out.exists(), flush=True)
        self.assertEqual(r["result"]["structuredContent"]["status"], "error")
        self.assertFalse(out.exists(), "failed atomic batch published an irreversible filesystem effect")

    def test_activity_batch_export_keeps_committed_batch_operations(self):
        c = self.c
        scene = c.project()
        result = c.call("batch", {"label": "Keep in replay", "steps": [
            {"id": "doc.object.patch", "params": {"targets": [scene], "props": {"meta": {"replaySentinel": 99}}}}]})
        self.assertEqual(result["result"]["structuredContent"]["status"], "ok")
        out = c.root / "replay.json"
        self.assertEqual(c.run("agent.activity.export", {"path": str(out), "format": "batch"})["status"], "ok")
        replay = out.read_text(encoding="utf-8")
        print("batch_activity_replay", replay, flush=True)
        self.assertIn("replaySentinel", replay, "export omitted a successfully committed batch")

    def test_activity_batch_export_does_not_turn_dry_runs_into_edits(self):
        c = self.c
        scene = c.project()
        result = c.call("set_properties", {"targets": [scene], "props": {"meta": {"dryRunSentinel": 99}}, "dryRun": True})
        self.assertEqual(result["result"]["structuredContent"]["status"], "ok")
        out = c.root / "replay.json"
        c.run("agent.activity.export", {"path": str(out), "format": "batch"})
        replay = json.loads(out.read_text(encoding="utf-8"))
        edits = [step for step in replay["steps"] if "dryRunSentinel" in json.dumps(step)]
        print("dry_activity_replay", json.dumps(edits), flush=True)
        self.assertTrue(all(step.get("dryRun") is True for step in edits), "a dry run becomes a real edit when replayed")

    def test_wrong_dry_run_type_does_not_commit(self):
        c = self.c
        scene = c.project()
        r = c.call("run_command", {"id": "doc.object.patch", "params": {
            "targets": [scene], "props": {"meta": {"review": 1}}}, "dryRun": "true"})
        print("wrong_dry_run", json.dumps(r), flush=True)
        self.assertEqual(r["result"]["structuredContent"]["status"], "error")

    def test_wrong_expect_rev_type_does_not_commit(self):
        c = self.c
        scene = c.project()
        r = c.call("set_properties", {"targets": [scene], "props": {"meta": {"review": 2}}, "expectRev": []})
        print("wrong_expect_rev", json.dumps(r), flush=True)
        self.assertEqual(r["result"]["structuredContent"]["status"], "error")

    def test_non_object_tool_arguments_are_rejected(self):
        r = self.c.call("get_state", [])
        print("wrong_arguments", json.dumps(r), flush=True)
        self.assertTrue("error" in r or r.get("result", {}).get("isError"), r)

    def test_get_state_matches_the_command_envelope(self):
        command = self.c.run("app.state.get", {})
        generic = self.c.call("get_state", {})["result"]["structuredContent"]
        print("state_wrapper_command", json.dumps(command), "generic", json.dumps(generic), flush=True)
        self.assertEqual(command, generic, "get_state changes its command's structured result")

    def test_unknown_tool_argument_is_rejected(self):
        r = self.c.call("get_state", {"misspelled": True})
        print("unknown_argument", json.dumps(r), flush=True)
        self.assertTrue("error" in r or r.get("result", {}).get("isError"), r)

    def test_malformed_elicitation_does_not_overwrite(self):
        c = self.c
        # Reinitialize only this isolated test client, with the capability needed for the request.
        c.close()
        self.c = c = Client({"elicitation": {}})
        c.project()
        out = c.root / "keep.jsonl"
        out.write_text("original", encoding="utf-8")
        c.n += 1
        rid = c.n
        c.send({"jsonrpc": "2.0", "id": rid, "method": "tools/call", "params": {
            "name": "run_command", "arguments": {"id": "agent.activity.export", "params": {
                "path": str(out), "onExists": "overwrite"}}}})
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            msg = c.q.get(timeout=5)
            if msg.get("method") == "elicitation/create":
                c.send({"jsonrpc": "2.0", "id": msg["id"], "result": {"action": "accept", "content": {"confirm": "false"}}})
                break
            c.saved.append(msg)
        r = c.wait(rid)
        print("malformed_elicitation", json.dumps(r), "preserved", out.read_text() == "original", flush=True)
        self.assertEqual(out.read_text(), "original", "invalid confirmation value authorized an overwrite")

    def test_elicitation_is_one_audit_entry_per_tool_call(self):
        c = self.c
        c.close()
        self.c = c = Client({"elicitation": {}})
        c.project()
        out = c.root / "keep.jsonl"
        out.write_text("original", encoding="utf-8")
        c.n += 1
        rid = c.n
        c.send({"jsonrpc": "2.0", "id": rid, "method": "tools/call", "params": {"name": "run_command", "arguments": {
            "id": "agent.activity.export", "params": {"path": str(out), "onExists": "overwrite"}}}})
        while True:
            msg = c.q.get(timeout=5)
            if msg.get("method") == "elicitation/create":
                c.send({"jsonrpc": "2.0", "id": msg["id"], "result": {"action": "accept", "content": {"confirm": True}}})
                break
            c.saved.append(msg)
        c.wait(rid)
        log = c.request("resources/read", {"uri": "app://activity"})["result"]["contents"][0]["text"]
        entries = [json.loads(line) for line in log.splitlines()]
        exports = [e for e in entries if e["commandId"] == "agent.activity.export"]
        print("elicitation_audit", json.dumps(exports), flush=True)
        self.assertEqual(len(exports), 1, "one protocol tool call was recorded twice")

    def test_accepted_elicitation_preserves_request_id_result(self):
        c = self.c
        c.close()
        self.c = c = Client({"elicitation": {}})
        c.project()
        out = c.root / "keep.jsonl"
        out.write_text("original", encoding="utf-8")
        args = {"id": "agent.activity.export", "params": {"path": str(out), "onExists": "overwrite"},
                "requestId": "confirmed-once"}
        results = []
        for _ in range(2):
            c.n += 1
            rid = c.n
            c.send({"jsonrpc": "2.0", "id": rid, "method": "tools/call", "params": {
                "name": "run_command", "arguments": args}})
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                msg = c.q.get(timeout=5)
                if msg.get("id") == rid and "method" not in msg:
                    results.append(msg["result"]["structuredContent"])
                    break
                if msg.get("method") == "elicitation/create":
                    c.send({"jsonrpc": "2.0", "id": msg["id"], "result": {
                        "action": "accept", "content": {"confirm": True}}})
                else:
                    c.saved.append(msg)
        print("elicitation_request_id", json.dumps(results), flush=True)
        self.assertEqual(len(results), 2)
        self.assertEqual(results[0]["status"], "ok")
        self.assertEqual(results[0], results[1], "an idempotent retry did not return the accepted result")

    def test_filter_has_bounded_runtime(self):
        c = self.c
        start = time.monotonic()
        try:
            # Short, schema-valid input should not occupy the sole call worker exponentially.
            c.request("tools/call", {"name": "list_commands", "arguments": {"filter": "*" * 32 + "Z"}}, timeout=5)
        except (queue.Empty, TimeoutError):
            self.fail("schema-valid list_commands filter did not answer within five seconds")
        self.assertLess(time.monotonic() - start, 5)

    def test_invalid_utf8_frame_does_not_destroy_stream(self):
        c = self.c
        c.p.stdin.write(b'\xff\n')
        c.p.stdin.flush()
        try:
            r = c.request("ping", {})
        except (BrokenPipeError, queue.Empty, TimeoutError):
            self.fail("invalid UTF-8 frame ended the server or prevented the following ping")
        self.assertEqual(r.get("result"), {})

    def test_project_resource_is_readable(self):
        self.c.project()
        r = self.c.request("resources/read", {"uri": "app://project"})
        print("project_resource", json.dumps(r), flush=True)
        self.assertIn("result", r)

    def test_size_and_mtime_preserving_edit_requires_confirmation(self):
        c = self.c
        c.project()
        path = c.root / "P.animpkg"
        stat = path.stat()
        data = bytearray(path.read_bytes())
        data[-1] ^= 1
        path.write_bytes(data)
        os.utime(path, ns=(stat.st_atime_ns, stat.st_mtime_ns))
        result = c.run("project.save", {})
        print("same_size_mtime_save", json.dumps(result), flush=True)
        self.assertEqual(result["status"], "needs_confirmation", "RO-02 did not notice changed project bytes")

    def test_undo_and_redo_emit_the_declared_events(self):
        c = self.c
        scene = c.project()
        c.run("doc.object.patch", {"targets": [scene], "props": {"meta": {"eventSentinel": 1}}})
        c.call("undo", {})
        c.call("redo", {})
        time.sleep(.1)
        body = c.request("resources/read", {"uri": "app://events"})["result"]["contents"][0]["text"]
        names = [e["type"] for e in json.loads(body)["events"]]
        print("undo_redo_events", json.dumps(names), flush=True)
        self.assertIn("doc.undone", names)
        self.assertIn("doc.redone", names)

    def test_closed_stdout_ends_the_unusable_session(self):
        # A fresh process with no background stdout reader, so closing the read end is deterministic.
        c = self.c
        p = subprocess.Popen([BINARY, "--mcp-stdio", "--user-dir", str(c.root / "closed-user")],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            p.stdout.close()
            p.stdout = None
            p.stdin.write(b'{"jsonrpc":"2.0","id":1,"method":"ping"}\n')
            p.stdin.flush()
            try:
                p.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.fail("output failure was ignored and the unusable server session remained running")
        finally:
            if p.poll() is None:
                p.kill()
            p.communicate(timeout=5)

    def test_eof_while_eliciting_ends_the_session(self):
        c = self.c
        c.close()
        self.c = c = Client({"elicitation": {}})
        c.project()
        out = c.root / "keep.jsonl"
        out.write_text("original", encoding="utf-8")
        c.send({"jsonrpc": "2.0", "id": "pending", "method": "tools/call", "params": {
            "name": "run_command", "arguments": {"id": "agent.activity.export", "params": {
                "path": str(out), "onExists": "overwrite"}}}})
        while c.q.get(timeout=5).get("method") != "elicitation/create":
            pass
        c.p.stdin.close()
        c.p.stdin = None
        try:
            c.p.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.fail("EOF joins the call worker while it still waits up to 300 seconds for a client response")

    def test_second_initialize_is_rejected(self):
        r = self.c.request("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                          "clientInfo": {"name": "second", "version": "1"}})
        self.assertIn("error", r, "a second initialize changed the negotiated session")

    def test_nonreading_client_with_eof_does_not_pin_the_session(self):
        c = self.c
        p = subprocess.Popen([BINARY, "--mcp-stdio", "--user-dir", str(c.root / "pressure-user")],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def writer():
            try:
                messages = [{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                    "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": {"name": "pressure", "version": "1"}}}]
                messages += [{"jsonrpc": "2.0", "id": i + 10, "method": "tools/call",
                              "params": {"name": "get_state", "arguments": {}}} for i in range(300)]
                for msg in messages:
                    p.stdin.write((json.dumps(msg) + "\n").encode())
                    p.stdin.flush()
                p.stdin.close()
            except (OSError, ValueError):
                pass
        thread = threading.Thread(target=writer, daemon=True)
        thread.start()
        try:
            thread.join(timeout=5)
            self.assertFalse(thread.is_alive(), "input writer did not finish bounded input")
            try:
                p.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.fail("a client that never reads output pins shutdown after input EOF")
        finally:
            if p.poll() is None:
                p.kill()
            thread.join(timeout=2)
            p.stdin = None
            p.communicate(timeout=5)

    def test_unknown_cancellation_does_not_cancel_a_future_request(self):
        c = self.c
        c.send({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 9000}})
        c.send({"jsonrpc": "2.0", "id": 9000, "method": "tools/list", "params": {}})
        try:
            c.wait(9000, 2)
        except (queue.Empty, TimeoutError):
            self.fail("a cancellation for an unknown request cancelled a later request")


if __name__ == "__main__":
    unittest.main(verbosity=2)
