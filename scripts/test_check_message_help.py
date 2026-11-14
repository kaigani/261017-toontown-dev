"""Fixture tests of scripts/check-message-help.py's scanner (M4 C4.1 round 2, C41-R2-20): its coverage is tested, not
assumed. Each case writes a small product source into a temporary folder and runs the scanner over it.

Usage: python scripts/test_check_message_help.py
"""
import importlib.util
import pathlib
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("checker", HERE / "check-message-help.py")
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class Scanner(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        self.saved = (checker.ROOT, checker.rust_sources, checker.frontend_sources)
        checker.ROOT = self.root

    def tearDown(self):
        checker.ROOT, checker.rust_sources, checker.frontend_sources = self.saved
        self.tmp.cleanup()

    def rust(self, text):
        p = self.root / "fixture.rs"
        p.write_text(text, encoding="utf-8")
        checker.rust_sources = lambda: iter([p])
        return checker.produced_codes()

    def front(self, text):
        p = self.root / "fixture.tsx"
        p.write_text(text, encoding="utf-8")
        checker.frontend_sources = lambda: iter([p])
        unresolved = []
        return checker.frontend_ids(unresolved), unresolved

    def test_an_undotted_code_is_collected(self):
        self.assertIn("newUndotted", self.rust('fn f() { CommandError::new("newUndotted", "x"); }'))

    def test_every_constructor_shape_is_collected(self):
        codes = self.rust(
            'fn f() {\n'
            '  CommandError::with_details(\n    "a.multiLine",\n    "m",\n    json!({}),\n  );\n'
            '  ModelError::new("b.model", "m");\n'
            '  let d = Disabled { code: "c.disabled", reason: r };\n'
            '  let e = ErrorInfo { code: "d.info".into(), message: m, details: v };\n'
            '  envelope_error("e.mcp", m, json!({}), None);\n'
            '  json!({ "code": "f.json", "x": 1 });\n'
            '  json!({ "messageId": "g.shown" });\n'
            '}\n'
        )
        for code in ["a.multiLine", "b.model", "c.disabled", "d.info", "e.mcp", "f.json", "g.shown"]:
            self.assertIn(code, codes)

    def test_code_module_constants_are_collected(self):
        codes = self.rust('pub mod codes {\n    pub const NEW_ONE: &str = "h.constant";\n}\n')
        self.assertIn("h.constant", codes)

    def test_test_modules_are_ignored(self):
        codes = self.rust('#[cfg(test)]\nmod tests {\n    fn t() { CommandError::new("only.inTests", "x"); }\n}\n')
        self.assertNotIn("only.inTests", codes)

    def test_a_code_outside_the_grammar_is_reported_not_dropped(self):
        codes = self.rust('fn f() { CommandError::new("Bad Code", "x"); CommandError::new("", "x"); }')
        missing, bad, _ = checker.problems(codes, [], set())
        self.assertEqual(bad, ["", "Bad Code"])
        self.assertIn("Bad Code", missing)

    def test_a_computed_front_end_id_is_resolved(self):
        ids, unresolved = self.front(
            "const SAVE_FAILED = 'prefs.saveFailed';\n"
            "show({ messageId: SAVE_FAILED, text });\n"
            "show({ messageId: ok ? 'a.first' : 'b.second' });\n"
            "show({ messageId: e?.code ?? 'internal' });\n"
        )
        for code in ["prefs.saveFailed", "a.first", "b.second", "internal"]:
            self.assertIn(code, ids)
        self.assertEqual(unresolved, [])

    def test_forwarded_ids_and_types_are_accepted(self):
        ids, unresolved = self.front(
            "interface M { messageId: string; }\n"
            "const schema = { messageId: { type: 'string' } };\n"
            "notify({ messageId: n.messageId });\n"
            "notify({ messageId: typeof p['messageId'] === 'string' ? p['messageId'] : 'agent' });\n"
        )
        self.assertEqual(set(ids), {"agent"})
        self.assertEqual(unresolved, [])

    def test_an_unresolvable_front_end_value_is_reported(self):
        _, unresolved = self.front("show({ messageId: makeId(kind) });\nshow({ messageId: someVariable });\n")
        self.assertEqual([v for _, v in unresolved], ["makeId(kind)", "someVariable"])


if __name__ == "__main__":
    unittest.main()
