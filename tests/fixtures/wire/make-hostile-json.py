"""Writes tests/fixtures/wire/hostile-json.json (C11-R1-17). Run from the repository root: python tests/fixtures/wire/make-hostile-json.py"""
import json, os

reject = [
    ("empty text", ""),
    ("white space only", " \n"),
    ("byte order mark", "﻿{}"),
    ("trailing characters", "{} x"),
    ("two values", "1 2"),
    ("trailing comma in array", "[1,]"),
    ("trailing comma in object", '{"a":1,}'),
    ("missing comma", "[1 2]"),
    ("leading comma", "[,1]"),
    ("unclosed array", "["),
    ("unclosed object", "{"),
    ("key without value", '{"a"}'),
    ("colon without value", '{"a":}'),
    ("unquoted key", "{a:1}"),
    ("single-quoted string", "'a'"),
    ("line comment", "[1]//c"),
    ("block comment", "/*c*/[1]"),
    ("non-breaking space as white space", " [1]"),
    ("vertical tab as white space", "\u000b[1]"),
    ("leading zero", "01"),
    ("negative leading zero", "-01"),
    ("fraction without digits", "1."),
    ("fraction without integer part", ".5"),
    ("plus sign", "+1"),
    ("lone minus", "-"),
    ("exponent without digits", "1e"),
    ("signed exponent without digits", "1e+"),
    ("hexadecimal", "0x10"),
    ("NaN", "NaN"),
    ("Infinity", "Infinity"),
    ("negative Infinity", "-Infinity"),
    ("truncated null", "nul"),
    ("truncated true", "tru"),
    ("capitalised literal", "True"),
    ("double overflow", "1e400"),
    ("negative double overflow", "-1.5e400"),
    ("integer above u64::MAX, not exactly a double", "18446744073709551617"),
    ("integer below -u64::MAX, not exactly a double", "-18446744073709551615"),
    ("integer below i64::MIN, not exactly a double", "-9223372036854775809"),
    ("30-digit integer, not exactly a double", "123456789012345678901234567890"),
    ("21-digit integer, not exactly a double", "123456789012345678901"),
    ("duplicate key", '{"a":1,"a":2}'),
    ("duplicate key after unescaping", '{"a":1,"\\u0061":2}'),
    ("nested duplicate key", '{"meta":{"x":1,"x":2}}'),
    ("duplicate key deep in an array", '[{"k":[{"x":1,"y":2,"x":3}]}]'),
    ("lone high surrogate escape", '"\\ud800"'),
    ("lone low surrogate escape", '"\\udc00"'),
    ("high surrogate followed by a letter", '"\\ud800x"'),
    ("two high surrogates", '"\\udbff\\udbff"'),
    ("reversed surrogate pair", '"\\udc00\\ud800"'),
    ("lone surrogate in a key", '{"\\ud800":1}'),
    ("raw control character in a string", '"a\u0001b"'),
    ("raw line feed in a string", '"a\nb"'),
    ("invalid escape", '"\\x"'),
    ("short unicode escape", '"\\u00"'),
    ("non-hex unicode escape", '"\\u00G0"'),
    ("unterminated string", '"abc'),
    ("unterminated escape", '"\\'),
]
nest_reject = [
    ("129 nested arrays", "[", "]", 129, ""),
    ("128 nested arrays around a scalar (scalar at depth 129)", "[", "]", 128, "0"),
    ("128 nested objects around a scalar", '{"a":', "}", 128, "0"),
]
accept = [
    ("zero", "0", "0"),
    ("negative zero is written as 0", "-0", "0"),
    ("white space and scalars", " [1, 2.5 ,true,null] ", "[1,2.5,true,null]"),
    ("u64::MAX", "18446744073709551615", "18446744073709551615"),
    ("i64::MIN", "-9223372036854775808", "-9223372036854775808"),
    ("2^53 + 1", "9007199254740993", "9007199254740993"),
    ("2^64, exactly a double", "18446744073709551616", "18446744073709551616"),
    ("-2^64, exactly a double", "-18446744073709551616", "-18446744073709551616"),
    ("1e20 in full, exactly a double", "100000000000000000000", "100000000000000000000"),
    ("exponent form", "1E2", "100"),
    ("small double", "1e-7", "1e-7"),
    ("large double", "1.5e300", "1.5e+300"),
    ("underflow to zero", "1e-400", "0"),
    ("1e21 switches to exponent form", "1e21", "1e+21"),
    ("shortest round-trip digits", "0.30000000000000004", "0.30000000000000004"),
    ("surrogate pair", '"\\ud83d\\ude00"', '"\U0001F600"'),
    ("line separator", '"\\u2028"', '" "'),
    ("control escapes", '"\\b\\f\\n\\r\\t\\u001f"', '"\\b\\f\\n\\r\\t\\u001f"'),
    ("escaped solidus", '"\\/"', '"/"'),
    ("delete character", '"\\u007f"', '"\u007f"'),
    ("__proto__ is an ordinary key", '{"__proto__":{"x":1},"constructor":2}', '{"__proto__":{"x":1},"constructor":2}'),
    ("empty containers", '{"a":{},"b":[]}', '{"a":{},"b":[]}'),
]
nest_accept = [
    ("128 nested arrays", "[", "]", 128, ""),
    ("127 nested arrays around a scalar", "[", "]", 127, "0"),
]
bytes_reject = [
    ("invalid UTF-8 byte", [0xFF]),
    ("overlong encoding of /", [0x22, 0xC0, 0xAF, 0x22]),
    ("UTF-8-encoded surrogate", [0x22, 0xED, 0xA0, 0x80, 0x22]),
    ("truncated four-byte sequence", [0x22, 0xF0, 0x9F, 0x98, 0x22]),
]

def nest(o, c, n, inner):
    return o * n + inner + c * n

doc = {
    "description": (
        "Shared hostile-JSON corpus (C11-R1-17): the engine (tt_model::json_text via tt_engine::wire::from_wire_json, "
        "crates/engine/tests/wire_corpus.rs) and the front end (parseWireJson, desktop/src/ipc/wireCorpus.test.ts) "
        "must reject every `reject` text and accept every `accept` text, writing it back as `canonical`. `nest` "
        "entries expand to open*count + inner + close*count. `bytes` entries are invalid UTF-8 request bodies."
    ),
    "maxDepth": 128,
    "reject": [{"why": w, "text": t} for w, t in reject]
    + [{"why": w, "nest": {"open": o, "close": c, "count": n, "inner": i}} for w, o, c, n, i in nest_reject],
    "accept": [{"why": w, "text": t, "canonical": c} for w, t, c in accept]
    + [{"why": w, "nest": {"open": o, "close": c, "count": n, "inner": i}, "canonical": nest(o, c, n, i)}
       for w, o, c, n, i in nest_accept],
    "bytes": [{"why": w, "bytes": b} for w, b in bytes_reject],
}
os.makedirs("tests/fixtures/wire", exist_ok=True)
with open("tests/fixtures/wire/hostile-json.json", "w", encoding="utf-8", newline="\n") as f:
    json.dump(doc, f, indent=2, ensure_ascii=True)
    f.write("\n")
print(len(doc["reject"]), len(doc["accept"]), len(doc["bytes"]))
