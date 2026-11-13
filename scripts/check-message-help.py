"""Every message the user or an agent can be shown has help content (M4 C4.1 round 1, ruling O-11).

The message id is the agent error code (01 §9.5), so the set to cover is mechanical:
1. every error code the product can produce: the `pub const NAME: &str = "a.b"` constants of the code modules,
   every string literal passed as the code of an error constructor (`CommandError::new` / `with_details`,
   `ModelError::new` / `with_details`, `Disabled { code: ... }`) and every `"messageId": "..."` the kernel emits, in
   the Rust product sources;
2. every message id the front end can show: `messageId: '...'` literals and fallbacks (`e?.code ?? '...'`) in
   desktop/src (tests excluded);
3. every notice the desktop host sends (`notice(app, "id", ...)`).

The help catalogue is `MESSAGES` in crates/executor/src/help.rs plus the suppressible tips (`TIPS`) in
crates/executor/src/commands/ui_shell.rs; `help::message` serves both (`ui.help.get {messageId}`).

Usage: python scripts/check-message-help.py [--list]   (exit 1 and the missing ids when coverage is incomplete)
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CODE = r"[a-z][A-Za-z0-9]*(?:\.[a-z][A-Za-z0-9]*)+"


def rust_sources():
    for base in ["crates", "desktop/src-tauri/src", "bins"]:
        for p in (ROOT / base).rglob("*.rs"):
            rel = p.relative_to(ROOT).as_posix()
            if "/tests/" in rel or "/benches/" in rel or "/examples/" in rel or rel.startswith("crates/registry/src/declared"):
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
    constructors = re.compile(
        r"(?:CommandError|ModelError)::(?:new|with_details)\(\s*\"(" + CODE + r")\"|Disabled\s*\{\s*code:\s*\"(" + CODE + r")\""
    )
    constants = re.compile(r"pub(?:\(crate\))?\s+const\s+[A-Z0-9_]+\s*:\s*&str\s*=\s*\"(" + CODE + r")\"")
    for p in rust_sources():
        text = strip_tests(p.read_text(encoding="utf-8"))
        rel = p.relative_to(ROOT).as_posix()
        for m in constructors.finditer(text):
            codes.setdefault(m.group(1) or m.group(2), rel)
        # Messages the kernel shows itself (`ui.message.shown` payloads such as `prefs.corrupt`).
        for m in re.finditer(r'"messageId":\s*"(' + CODE + r')"', text):
            codes.setdefault(m.group(1), rel)
        # Constants count only in code modules (`mod codes` or a `codes.rs` file).
        in_codes = rel.endswith("/codes.rs") or "pub mod codes" in text
        if in_codes:
            for block in re.findall(r"pub mod codes\s*\{(.*?)\n\}", text, re.S) or ([text] if rel.endswith("/codes.rs") else []):
                for m in constants.finditer(block):
                    codes.setdefault(m.group(1), rel)
    return codes


def frontend_ids():
    ids = {}
    pat = re.compile(r"messageId:\s*['\"](" + CODE + r"|[a-z]+)['\"]|\?\?\s*['\"](" + CODE + r"|[a-z]+)['\"]\s*,?\s*$", re.M)
    for p in (ROOT / "desktop/src").rglob("*.ts*"):
        rel = p.relative_to(ROOT).as_posix()
        if ".test." in rel or "/test/" in rel or "/test-support/" in rel or "/generated/" in rel or "/spikes/" in rel:
            continue
        text = p.read_text(encoding="utf-8")
        for m in re.finditer(r"messageId:\s*(?:[^'\",\n]*\?\?\s*)?['\"]([A-Za-z][\w.]*)['\"]", text):
            ids.setdefault(m.group(1), rel)
    return ids


def host_notices():
    ids = {}
    for p in (ROOT / "desktop/src-tauri/src").rglob("*.rs"):
        for m in re.finditer(r"notice\(\s*&?\w+\s*,\s*\"(" + CODE + r")\"", p.read_text(encoding="utf-8")):
            ids.setdefault(m.group(1), p.relative_to(ROOT).as_posix())
    return ids


def catalogue():
    text = (ROOT / "crates/executor/src/help.rs").read_text(encoding="utf-8")
    block = text.split("pub(crate) const MESSAGES", 1)[1].split("];", 1)[0]
    tips = (ROOT / "crates/executor/src/commands/ui_shell.rs").read_text(encoding="utf-8")
    tips_block = tips.split("pub(crate) const TIPS", 1)[1].split("];", 1)[0] if "pub(crate) const TIPS" in tips else ""
    return set(re.findall(r"\(\s*\"([A-Za-z][\w.]*)\"\s*,", block + tips_block))


def main():
    known = catalogue()
    need = {}
    for source in (produced_codes(), frontend_ids(), host_notices()):
        for k, v in source.items():
            need.setdefault(k, v)
    missing = sorted(k for k in need if k not in known)
    if "--list" in sys.argv:
        for k in sorted(need):
            print(("ok   " if k in known else "MISS ") + k + "  (" + need[k] + ")")
    print(f"{len(need)} message ids, {len(need) - len(missing)} with help, {len(missing)} missing")
    for k in missing:
        print(f"  missing help: {k}  (first seen in {need[k]})")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
