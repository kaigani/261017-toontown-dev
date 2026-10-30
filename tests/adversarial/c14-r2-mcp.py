"""M1-C1.4 round-2 real-process regressions and variants. All waits are bounded.

The Cargo wrapper sets TT_REVIEW_BINARY; the Python suite runner discovers this file too.
"""
import importlib.util
import itertools
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
spec = importlib.util.spec_from_file_location("r1", ROOT / "tests/adversarial/c14-r1-mcp.py")
r1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r1)
BINARY = os.environ.get("TT_REVIEW_BINARY")


def env(response):
    return response["result"]["structuredContent"]


@unittest.skipUnless(BINARY, "real binary supplied by bins/headless/tests/c14_r2.rs")
class Protocol(unittest.TestCase):
    def setUp(self):
        self.c = r1.Client()

    def tearDown(self):
        self.c.close()

    def test_typed_tools_reject_nested_types_and_extra_options(self):
        self.c.close()
        self.c = c = r1.Client(extra=("--typed-tools", "doc"))
        scene = c.project()
        for args in [{"targets": [scene], "props": {}, "dryRun": "true"},
                     {"targets": [scene], "props": {}, "unknownReviewOption": True},
                     {"targets": [None], "props": {}}, {"targets": [scene], "props": None}]:
            e = env(c.call("doc_object_patch", args))
            self.assertEqual(e["status"], "error", e)
            self.assertTrue(e["error"]["code"].startswith("arg."), e)

    def test_nonreading_client_without_eof_is_closed_by_pressure_timeout(self):
        p = subprocess.Popen([BINARY, "--mcp-stdio", "--user-dir", str(self.c.root / "pressure")],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def flood():
            try:
                # Inline ping doesn't need initialize. Enough replies fill the OS pipe and bounded queue.
                for i in range(15000):
                    p.stdin.write((json.dumps({"jsonrpc": "2.0", "id": i, "method": "ping"}) + "\n").encode())
                    p.stdin.flush()
            except (OSError, ValueError):
                pass
        thread = threading.Thread(target=flood, daemon=True)
        thread.start()
        try:
            p.wait(timeout=18)
        finally:
            if p.poll() is None:
                p.kill()
            thread.join(timeout=2)
            p.communicate(timeout=5)

    def test_invalid_utf8_at_eof_is_a_parse_error_and_shutdown_is_bounded(self):
        p = subprocess.Popen([BINARY, "--mcp-stdio", "--user-dir", str(self.c.root / "utf8-eof")],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            out, _ = p.communicate(b'{"bad":"\xff"}', timeout=5)
            self.assertTrue(any(json.loads(line).get("error", {}).get("code") == -32700 for line in out.splitlines()), out)
        finally:
            if p.poll() is None:
                p.kill()
                p.communicate(timeout=5)

    def test_nested_wrapper_validation_never_edits(self):
        c = self.c
        scene = c.project()
        patch = {"id": "doc.object.patch", "params": {"targets": [scene], "props": {"meta": {"bad": 1}}}}
        cases = [("run_command", {**patch, k: v}) for k, v in [
            ("dryRun", None), ("dryRun", 1), ("expectRev", {scene: "1"}),
            ("expectRev", {scene: -1}), ("confirm", "false"), ("confirmToken", False), ("requestId", [])]]
        cases += [("batch", {"label": "invalid", "steps": [patch], "dryRun": "true"}),
                  ("batch", {"label": "invalid", "steps": [{**patch, "dryRun": True}]}),
                  ("batch", {"label": "invalid", "steps": [{"id": patch["id"], "params": None}]}),
                  ("set_properties", {"targets": [scene], "props": {}, "expectRev": {scene: []}}),
                  ("get_state", {"include": ["project", None]}),
                  ("get_object", {"ref": scene, "page": {"limit": 1, "extra": True}}),
                  ("get_object", {"ref": scene, "fields": [False]}),
                  ("list_commands", {"page": {"limit": 0}}),
                  ("list_commands", {"page": {"limit": "1"}})]
        for name, args in cases:
            with self.subTest(name=name, args=args):
                e = env(c.call(name, args))
                self.assertEqual(e["status"], "error", e)
                self.assertTrue(e["error"]["code"].startswith("arg."), e)
        obj = c.run("doc.object.get", {"ref": scene})["result"]["object"]
        self.assertNotIn("bad", obj.get("meta", {}))

    def test_all_state_include_combinations_are_command_projections(self):
        c = self.c
        c.project()
        full = c.run("app.state.get", {})["result"]
        mapping = {"project": ["project"], "scene": ["scene"], "frame": ["frame", "element", "drawing"],
                   "selection": ["selection"], "tool": ["activeTool"],
                   "views": ["activeView", "focusedPanel", "workspace", "mode", "fullScreen"],
                   "playback": ["playback"], "jobs": ["jobs"],
                   "ui": ["modal", "startScreenVisible", "focusedPanel", "workspace", "fullScreen"],
                   "agent": ["agent", "agentSessions"]}
        for bits in itertools.product([False, True], repeat=len(mapping)):
            include = [k for k, yes in zip(mapping, bits) if yes]
            keys = {k for i in include for k in mapping[i]}
            projected = env(c.call("get_state", {"include": include}))["result"]
            self.assertEqual(projected, {k: v for k, v in full.items() if k in keys}, include)

    def test_long_unicode_and_nested_globs_keep_the_server_usable(self):
        c = self.c
        for pattern in ["*" * 10000 + "Z", "*?" * 5000 + "Z", "\u52d5\u756b*?", "*a*b*c*?*project*", "?" * 5000]:
            started = time.monotonic()
            self.assertEqual(env(c.call("list_commands", {"filter": pattern}))["status"], "ok")
            self.assertLess(time.monotonic() - started, 5)
            self.assertEqual(c.request("ping", {})["result"], {})

    def test_export_publication_failure_is_not_a_successful_audit_record(self):
        # C14-R2-01: staging a sibling works, publishing onto a directory cannot.
        self.c.close()
        self.c = c = r1.Client(extra=("--policy", "autonomous"))
        c.project()
        target = c.root / "directory.jsonl"
        target.mkdir()
        e = c.run("agent.activity.export", {"path": str(target), "onExists": "overwrite"})
        print("publication_failure", json.dumps(e), "target_is_directory", target.is_dir(), flush=True)
        entries = c.run("agent.activity.list", {"limit": 100})["result"]
        exported = [x for x in entries if x["commandId"] == "agent.activity.export"]
        self.assertEqual(e["status"], "error", "a file that was never published was reported as exported")
        self.assertEqual(exported[-1]["status"], "error", exported)

    def test_close_with_save_offers_external_change_confirmation(self):
        for folder in [False, True]:
            with self.subTest(folder=folder):
                self.c.close()
                self.c = c = r1.Client()
                target = c.root / ("Folder" if folder else "P.animpkg")
                made = c.run("project.create", {"name": "P", "path": str(target) + (os.sep if folder else "")})
                self.assertEqual(made["status"], "ok", made)
                scene = made["result"]["sceneId"]
                c.run("doc.object.patch", {"targets": [scene], "props": {"meta": {"n": 1}}})
                project = target / "project.json" if folder else target
                stamp = project.stat()
                changed = bytearray(project.read_bytes())
                changed[-1] ^= 1
                project.write_bytes(changed)
                os.utime(project, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
                e = c.run("project.close", {"onUnsaved": "save"})
                self.assertEqual(project.read_bytes(), changed)
                self.assertEqual(e["status"], "needs_confirmation", e)
                self.assertEqual(e["result"]["category"], "externalChange", e)

    def test_partial_sequential_batch_keeps_its_committed_edits_in_replay(self):
        # C14-R2-02: an error envelope does not roll back earlier non-atomic steps.
        c = self.c
        scene = c.project()
        e = env(c.call("batch", {"label": "partial", "atomic": False, "steps": [
            {"id": "doc.object.patch", "params": {"targets": [scene], "props": {"meta": {"kept": 1}}}},
            {"id": "unknown.command", "params": {}}]}))
        self.assertEqual(e["status"], "error", e)
        self.assertEqual(c.run("doc.object.get", {"ref": scene})["result"]["object"]["meta"]["kept"], 1)
        target = c.root / "partial.json"
        export = c.run("agent.activity.export", {"path": str(target), "format": "batch"})
        replay = json.loads(target.read_text())
        print("partial_replay", json.dumps({"export": export, "batch": replay}), flush=True)
        kept = [s for s in replay["steps"] if s["id"] == "doc.object.patch"]
        self.assertEqual(len(kept), 1, "export omitted an edit that committed before a later step failed")

    def test_idempotent_retry_is_not_a_second_replay_operation(self):
        # C14-R2-03: the same successful cached undo is two calls, but only one edit.
        c = self.c
        scene = c.project()
        for n in [1, 2]:
            self.assertEqual(c.run("doc.object.patch", {"targets": [scene], "props": {"meta": {"n": n}}})["status"], "ok")
        args = {"id": "edit.undo", "params": {}, "requestId": "undo-once"}
        first = env(c.call("run_command", args))
        self.assertEqual(first["status"], "ok", first)
        self.assertEqual(env(c.call("run_command", args)), first)
        target = c.root / "retry.json"
        c.run("agent.activity.export", {"path": str(target), "format": "batch"})
        replay = json.loads(target.read_text())
        print("retry_replay", json.dumps(replay), flush=True)
        self.assertEqual(sum(s["id"] == "edit.undo" for s in replay["steps"]), 1,
                         "a cached requestId retry became another undo in the replay")

    def test_cancel_live_elicitation_releases_the_call_worker(self):
        # C14-R2-04: cancelling a waiting call must not pin subsequent work for the token's 300 seconds.
        self.c.close()
        self.c = c = r1.Client(caps={"elicitation": {}})
        c.project()
        target = c.root / "old.jsonl"
        target.write_text("old")
        c.send({"jsonrpc": "2.0", "id": 800, "method": "tools/call", "params": {
            "name": "run_command", "arguments": {"id": "agent.activity.export", "params": {
                "path": str(target), "onExists": "overwrite"}}}})
        deadline = time.monotonic() + 10
        while True:
            msg = c.q.get(timeout=max(.001, deadline - time.monotonic()))
            if msg.get("method") == "elicitation/create":
                break
            c.saved.append(msg)
        c.send({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 800}})
        self.assertEqual(c.request("ping", {}, timeout=2)["result"], {})
        c.send({"jsonrpc": "2.0", "id": 801, "method": "tools/call", "params": {"name": "get_state", "arguments": {}}})
        try:
            answer = c.wait(801, timeout=5)
        except (queue.Empty, TimeoutError):
            self.fail("cancelled elicitation still holds the call worker after 5 seconds (ping answered)")
        self.assertEqual(env(answer)["status"], "ok")
        self.assertEqual(target.read_text(), "old")

    def test_decline_cancel_and_invalid_accept_settle_request_id_and_audit_once(self):
        for answer in [{"action": "decline"}, {"action": "cancel"}, {"action": "accept"},
                       {"action": "accept", "content": {"confirm": None}},
                       {"action": "accept", "content": {"confirm": 1}}]:
            with self.subTest(answer=answer):
                c = r1.Client(caps={"elicitation": {}})
                try:
                    c.project()
                    target = c.root / "old.jsonl"
                    target.write_text("old")
                    args = {"id": "agent.activity.export", "params": {"path": str(target), "onExists": "overwrite"},
                            "requestId": "declined"}
                    c.send({"jsonrpc": "2.0", "id": 800, "method": "tools/call", "params": {"name": "run_command", "arguments": args}})
                    while True:
                        msg = c.q.get(timeout=10)
                        if msg.get("method") == "elicitation/create":
                            break
                        c.saved.append(msg)
                    c.send({"jsonrpc": "2.0", "id": msg["id"], "result": answer})
                    first = env(c.wait(800))
                    self.assertEqual(first["error"]["code"], "confirmation.declined", first)
                    entries = c.run("agent.activity.list", {"limit": 100})["result"]
                    exported = [x for x in entries if x["commandId"] == "agent.activity.export"]
                    self.assertEqual(len(exported), 1, exported)
                    self.assertEqual(exported[0]["errorCode"], "confirmation.declined", exported)
                    self.assertEqual(env(c.call("run_command", args)), first)
                    self.assertEqual(target.read_text(), "old")
                finally:
                    c.close()

    def test_invalid_utf8_mid_frame_and_oversized_frames_resynchronize(self):
        c = self.c
        c.p.stdin.write(b'{"bad":"\xff"}\n')
        c.p.stdin.flush()
        self.assertEqual(c.request("ping", {}, timeout=5)["result"], {})
        self.assertTrue(any(m.get("error", {}).get("code") == -32700 for m in c.saved))
        # Default maxInlineBytes is 50 MiB; stream chunks instead of allocating that in the test.
        for _ in range(51):
            c.p.stdin.write(b"x" * (1024 * 1024))
        c.p.stdin.write(b"\n")
        c.p.stdin.flush()
        self.assertEqual(c.request("ping", {}, timeout=5)["result"], {})
        self.assertTrue(any(m.get("error", {}).get("data", {}).get("limit") == "maxInlineBytes" for m in c.saved))


class MeasurementValidation(unittest.TestCase):
    def test_analyzer_rejects_backwards_time_nonfinite_and_empty_series(self):
        spec = importlib.util.spec_from_file_location("measurements", ROOT / "scripts/analyse-measurements.py")
        m = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(m)
        base = {"environment": {"os": "windows", "arch": "x86_64", "cpus": 1, "profile": "test",
                                "startedAt": "2026-10-30T00:00:00Z", "timeOrigin": "Instant at test start"},
                "series": {"latency": {"unit": "ms", "samples": [{"tMs": 2, "value": 1}, {"tMs": 1, "value": 2}],
                                        "summary": m.summarise([1, 2])}}}
        cases = [base, {**base, "series": {}},
                 {**base, "series": {"latency": {"unit": "ms", "samples": [{"tMs": float("inf"), "value": 1}],
                                                   "summary": m.summarise([1])}}}]
        with tempfile.TemporaryDirectory() as d:
            for i, doc in enumerate(cases):
                p = pathlib.Path(d) / f"bad-{i}.json"
                p.write_text(json.dumps(doc))
                with self.subTest(case=i):
                    self.assertFalse(m.check_file(p)["ok"], "malformed measurement evidence was accepted")


if __name__ == "__main__":
    unittest.main(verbosity=2)
