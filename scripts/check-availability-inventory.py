"""Availability completeness: every refusal a GUI command's handler can return is classified (M4 C4.1, ruling O-24).

Availability (ADR-0021 decision 9) evaluates a command's static checks and its declared precheck. It is complete when
every refusal the handler can give for a state reason is also returned by the precheck. This script makes that
mechanical:

1. **Commands in scope:** every implemented command (a `#[command(...)]` handler in crates/executor/src) that is
   not agent-only and not an event, i.e. every command a GUI surface can offer (menus, toolbars, the tool rail, key
   dispatch, the command palette).
2. **Refusal codes of a handler:** every error code written at an error constructor site in the handler's body or in
   any executor function it reaches through calls (transitively): `CommandError::new` / `with_details` with a literal
   or a named constant, `ModelError::new` / `with_details`, `CommandError::internal` (`internal`),
   `CommandError::not_implemented` (`command.notImplemented`), `Disabled::because` (`command.disabled`),
   `no_project()` (`project.none`), and `codes::NAME` constants at those sites; a code passed to a function that
   forwards it to a constructor (`draw::err(code, ...)`, to a fixpoint), and a code chosen locally among literals
   (`let code = match ... { ... => "name.empty" }`). Codes the command's static checks return (`project.none` of
   `needs_project`, the codes of its enablement rule) are answered before the handler runs and are not listed.
3. **The table:** docs/availability-inventory.md classifies each (command, code) pair, in a row of its own or in a
   `*` row that classifies the code for every command (a row of its own takes precedence):
   - `parameter`: the refusal depends on what the caller supplies (an explicit ID, a value out of range, the caller's
     role);
   - `precheck`: the declared precheck returns it (checked: the code is reachable from the precheck); the row names a
     state in which it occurs and one in which it does not, which the sweep in
     crates/executor/tests/c41/availability.rs builds, or says why the state cannot be built;
   - `asks-user`: one of the two refusals that keep the reference enabled (`project.pathRequired`,
     `project.unsavedChanges`, 01 §3.5);
   - `execution`: an outcome of doing the work that no state check can predict (a file system failure, an internal
     invariant, a cancelled job, a work budget reached during the work);
   - `deferred <checkpoint>`: a state refusal whose state needs a view or facility that does not exist yet; the owner
     is the checkpoint that builds it;
   - `unreachable`: the scanner's over-approximation; the reason names the guard that makes the path impossible.
   Every row has a one-line reason.
4. **Failures:** a pair the table lacks; a stale row (the scanner no longer finds its pair; a `*` row no pair uses);
   a `precheck` row whose code the command's precheck cannot reach, or without states; an unknown class; a
   `deferred` row without an owner; an `asks-user` row for another code.

**Limits.** This is a text scanner over the executor crate:
- It follows calls by name: a bare call resolves to the function of that name in the same module or imported by a
  `use` (globs included); a path call (`super::draw::f`, `crate::cx::f`) to that module's function; `Self::f` /
  `Type::f` to that impl's function; a function passed as a value (`.ok_or_else(no_target)`) counts as a call. A
  method call `.f(...)` resolves by the receiver where its name fixes its type (`self`, `cx`, `ui`, `env`, ...; `doc`,
  `before`, `after`, `sc`, `dr` hold model types, which have no executor methods), else to the executor methods named
  `f` of the types the calling file names (never the engine's own types, which a handler cannot hold). Every
  resolution requires the callee's parameter count to equal the call's argument count (Rust has no default or
  variadic arguments). A call whose result is dropped (`let _ = f(...)`) passes no refusal on. These rules
  over-approximate (the extra pairs are classified, often as `unreachable`); a method call on a value whose type the
  calling file never names, through a receiver whose name does not fix its type, is the case they can miss.
- It does not follow calls into other crates (the model, persistence, rendering crates): a code those crates build
  is not collected. The executor maps their errors to its own codes where the handler checks a state itself; the
  dry-run sweeps of crates/executor/tests/c41/availability.rs are the dynamic check of the remainder.
- A code built at run time (formatted or passed in from elsewhere) is not seen (as in check-message-help.py).
- Reachability from the precheck is reachability of the code, not of every cause of it: where a precheck covers a
  state refusal in part (an empty selection, not the kinds of the selected objects), the row's reason says so.

Usage: python scripts/check-availability-inventory.py [--list] [--missing]
  --list     print every pair with its first site (function, file:line)
  --missing  print table rows (class `?`) for the pairs the table lacks
Exit 1 with the problems when the inventory is incomplete or inconsistent. Standard library only.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "crates" / "executor" / "src"
TABLE = ROOT / "docs" / "availability-inventory.md"
REGISTRY = ROOT / "crates" / "registry" / "src" / "lib.rs"
CLASSES = {"parameter", "precheck", "asks-user", "execution", "deferred", "unreachable"}
ASKS_USER = {"project.pathRequired", "project.unsavedChanges"}
KEYWORDS = {
    "if", "while", "for", "match", "return", "loop", "fn", "let", "in", "as", "move", "ref", "mut", "else", "where",
    "Some", "Ok", "Err", "None", "Box", "Vec", "String", "Arc", "Rc", "matches", "assert", "debug_assert", "format",
    "json", "vec", "write", "writeln", "println", "eprintln", "panic", "unreachable", "todo", "impl", "dyn", "pub",
    "crate", "super", "self", "Self", "use", "mod", "struct", "enum", "type", "const", "static", "unsafe", "async",
    "await", "break", "continue", "true", "false",
}


# ----- lexing ---------------------------------------------------------------------------------------------------------


def mask(text):
    """(masked, literals): comments blanked and string/char literal contents replaced by spaces (positions kept);
    `literals` maps the offset of each string literal's opening quote to its value."""
    out = list(text)
    lits = {}
    i, n = 0, len(text)

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif (c == "r" or (c == "b" and i + 1 < n and text[i + 1] == "r")) and re.match(r'b?r#*"', text[i:]) and (
            i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")
        ):
            m = re.match(r'b?r(#*)"', text[i:])
            hashes = m.group(1)
            start = i + m.end()
            end = text.find('"' + hashes, start)
            end = n if end < 0 else end
            lits[start - 1] = text[start:end]
            blank(start, end)
            i = end + 1 + len(hashes)
        elif c == '"' or (c == "b" and i + 1 < n and text[i + 1] == '"' and not (i and (text[i - 1].isalnum() or text[i - 1] == "_"))):
            q = i if c == '"' else i + 1
            j = q + 1
            buf = []
            while j < n and text[j] != '"':
                if text[j] == "\\" and j + 1 < n:
                    buf.append(text[j : j + 2])
                    j += 2
                else:
                    buf.append(text[j])
                    j += 1
            lits[q] = "".join(buf)
            blank(q + 1, j)
            i = j + 1
        elif c == "'":
            # A char literal ('a', '\n', '\u{..}') or a lifetime ('a): a char literal closes within a few characters.
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'", text[i:])
            if m:
                blank(i + 1, i + m.end() - 1)
                i += m.end()
            else:
                i += 1
        else:
            i += 1
    return "".join(out), lits


