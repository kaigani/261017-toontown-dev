"""Every message the user or an agent can be shown has help content (M4 C4.1, ruling O-11; round 2, C41-R2-20).

The message id is the agent error code (01 §9.5), so the set to cover is mechanical:
1. every error code the product can produce, in the Rust product sources: every string literal passed as the code at
   an error constructor site, whatever its shape (`CommandError::new` / `with_details`, `ModelError::new` /
   `with_details`, `Disabled { code: ... }`, `ErrorInfo { code: ... }`, the MCP `envelope_error(...)`, a JSON
   `"code": "..."` member), the `pub const NAME: &str = "..."` constants of the code modules, and every
   `"messageId": "..."` the kernel emits;
2. every message id the front end can show (desktop/src, tests excluded): the value of every `messageId:` property.
   A literal anywhere in the value counts (`'a.b'`, `x ?? 'a.b'`, `c ? y : 'a.b'`); an identifier is resolved to its
   `const NAME = '...'` in the same file; a value that forwards an id produced elsewhere (`x.messageId`,
   `x['messageId']`, `x.code`: engine codes, host notices, tips, all collected at their source) is accepted; any
   other value is reported as unresolved, so a computed id cannot evade the check;
3. every notice the desktop host sends (`notice(app, "id", ...)`).

Collection does not depend on the shape of a code; the grammar is checked separately: a collected code must be
lower camel case words joined by dots (`internal`, `ui.busy`, `prefs.saveFailed`).

The help catalogue is `MESSAGES` in crates/executor/src/help.rs plus the suppressible tips (`TIPS`) in
crates/executor/src/commands/ui_shell.rs; `help::message` serves both (`ui.help.get {messageId}`).

The scanner's own coverage is tested with fixtures: scripts/test_check_message_help.py.

Usage: python scripts/check-message-help.py [--list]   (exit 1 and the problems when coverage is incomplete)
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
#: The grammar of a code (checked after collection, never used to collect).
GRAMMAR = re.compile(r"[a-z][A-Za-z0-9]*(?:\.[a-z][A-Za-z0-9]*)*")
LITERAL = r'"((?:[^"\\]|\\.)*)"'
#: Error constructor sites: the code literal follows.
CONSTRUCTOR = re.compile(
    r"(?:(?:CommandError|ModelError)::(?:new|with_details)\(\s*"
    r"|\bDisabled\s*\{\s*code:\s*"
    r"|\bErrorInfo\s*\{\s*code:\s*"
    r"|\benvelope_error\(\s*"
    r'|"code"\s*:\s*)' + LITERAL
)
MESSAGE_ID = re.compile(r'"messageId"\s*:\s*' + LITERAL)
CONSTANT = re.compile(r"pub(?:\(crate\))?\s+const\s+[A-Z0-9_]+\s*:\s*&str\s*=\s*" + LITERAL)


def rust_sources():
    for base in ["crates", "desktop/src-tauri/src", "bins"]:
        for p in (ROOT / base).rglob("*.rs"):
            rel = p.relative_to(ROOT).as_posix()
            if "/tests/" in rel or "/benches/" in rel or "/examples/" in rel or rel.startswith("crates/registry/src/declared"):
                continue
            yield p


def frontend_sources():
    for p in (ROOT / "desktop/src").rglob("*.ts*"):
        rel = p.relative_to(ROOT).as_posix()
        if ".test." in rel or "/test/" in rel or "/test-support/" in rel or "/generated/" in rel or "/spikes/" in rel:
            continue
        yield p


def strip_tests(text):
    """Drops `#[cfg(test)] mod ... { ... }` blocks (brace matching)."""
    out, i = [], 0
    for m in re.finditer(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{", text):
        if m.start() < i:
            continue
        out.append(text[i:m.start()])
        depth, j = 1, m.end()
        while j < len(text) and depth:
            depth += {"{": 1, "}": -1}.get(text[j], 0)
            j += 1
        i = j
    out.append(text[i:])
    return "".join(out)


def produced_codes():
    codes = {}
    for p in rust_sources():
        text = strip_tests(p.read_text(encoding="utf-8"))
        rel = p.relative_to(ROOT).as_posix()
        for m in CONSTRUCTOR.finditer(text):
            codes.setdefault(m.group(1), rel)
        # Messages the kernel shows itself (`ui.message.shown` payloads such as `prefs.corrupt`).
        for m in MESSAGE_ID.finditer(text):
            codes.setdefault(m.group(1), rel)
        # Constants count only in code modules (`mod codes` or a `codes.rs` file).
        if rel.endswith("/codes.rs") or "pub mod codes" in text:
            blocks = re.findall(r"pub mod codes\s*\{(.*?)\n\}", text, re.S) or ([text] if rel.endswith("/codes.rs") else [])
            for block in blocks:
                for m in CONSTANT.finditer(block):
                    codes.setdefault(m.group(1), rel)
    return codes


def _value_of(text, start):
    """The expression after `messageId:` at `start`, to the `,` / `}` / `)` / newline that ends it at depth 0."""
    depth, i, quote = 0, start, None
    while i < len(text):
        c = text[i]
        if quote:
            if c == "\\":
                i += 1
            elif c == quote:
                quote = None
        elif c in "'\"`":
            quote = c
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            if depth == 0:
                break
            depth -= 1
        elif c in ",;\n" and depth == 0:
            break
        i += 1
    return text[start:i].strip()


#: Values that forward an id produced elsewhere (collected at its source).
FORWARD = re.compile(r"^[\w$.?\[\]'\"]*(?:\.messageId|\['messageId'\]|\[\"messageId\"\]|\.code)$")


def frontend_ids(unresolved=None):
    """Front-end message ids; values that cannot be resolved are appended to `unresolved` as (file, value)."""
    ids = {}
    for p in frontend_sources():
        rel = p.relative_to(ROOT).as_posix()
        text = p.read_text(encoding="utf-8")
        consts = dict(re.findall(r"\bconst\s+([A-Za-z_$][\w$]*)\s*(?::\s*string\s*)?=\s*['\"]([^'\"]+)['\"]", text))
        for m in re.finditer(r"\bmessageId\??:\s*", text):
            value = _value_of(text, m.end())
            if not value or value == "string" or value.startswith("{"):
                continue  # a type annotation or a schema
            literals = re.findall(r"'([^'\\]*)'|\"([^\"\\]*)\"", value)
            literal_ids = [a or b for a, b in literals if (a or b) != "string" and (a or b) != "messageId"]
            for lit in literal_ids:
                ids.setdefault(lit, rel)
            forwarded = re.sub(r"\[\s*['\"]messageId['\"]\s*\]", ".messageId", value)
            bare = re.sub(r"'[^']*'|\"[^\"]*\"", "''", forwarded)
            # A ternary's condition is not a value: only its branches (and `??` alternatives) are.
            branches = re.split(r"\s:\s", re.split(r"\s\?\s", bare, maxsplit=1)[-1])
            parts = [x.strip() for b in branches for x in b.split("??")]
            for part in parts:
                if part in ("", "''"):
                    continue
                if part in consts:
                    ids.setdefault(consts[part], rel)
                elif FORWARD.match(part):
                    continue
                elif unresolved is not None:
                    unresolved.append((rel, value))
    return ids


def host_notices():
    ids = {}
    for p in (ROOT / "desktop/src-tauri/src").rglob("*.rs"):
        for m in re.finditer(r"notice\(\s*&?\w+\s*,\s*" + LITERAL, p.read_text(encoding="utf-8")):
            ids.setdefault(m.group(1), p.relative_to(ROOT).as_posix())
    return ids


def catalogue():
    text = (ROOT / "crates/executor/src/help.rs").read_text(encoding="utf-8")
    block = text.split("pub(crate) const MESSAGES", 1)[1].split("];", 1)[0]
    tips = (ROOT / "crates/executor/src/commands/ui_shell.rs").read_text(encoding="utf-8")
    tips_block = tips.split("pub(crate) const TIPS", 1)[1].split("];", 1)[0] if "pub(crate) const TIPS" in tips else ""
    return set(re.findall(r"\(\s*\"([A-Za-z][\w.]*)\"\s*,", block + tips_block))


def problems(need, unresolved, known):
    """Missing help, codes outside the grammar, unresolved front-end values."""
    missing = sorted(k for k in need if k not in known)
    bad = sorted(k for k in need if not GRAMMAR.fullmatch(k))
    return missing, bad, unresolved


def main():
    known = catalogue()
    need, unresolved = {}, []
    for source in (produced_codes(), frontend_ids(unresolved), host_notices()):
        for k, v in source.items():
            need.setdefault(k, v)
    missing, bad, unresolved = problems(need, unresolved, known)
    if "--list" in sys.argv:
        for k in sorted(need):
            print(("ok   " if k in known else "MISS ") + k + "  (" + need[k] + ")")
    print(f"{len(need)} message ids, {len(need) - len(missing)} with help, {len(missing)} missing, "
          f"{len(bad)} outside the code grammar, {len(unresolved)} unresolved front-end values")
    for k in missing:
        print(f"  missing help: {k}  (first seen in {need[k]})")
    for k in bad:
        print(f"  not a code (grammar): {k!r}  (first seen in {need[k]})")
    for rel, value in unresolved:
        print(f"  unresolved messageId value in {rel}: {value}")
    return 1 if (missing or bad or unresolved) else 0


if __name__ == "__main__":
    sys.exit(main())
