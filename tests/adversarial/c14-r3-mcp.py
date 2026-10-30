"""Round-three malformed-parameter watchdog and staged-export entry-point variants."""
import importlib.util
import json
import os
import pathlib
import subprocess
import time
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("r1", ROOT / "tests/adversarial/c14-r1-mcp.py")
r1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r1)
BINARY = os.environ.get("TT_REVIEW_BINARY")


@unittest.skipUnless(BINARY, "set TT_REVIEW_BINARY to a built headless binary")
class Protocol(unittest.TestCase):
    def setUp(self):
        self.c = r1.Client(extra=("--typed-tools", "doc,agent", "--policy", "autonomous"))

    def tearDown(self):
        self.c.close()

    def test_every_registered_command_rejects_malformed_params_within_watchdog(self):
        c = self.c
        c.project()
        cursor = None
        ids = []
        while True:
            args = {"page": {"limit": 500}}
            if cursor:
                args["page"]["cursor"] = cursor
            result = c.call("list_commands", args)["result"]["structuredContent"]["result"]
            items = result.get("items", result.get("commands", []))
            ids.extend(x["id"] for x in items)
            cursor = result.get("nextCursor")
            if not cursor:
                break
        self.assertGreater(len(ids), 700, result)
        for command in ids:
            for params in [None, [], {"__review_unknown_field__": {"nested": [False, None]}}]:
                with self.subTest(command=command, params=params):
                    started = time.monotonic()
                    response = c.request("tools/call", {"name": "run_command", "arguments": {"id": command, "params": params}}, timeout=5)
                    self.assertLess(time.monotonic() - started, 5)
                    self.assertEqual(response["result"]["structuredContent"]["status"], "error", response)
        print(f"malformed watchdog: {len(ids)} commands, {3 * len(ids)} calls", flush=True)

    def test_generic_typed_and_atomic_exports_preserve_targets_on_later_failure(self):
        c = self.c
        c.project()
        for command, base in [("doc.export.json", {"scope": "scenes", "refs": ["scene:#1"]}),
                              ("agent.activity.export", {})]:
            for existing in [False, True]:
                target = c.root / (command + str(existing) + ".json")
                if existing:
                    target.write_bytes(b"original")
                params = {**base, "path": str(target), "onExists": "overwrite"}
                e = c.call("batch", {"label": "rollback", "atomic": True, "steps": [
                    {"id": command, "params": params}, {"id": "unknown.command", "params": {}}]})
                self.assertEqual(e["result"]["structuredContent"]["status"], "error", e)
                self.assertEqual(target.read_bytes() if target.exists() else None, b"original" if existing else None)
                e = c.call("run_command", {"id": command, "params": params, "dryRun": True})
                self.assertEqual(e["result"]["structuredContent"]["status"], "ok", e)
                self.assertEqual(target.read_bytes() if target.exists() else None, b"original" if existing else None)
                e = c.call(command.replace(".", "_"), params)
                self.assertEqual(e["result"]["structuredContent"]["status"], "ok", e)
                self.assertTrue(target.is_file())
                self.assertNotEqual(target.read_bytes(), b"original")

    def test_headless_run_rolls_back_export_on_later_failure(self):
        c = self.c
        c.project()
        target = c.root / "script-export.json"
        script = c.root / "script.json"
        script.write_text(json.dumps({"label": "rollback", "atomic": True, "steps": [
            {"id": "doc.export.json", "params": {"path": str(target)}},
            {"id": "unknown.command", "params": {}}]}), encoding="utf-8")
        p = subprocess.run([BINARY, "--run", str(script), "--project", str(c.root / "P.animpkg"),
                            "--allow-path", str(c.root), "--user-dir", str(c.root / "script-user")],
                           capture_output=True, timeout=15)
        self.assertNotEqual(p.returncode, 0, p.stdout)
        self.assertFalse(target.exists(), p.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