def strip_tests(masked):
    """Blanks `#[cfg(test)] mod ... { ... }` blocks."""
    out = list(masked)
    for m in re.finditer(r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", masked):
        depth, j = 1, m.end()
        while j < len(masked) and depth:
            depth += {"{": 1, "}": -1}.get(masked[j], 0)
            j += 1
        for k in range(m.start(), j):
            if out[k] != "\n":
                out[k] = " "
    return "".join(out)


def close(masked, open_at, o="{", c="}"):
    depth, j = 1, open_at + 1
    while j < len(masked) and depth:
        depth += {o: 1, c: -1}.get(masked[j], 0)
        j += 1
    return j


# ----- the index ------------------------------------------------------------------------------------------------------


class Fn:
    def __init__(self, module, name, impl, file, start, end, line):
        self.module, self.name, self.impl, self.file = module, name, impl, file
        self.start, self.end, self.line = start, end, line
        self.calls, self.codes = set(), {}
        #: Parameter names (without `self`), the call sites `(callee, offset of its "(")`, and the index of the
        #: parameter whose value this function passes on as an error code (a forwarder), if any.
        self.params, self.sites, self.forwards = [], [], None

    @property
    def key(self):
        return f"{self.module}::{self.impl + '::' if self.impl else ''}{self.name}"


def module_of(path):
    rel = path.relative_to(SRC).with_suffix("").as_posix()
    if rel.endswith("/mod"):
        rel = rel[: -len("/mod")]
    return rel.replace("/", "::")


def registry_constants():
    text = REGISTRY.read_text(encoding="utf-8")
    return dict(re.findall(r'pub const ([A-Z][A-Z0-9_]*): &str = "([^"]+)"', text))


def parse_uses(masked, module):
    """local name -> fully qualified path (module path for a module alias, `module::name` for an item); and the
    modules glob-imported."""
    names, globs = {}, []
    for m in re.finditer(r"\buse\s+([^;]+);", masked):
        expand(m.group(1).replace("\n", " ").replace(" ", ""), "", names, globs, module)
    return names, globs


def absolute(path, module):
    parts = path.split("::")
    base = module.split("::")
    if parts[0] == "crate":
        return "::".join(parts[1:])
    if parts[0] == "super":
        up = base[:-1]
        parts = parts[1:]
        while parts and parts[0] == "super":
            up, parts = up[:-1], parts[1:]
        return "::".join(up + parts)
    if parts[0] == "self":
        return "::".join(base + parts[1:])
    return path


def expand(spec, prefix, names, globs, module):
    if "{" in spec:
        head, rest = spec.split("{", 1)
        inner = rest[: rest.rfind("}")]
        depth, cur, items = 0, "", []
        for ch in inner:
            if ch == "," and depth == 0:
                items.append(cur)
                cur = ""
                continue
            depth += {"{": 1, "}": -1}.get(ch, 0)
            cur += ch
        items.append(cur)
        for it in items:
            if it:
                expand(it, prefix + head, names, globs, module)
        return
    full = prefix + spec
    if full.endswith("::*"):
        globs.append(absolute(full[:-3], module))
        return
    alias = None
    if " as " in full or "as" in full.split("::")[-1]:
        m = re.match(r"(.+?)as(\w+)$", full)
        if m and m.group(1).endswith(tuple("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_")):
            full, alias = m.group(1), m.group(2)
    full = full.rstrip(":")
    if full.endswith("::self"):
        full = full[: -len("::self")]
    local = alias or full.split("::")[-1]
    names[local] = absolute(full, module)


def build_index():
    consts = registry_constants()
    fns, by_name, by_module, files = [], {}, {}, {}
    for path in sorted(SRC.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        masked, lits = mask(text)
        masked = strip_tests(masked)
        module = module_of(path)
        rel = path.relative_to(ROOT).as_posix()
        uses, globs = parse_uses(masked, module)
        # Local string constants, from the original text (the masked text has no literal contents).
        local_consts = dict(re.findall(r'\bconst ([A-Z][A-Z0-9_]*)\s*:\s*&(?:\'static\s+)?str\s*=\s*"([^"]*)"', text))
        impls = []
        for m in re.finditer(r"(?m)^\s*impl\b([^{;]*)\{", masked):
            header = m.group(1)
            t = re.search(r"\bfor\s+([A-Za-z_]\w*)", header) or re.search(r"([A-Za-z_]\w*)\s*(?:<[^{]*>)?\s*$", header.strip())
            impls.append((m.end() - 1, close(masked, m.end() - 1), t.group(1) if t else "?"))
        files[rel] = (text, masked, lits, module, uses, globs, local_consts)
        for m in re.finditer(r"\bfn\s+([A-Za-z_]\w*)", masked):
            # The body: the first `{` after the signature at depth 0 of parentheses (a `;` first: no body).
            j, depth = m.end(), 0
            while j < len(masked):
                ch = masked[j]
                if ch in "([":
                    depth += 1
                elif ch in ")]":
                    depth -= 1
                elif ch == ";" and depth == 0:
                    j = -1
                    break
                elif ch == "{" and depth == 0:
                    break
                j += 1
            if j < 0 or j >= len(masked):
                continue
            end = close(masked, j)
            impl = next((t for a, b, t in impls if a < m.start() < b), None)
            f = Fn(module, m.group(1), impl, rel, j, end, text.count("\n", 0, m.start()) + 1)
            # The parameter list: after the generics (which can hold `Fn(...)` bounds).
            g = m.end()
            while g < j and masked[g].isspace():
                g += 1
            if g < j and masked[g] == "<":
                depth = 0
                while g < j:
                    depth += {"<": 1, ">": -1}.get(masked[g], 0)
                    g += 1
                    if depth == 0:
                        break
            po = masked.find("(", g, j)
            if po >= 0:
                for a, b in args(masked, po):
                    text_arg = masked[a:b]
                    if re.fullmatch(r"\s*&?\s*(?:'\w+\s+)?(?:mut\s+)?self\s*(?::.*)?", text_arg, re.S):
                        continue
                    # Every other parameter counts for the arity; a destructuring pattern has no name ("_").
                    pm = re.match(r"\s*(?:mut\s+)?([a-z_]\w*)\s*:", text_arg)
                    f.params.append(pm.group(1) if pm else "_")
            fns.append(f)
            by_name.setdefault(f.name, []).append(f)
            by_module.setdefault((module, f.name), []).append(f)
    return fns, by_name, by_module, files, consts


CONSTRUCTOR_LIT = re.compile(
    r"(?:(?:CommandError|ModelError)::(?:new|with_details)\(\s*|\bDisabled\s*\{\s*code:\s*|\"code\"\s*:\s*)\""
)
CONSTRUCTOR_NAME = re.compile(
    r"(?:(?:CommandError|ModelError)::(?:new|with_details)\(\s*|\bDisabled\s*\{\s*code:\s*)"
    r"((?:[A-Za-z_]\w*::)*)([A-Z][A-Z0-9_]*)\b"
)
HELPERS = [
    (re.compile(r"\bCommandError::internal\s*\("), "internal"),
    (re.compile(r"\bCommandError::not_implemented\s*\("), "command.notImplemented"),
    (re.compile(r"\bDisabled::because\s*\("), "command.disabled"),
    (re.compile(r"\bDisabled::no_project\s*\("), "project.none"),
]
CALL = re.compile(r"(?<![\w.])((?:[A-Za-z_]\w*::)*)([A-Za-z_]\w*)\s*(?:::<[^()]*>)?\s*\(")
METHOD = re.compile(r"\.\s*([A-Za-z_]\w*)\s*(?:::<[^()]*>)?\s*\(")
VALUE = re.compile(r"[(,]\s*((?:[A-Za-z_]\w*::)*)([a-z_]\w*)\s*(?=[),])")


def link(fns, by_name, by_module, files, consts):
    methods = {}
    for f in fns:
        if f.impl:
            methods.setdefault(f.name, []).append(f)

    def resolve(path, name, f, uses, globs, as_value=False):
        if name in KEYWORDS:
            return []
        if path:
            segs = path.rstrip(":").split("::")
            segs = [s for s in segs if s]
            if segs and segs[0] == "Self":
                return [g for g in by_name.get(name, []) if g.impl == f.impl and g.module == f.module]
            head = segs[0]
            if head in uses:
                segs = uses[head].split("::") + segs[1:]
                mod = "::".join(segs)
            else:
                mod = absolute("::".join(segs), f.module)
            hit = by_module.get((mod, name))
            if hit:
                return hit
            # `Type::f` (an associated function): the impl of that type.
            last = segs[-1]
            if last[:1].isupper():
                return [g for g in by_name.get(name, []) if g.impl == last]
            return []
        hit = by_module.get((f.module, name))
        if hit:
            return [g for g in hit if not g.impl] or ([] if as_value else hit)
        if name in uses:
            full = uses[name]
            mod, _, item = full.rpartition("::")
            return by_module.get((mod, item), [])
        for g in globs:
            hit = by_module.get((g, name))
            if hit:
                return hit
        return []

    for f in fns:
        text, masked, lits, module, uses, globs, local_consts = files[f.file]
        body = masked[f.start : f.end]
        named = set(re.findall(r"\b[A-Z][A-Za-z0-9_]*\b", masked))
        for m in CALL.finditer(body):
            if discarded(body, m.start()):
                continue
            n = len(args(masked, f.start + m.end() - 1))
            for g in resolve(m.group(1), m.group(2), f, uses, globs):
                # Rust has no default or variadic arguments: a callee with another parameter count is not this one.
                if g is not f and len(g.params) == n:
                    f.calls.add(g)
                    f.sites.append((g, f.start + m.end() - 1))
        for m in METHOD.finditer(body):
            if discarded(body, m.start()):
                continue
            candidates = methods.get(m.group(1), [])
            recv = re.search(r"([A-Za-z_]\w*|\)|\])\s*$", body[: m.start()])
            recv = recv.group(1) if recv else ")"
            typed = f.impl if recv == "self" else RECEIVERS.get(recv)
            narrowed = [g for g in candidates if g.impl == typed] if typed else []
            if typed in MODEL_TYPES:
                narrowed = []
            elif not narrowed:
                # An unknown receiver: the methods of the types this file names (or defines), never the engine's.
                narrowed = [
                    g for g in candidates
                    if (g.impl not in ENGINE or f.impl in ENGINE) and (g.impl in named or g.file == f.file)
                ]
            n = len(args(masked, f.start + m.end() - 1))
            for g in narrowed:
                if g is not f and len(g.params) == n:
                    f.calls.add(g)
                    f.sites.append((g, f.start + m.end() - 1))
        for m in VALUE.finditer(body):
            for g in resolve(m.group(1), m.group(2), f, uses, globs, as_value=True):
                if g is not f:
                    f.calls.add(g)
        for m in CONSTRUCTOR_LIT.finditer(body):
            q = f.start + m.end() - 1
            if q in lits:
                f.codes.setdefault(lits[q], text.count("\n", 0, q) + 1)
        for m in CONSTRUCTOR_NAME.finditer(body):
            name = m.group(2)
            value = (local_consts.get(name) if not m.group(1) else None) or consts.get(name)
            if value:
                f.codes.setdefault(value, text.count("\n", 0, f.start + m.start()) + 1)
        for rx, code in HELPERS:
            for m in rx.finditer(body):
                f.codes.setdefault(code, text.count("\n", 0, f.start + m.start()) + 1)
        if f.module != "cx" or f.name != "no_project":
            for m in re.finditer(r"(?<![\w.])(?:crate::cx::|cx::)?no_project\s*\(", body):
                f.codes.setdefault("project.none", text.count("\n", 0, f.start + m.start()) + 1)
        # A constructor whose code is a variable: a parameter (the function forwards a code, `draw::err(code, ...)`)
        # or a local chosen among literals (`let code = match rule { "empty" => "name.empty", ... }`).
        for m in CONSTRUCTOR_VAR.finditer(body):
            take(f, m.group(1), f.start + m.start(), files)
    # Forwarders of forwarders, to a fixpoint: a call passing a literal, a constant, a parameter or a local at the
    # forwarded position.
    changed = True
    while changed:
        changed = False
        for f in fns:
            text, masked, lits, module, uses, globs, local_consts = files[f.file]
            for g, open_at in f.sites:
                if g.forwards is None:
                    continue
                spans = args(masked, open_at)
                if g.forwards >= len(spans):
                    continue
                a, b = spans[g.forwards]
                raw = masked[a:b].strip()
                lead = a + (len(masked[a:b]) - len(masked[a:b].lstrip()))
                line = text.count("\n", 0, a) + 1
                if raw.startswith('"') and lead in lits:
                    if lits[lead] not in f.codes:
                        f.codes[lits[lead]] = line
                        changed = True
                elif re.fullmatch(r"(?:[A-Za-z_]\w*::)*[A-Z][A-Z0-9_]*", raw):
                    name = raw.split("::")[-1]
                    value = local_consts.get(name) or consts.get(name)
                    if value and value not in f.codes:
                        f.codes[value] = line
                        changed = True
                elif re.fullmatch(r"[a-z_]\w*", raw):
                    before = (f.forwards, len(f.codes))
                    take(f, raw, a, files)
                    changed = changed or before != (f.forwards, len(f.codes))


#: Receiver names whose type is fixed by convention in the executor (a method call on them resolves to that type's
#: method when it has one).
RECEIVERS = {"cx": "Cx", "ui": "UiState", "env": "Env", "session": "Session", "core": "Core", "work": "Budget",
             "budget": "Budget", "column": "Column", "col": "Column", "hub": "AgentHub", "jobs": "JobTable"}
#: Receivers that hold model types (another crate): a method call on them is never an executor method.
MODEL_TYPES = {"Document", "Scene", "Drawing"}
RECEIVERS.update({"doc": "Document", "before": "Document", "after": "Document", "sc": "Scene", "dr": "Drawing"})
#: The engine's own types: a command handler holds a `&mut Cx`, never one of these, so a method call on an unknown
#: receiver inside a handler is not resolved to them.
ENGINE = {"Core", "ExecutorHandle", "Poster", "RangeJob", "ShutdownGuard", "LiveToken"}

def discarded(body, at):
    """Whether the call at `at` is the value of a `let _ = ...;` statement (its result, and any refusal, dropped)."""
    start = max(body.rfind(";", 0, at), body.rfind("{", 0, at), body.rfind("}", 0, at)) + 1
    return re.fullmatch(r"\s*let\s+_\s*=\s*[\w.:()&\s]*", body[start:at]) is not None


CONSTRUCTOR_VAR = re.compile(r"(?:CommandError|ModelError)::(?:new|with_details)\(\s*([a-z_]\w*)\b(?!\s*[.(:\[])")


def args(masked, open_at):
    """The argument spans of the call whose `(` is at `open_at` (depth-0 commas)."""
    spans, depth, start, j = [], 0, open_at + 1, open_at + 1
    while j < len(masked):
        ch = masked[j]
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            if depth == 0:
                if masked[start:j].strip():
                    spans.append((start, j))
                return spans
            depth -= 1
        elif ch == "," and depth == 0:
            spans.append((start, j))
            start = j + 1
        j += 1
    return spans


def take(f, ident, at, files):
    """`ident` carries a code inside `f`: a parameter makes `f` a forwarder; a local's literals are codes."""
    text, masked, lits, *_ = files[f.file]
    if ident in f.params:
        if f.forwards is None:
            f.forwards = f.params.index(ident)
        return
    m = None
    for m2 in re.finditer(r"\blet\s+" + re.escape(ident) + r"\b[^=]*=", masked[f.start : at]):
        m = m2
    if not m:
        return
    a = f.start + m.end()
    j, depth = a, 0
    while j < len(masked) and not (masked[j] == ";" and depth == 0):
        depth += {"(": 1, "[": 1, "{": 1, ")": -1, "]": -1, "}": -1}.get(masked[j], 0)
        j += 1
    for q, value in lits.items():
        if a <= q < j and GRAMMAR.fullmatch(value) and "." in value:
            # Inside a `match`, only the arm values (after `=>`), not the patterns.
            seg = masked[a:q]
            if "match" in masked[a:j] and not seg.rstrip().endswith("=>"):
                continue
            f.codes.setdefault(value, text.count("\n", 0, q) + 1)


GRAMMAR = re.compile(r"[a-z][A-Za-z0-9]*(?:\.[a-z][A-Za-z0-9]*)*")


def reach(f, memo):
    """Every code reachable from `f`: code -> (function key, file, line) of a site."""
    if f in memo:
        return memo[f]
    memo[f] = {}
    out = {}
    stack, seen = [f], {f}
    while stack:
        g = stack.pop()
        for code, line in g.codes.items():
            out.setdefault(code, (g.key, g.file, line))
        for h in g.calls:
            if h not in seen:
                seen.add(h)
                stack.append(h)
    memo[f] = out
    return out


# ----- commands -------------------------------------------------------------------------------------------------------


def commands(fns, files):
    """[(command id, handler Fn, precheck Fn | None)] for every non-agent-only implemented command."""
    out = []
    fn_at = {(f.file, f.start): f for f in fns}
    starts = {}
    for f in fns:
        starts.setdefault(f.file, []).append(f)
    for rel, (text, masked, lits, module, uses, globs, local_consts) in files.items():
        for m in re.finditer(r"#\[(?:tt_registry::)?command\(", masked):
            end = close(masked, m.end() - 1, "(", ")")
            attr = text[m.start() : end]
            cid = re.search(r'\bid\s*=\s*"([^"]+)"', attr)
            if not cid or re.search(r"\bagent_only\s*=", attr) or re.search(r"\bkind\s*=\s*event\b", attr):
                continue
            handler = min((f for f in starts.get(rel, []) if f.start > end and not f.impl), key=lambda f: f.start, default=None)
            if handler is None:
                continue
            pre = None
            pm = re.search(r"\bprecheck\s*=\s*([A-Za-z_][\w:]*)", attr)
            if pm:
                path = pm.group(1)
                name = path.split("::")[-1]
                mod = "::".join(path.split("::")[:-1])
                mod = absolute(mod, module) if mod else module
                if mod in uses:
                    mod = uses[mod]
                pre = next((f for f in fns if f.module == mod and f.name == name), None)
                if pre is None:
                    pre = next((f for f in fns if f.name == name and f.file == rel), None)
            out.append((cid.group(1), handler, pre))
            STATIC[cid.group(1)] = static_codes(attr, fns)
    return sorted(out, key=lambda c: c[0])


#: The codes each command's static checks return (`needs_project`, its enablement rule): availability evaluates those
#: checks for every reference, so a handler's refusal with one of these codes is answered before the handler runs.
STATIC = {}
#: Enablement rules that refuse without an open project although their function does not return `project.none`
#: (a current scene exists only in an open project).
RULES_NEEDING_A_PROJECT = {"HAS_SCENE"}


def static_codes(attr, fns):
    codes = set()
    if re.search(r"\bneeds_project\b", attr):
        codes.add("project.none")
    rm = re.search(r"\benablement\s*=\s*(?:rules::)?([A-Z][A-Z0-9_]*)", attr)
    if rm:
        rules = (SRC / "rules.rs").read_text(encoding="utf-8")
        cm = re.search(r"const " + rm.group(1) + r"\b[^;]*?check:\s*Some\((\w+)\)", rules, re.S)
        check = next((f for f in fns if f.module == "rules" and cm and f.name == cm.group(1)), None)
        if check is not None:
            codes |= set(reach(check, {}))
        if rm.group(1) in RULES_NEEDING_A_PROJECT:
            codes.add("project.none")
    return codes


# ----- the table ------------------------------------------------------------------------------------------------------

ROW = re.compile(
    r"^\|\s*`([^`]+)`\s*\|\s*`([^`]+)`\s*\|\s*([^|]+?)\s*\|\s*([^|]*?)\s*\|(?:\s*([^|]*?)\s*\|)?\s*$"
)
#: The states of a `precheck` row: the sweep builds both (crates/executor/tests/c41/availability.rs), or a reason why
#: it cannot.
STATES = re.compile(r"refuse: [a-z0-9+-]+(?: \{.*\})? · pass: [a-z0-9+-]+(?: \{.*\})?|untestable: .+")


def read_table():
    rows, problems = {}, []
    if not TABLE.exists():
        return rows, [f"{TABLE.relative_to(ROOT).as_posix()} is missing"]
    for n, line in enumerate(TABLE.read_text(encoding="utf-8").splitlines(), 1):
        m = ROW.match(line)
        if not m or m.group(1) == "command":
            continue
        cid, code, cls, reason, states = m.groups()
        states = states or ""
        kind = cls.split()[0]
        if kind not in CLASSES:
            problems.append(f"docs/availability-inventory.md:{n}: unknown class `{cls}`")
        if kind == "deferred" and len(cls.split()) < 2:
            problems.append(f"docs/availability-inventory.md:{n}: `deferred` without an owner checkpoint")
        if kind == "asks-user" and code not in ASKS_USER:
            problems.append(f"docs/availability-inventory.md:{n}: `{code}` is not one of the refusals that ask the user")
        if kind == "precheck" and not STATES.fullmatch(states):
            problems.append(f"docs/availability-inventory.md:{n}: a precheck row names its states or why they cannot be built")
        if kind == "precheck" and cid == "*":
            problems.append(f"docs/availability-inventory.md:{n}: a precheck row names its command")
        if not reason:
            problems.append(f"docs/availability-inventory.md:{n}: no reason")
        if (cid, code) in rows:
            problems.append(f"docs/availability-inventory.md:{n}: duplicate row {cid} {code}")
        rows[(cid, code)] = (kind, cls, reason, n)
    return rows, problems


def classify(rows, cid, code):
    """The row that classifies a pair: its own, else the `*` row of its code."""
    return rows.get((cid, code)) or rows.get(("*", code))


def main(argv):
    fns, by_name, by_module, files, consts = build_index()
    link(fns, by_name, by_module, files, consts)
    memo = {}
    cmds = commands(fns, files)
    found = {}
    pre_codes = {}
    declared = {cid for cid, _, pre in cmds if pre is not None}
    for cid, handler, pre in cmds:
        for code, site in reach(handler, memo).items():
            if code not in STATIC.get(cid, set()):
                found[(cid, code)] = site
        pre_codes[cid] = set(reach(pre, memo)) if pre else set()
    if "--list" in argv:
        for (cid, code), (key, rel, line) in sorted(found.items()):
            print(f"{cid}\t{code}\t{key}\t{rel}:{line}")
        return 0
    rows, problems = read_table()
    missing = sorted(k for k in found if classify(rows, *k) is None)
    if "--missing" in argv:
        for cid, code in missing:
            key, rel, line = found[(cid, code)]
            print(f"| `{cid}` | `{code}` | ? | {rel}:{line} ({key}) | |")
        return 0
    for cid, code in missing:
        key, rel, line = found[(cid, code)]
        problems.append(f"missing from the table: {cid} {code} (returned at {rel}:{line}, {key})")
    used_wild = {code for (cid, code) in found if (cid, code) not in rows and ("*", code) in rows}
    for (cid, code), (kind, cls, reason, n) in sorted(rows.items(), key=lambda r: r[1][3]):
        if cid == "*":
            if code not in used_wild:
                problems.append(f"docs/availability-inventory.md:{n}: stale row * {code} (no pair uses it)")
        elif (cid, code) not in found:
            problems.append(f"docs/availability-inventory.md:{n}: stale row {cid} {code} (the scanner no longer finds it)")
        elif kind == "precheck" and code not in pre_codes.get(cid, set()):
            why = "cannot reach that code" if cid in declared else "is not declared"
            problems.append(f"docs/availability-inventory.md:{n}: {cid} {code} is marked precheck, but the command's precheck {why}")
    counts = {}
    for k in found:
        row = classify(rows, *k)
        if row:
            counts[row[0]] = counts.get(row[0], 0) + 1
    print(
        f"{len(cmds)} commands in scope, {len(found)} (command, code) pairs; "
        + ", ".join(f"{v} {k}" for k, v in sorted(counts.items()))
    )
    if problems:
        print("\n".join(problems))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
