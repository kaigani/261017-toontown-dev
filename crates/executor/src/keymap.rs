//! The keymap (01 §8; M4 C4.1): canonical key sequences, the bundled default set, user overrides stored as deltas,
//! conflict and reserved-key rules, and the one dispatch resolution shared by real key presses and the MCP `key`
//! tool (ADR-0021).
//!
//! **Sequences** (01 §12.1 rule 17) are platform-neutral text: modifiers `Primary`, `Ctrl`, `Alt`, `Shift` joined
//! with `+` before the key name, canonical order `Primary+Ctrl+Alt+Shift+<Key>`, letters upper-case. On Windows and
//! Linux the physical Control key is `Primary`, so `Ctrl+Z` and `Primary+Z` are the same sequence there and are
//! written `Primary+Z`. Shifted punctuation is written as Shift and the unshifted key (`?` = `Shift+/`), so one
//! physical chord has one spelling.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::CommandError;

/// Error codes of the keymap (01 §12.1 rule 7).
pub mod codes {
    /// The command id is not registered (or is an event).
    pub const UNKNOWN_COMMAND: &str = "keymap.unknownCommand";
    /// The key sequence does not parse.
    pub const INVALID_SEQUENCE: &str = "keymap.invalidSequence";
    /// The sequence is reserved by the platform or the application.
    pub const RESERVED_KEY: &str = "keymap.reservedKey";
    /// The sequence is bound to another command in a conflicting context.
    pub const CONFLICT: &str = "keymap.conflict";
}

/// The platform whose conventions apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Windows.
    Windows,
    /// macOS.
    Macos,
    /// Linux and other Unix systems.
    Linux,
}

impl Platform {
    /// The platform this build runs on.
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }

    fn name(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Macos => "macos",
            Platform::Linux => "linux",
        }
    }
}

/// A parsed key sequence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sequence {
    /// The primary modifier (Command on macOS, Control elsewhere).
    pub primary: bool,
    /// The physical Control key on macOS (always false elsewhere: it is `primary` there).
    pub ctrl: bool,
    /// Alt / Option.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// The canonical key name.
    pub key: String,
}

/// Canonical names of non-character keys, with their accepted aliases (case-insensitive).
const NAMED: &[(&str, &[&str])] = &[
    ("Space", &["space", "spacebar"]),
    ("Enter", &["enter", "return"]),
    ("Tab", &["tab"]),
    ("Escape", &["escape", "esc"]),
    ("Backspace", &["backspace"]),
    ("Delete", &["delete", "del"]),
    ("Insert", &["insert", "ins"]),
    ("Home", &["home"]),
    ("End", &["end"]),
    ("PageUp", &["pageup", "pgup"]),
    ("PageDown", &["pagedown", "pgdn"]),
    ("Left", &["left", "arrowleft"]),
    ("Right", &["right", "arrowright"]),
    ("Up", &["up", "arrowup"]),
    ("Down", &["down", "arrowdown"]),
    (",", &["comma"]),
    (".", &["period"]),
    ("/", &["slash"]),
    ("\\", &["backslash"]),
    (";", &["semicolon"]),
    ("'", &["quote"]),
    ("[", &["bracketleft"]),
    ("]", &["bracketright"]),
    ("-", &["minus"]),
    ("=", &["equal"]),
    ("`", &["backquote"]),
];

/// Shifted characters of the reference (US) layout: the character → its unshifted key.
const SHIFTED: &[(char, &str)] = &[
    ('?', "/"),
    ('<', ","),
    ('>', "."),
    ('_', "-"),
    ('{', "["),
    ('}', "]"),
    (':', ";"),
    ('"', "'"),
    ('~', "`"),
    ('|', "\\"),
    ('!', "1"),
    ('@', "2"),
    ('#', "3"),
    ('$', "4"),
    ('%', "5"),
    ('^', "6"),
    ('&', "7"),
    ('*', "8"),
    ('(', "9"),
    (')', "0"),
];

fn invalid(text: &str, why: &str) -> CommandError {
    CommandError::with_details(
        codes::INVALID_SEQUENCE,
        format!("`{text}` is not a key sequence: {why}"),
        json!({ "keys": text, "reason": why }),
    )
    .hint(
        "modifiers Primary, Ctrl, Alt, Shift joined with + before one key name, e.g. Primary+Shift+Z, Shift+F1, Space",
    )
}

fn reserved_error(text: &str, why: &str) -> CommandError {
    CommandError::with_details(
        codes::RESERVED_KEY,
        format!("`{text}` is reserved ({why}) and cannot be bound"),
        json!({ "keys": text, "reason": why }),
    )
}

/// The canonical key name of one key token (`None`: not a key).
fn key_name(token: &str) -> Option<(String, bool)> {
    let lower = token.to_ascii_lowercase();
    if let Some((name, _)) = NAMED.iter().find(|(n, aliases)| *n == token || aliases.contains(&lower.as_str())) {
        return Some(((*name).to_owned(), false));
    }
    if lower == "plus" {
        return Some(("=".to_owned(), true));
    }
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphanumeric() {
            return Some((c.to_ascii_uppercase().to_string(), false));
        }
        if let Some((_, base)) = SHIFTED.iter().find(|(s, _)| *s == c) {
            return Some(((*base).to_owned(), true));
        }
    }
    let f = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=24).contains(n));
    f.map(|n| (format!("F{n}"), false))
}

/// Parses a sequence for a platform.
///
/// # Errors
/// `keymap.invalidSequence` (empty, unknown key, two keys, a modifier twice); `keymap.reservedKey` for a bare
/// modifier or a modifier the platform reserves (the Windows key).
pub fn parse(text: &str, platform: Platform) -> Result<Sequence, CommandError> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(invalid(text, "empty or too long"));
    }
    // A trailing "+" is the key itself only when written as the alias `Plus`.
    let tokens: Vec<&str> = trimmed.split('+').map(str::trim).collect();
    if tokens.iter().any(|t| t.is_empty()) {
        return Err(invalid(text, "empty part (write the plus key as Plus)"));
    }
    let mut seq = Sequence { primary: false, ctrl: false, alt: false, shift: false, key: String::new() };
    let (last, mods) = tokens.split_last().ok_or_else(|| invalid(text, "empty"))?;
    let set = |flag: &mut bool, name: &str| {
        if *flag {
            Err(invalid(text, &format!("{name} appears twice")))
        } else {
            *flag = true;
            Ok(())
        }
    };
    let modifier = |t: &str| -> Option<&'static str> {
        match t.to_ascii_lowercase().as_str() {
            "primary" => Some("primary"),
            "ctrl" | "control" => Some("ctrl"),
            "alt" | "option" | "opt" => Some("alt"),
            "shift" => Some("shift"),
            "cmd" | "command" | "meta" | "super" | "win" | "windows" => Some("meta"),
            _ => None,
        }
    };
    if modifier(last).is_some() {
        return Err(reserved_error(text, "a bare modifier"));
    }
    for m in mods {
        match modifier(m) {
            Some("primary") => set(&mut seq.primary, "Primary")?,
            Some("ctrl") if platform == Platform::Macos => set(&mut seq.ctrl, "Ctrl")?,
            Some("ctrl") => set(&mut seq.primary, "Ctrl (Primary on this platform)")?,
            Some("alt") => set(&mut seq.alt, "Alt")?,
            Some("shift") => set(&mut seq.shift, "Shift")?,
            Some(_) if platform == Platform::Macos => set(&mut seq.primary, "Command (Primary)")?,
            Some(_) => return Err(reserved_error(text, "the Windows key is reserved by the operating system")),
            None => return Err(invalid(text, &format!("`{m}` is not a modifier"))),
        }
    }
    let (key, implied_shift) = key_name(last).ok_or_else(|| invalid(text, &format!("`{last}` is not a key name")))?;
    if implied_shift {
        if seq.shift {
            return Err(invalid(text, &format!("`{last}` already implies Shift")));
        }
        seq.shift = true;
    }
    seq.key = key;
    Ok(seq)
}

impl Sequence {
    /// The canonical text (`Primary+Ctrl+Alt+Shift+<Key>`).
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut parts: Vec<&str> = Vec::with_capacity(5);
        if self.primary {
            parts.push("Primary");
        }
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(&self.key);
        parts.join("+")
    }

    /// The native notation shown in menus and tooltips (01 §0): `⌃⌥⇧⌘Z` on macOS, `Ctrl+Alt+Shift+Z` elsewhere.
    #[must_use]
    pub fn native(&self, platform: Platform) -> String {
        if platform == Platform::Macos {
            let mut s = String::new();
            if self.ctrl {
                s.push('\u{2303}');
            }
            if self.alt {
                s.push('\u{2325}');
            }
            if self.shift {
                s.push('\u{21E7}');
            }
            if self.primary {
                s.push('\u{2318}');
            }
            s.push_str(&self.key);
            s
        } else {
            let mut parts: Vec<&str> = Vec::new();
            if self.primary {
                parts.push("Ctrl");
            }
            if self.alt {
                parts.push("Alt");
            }
            if self.shift {
                parts.push("Shift");
            }
            parts.push(&self.key);
            parts.join("+")
        }
    }
}

/// Why a sequence is reserved on a platform (01 §8.3 item 5), or `None`.
#[must_use]
pub fn reserved(seq: &Sequence, platform: Platform) -> Option<&'static str> {
    let plain = !seq.primary && !seq.ctrl && !seq.alt && !seq.shift;
    if plain && seq.key == "Escape" {
        return Some("Escape alone cancels gestures, captures and dialogs");
    }
    let c = seq.canonical();
    let os: &[(&str, &str)] = match platform {
        Platform::Macos => &[
            ("Primary+Tab", "application switcher"),
            ("Primary+Shift+Tab", "application switcher"),
            ("Primary+H", "hide the application"),
            ("Primary+Alt+H", "hide other applications"),
            ("Primary+Alt+Escape", "force quit"),
            ("Primary+Space", "system search"),
            ("Primary+Shift+3", "screen capture"),
            ("Primary+Shift+4", "screen capture"),
            ("Primary+Shift+5", "screen capture"),
            ("Primary+Ctrl+Q", "lock screen"),
            ("Primary+`", "window cycling"),
            ("Primary+Ctrl+Space", "character viewer"),
        ],
        Platform::Windows | Platform::Linux => &[
            ("Alt+Tab", "window switcher"),
            ("Alt+Shift+Tab", "window switcher"),
            ("Primary+Alt+Delete", "secure attention sequence"),
            ("Alt+F4", "close window"),
            ("Primary+Escape", "start menu"),
            ("Primary+Shift+Escape", "task manager"),
        ],
    };
    os.iter().find(|(k, _)| *k == c).map(|(_, why)| *why)
}

/// A keymap context (01 §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Context {
    /// Everywhere.
    Global,
    /// Drawing mode.
    Drawing,
    /// Staging mode.
    Staging,
}

impl Context {
    /// The context's name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Context::Global => "global",
            Context::Drawing => "drawing",
            Context::Staging => "staging",
        }
    }
}

/// Two contexts conflict for one key (01 §8.3 item 6): the same context, or `global` with a mode context, where the
/// mode binding would shadow the global one.
#[must_use]
pub fn contexts_conflict(a: Context, b: Context) -> bool {
    a == b || a == Context::Global || b == Context::Global
}

/// A binding (01 §12.4 `Binding`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    /// The bound command.
    pub command_id: String,
    /// Fixed parameters of the command reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Map<String, Value>>,
    /// Canonical key sequence.
    pub keys: String,
    /// Context.
    pub context: Context,
    /// Part of the bundled default set.
    pub is_default: bool,
}

impl Binding {
    fn same_reference(&self, other: &Binding) -> bool {
        self.command_id == other.command_id && self.params == other.params
    }

    fn same(&self, other: &Binding) -> bool {
        self.same_reference(other) && self.keys == other.keys && self.context == other.context
    }

    fn sort_key(&self) -> (String, Context, String, String) {
        let params = self.params.as_ref().map(|p| Value::Object(p.clone()).to_string()).unwrap_or_default();
        (self.command_id.clone(), self.context, self.keys.clone(), params)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DefaultBinding {
    command_id: String,
    #[serde(default)]
    params: Option<Map<String, Value>>,
    keys: String,
    context: Context,
    #[serde(default)]
    platforms: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultFile {
    #[serde(rename = "$comment")]
    _comment: String,
    set: String,
    version: u32,
    bindings: Vec<DefaultBinding>,
}

/// The parsed default file, or why it did not parse (a build defect; `c41_keymap` proves it parses).
fn default_file() -> &'static Result<DefaultFile, String> {
    static FILE: OnceLock<Result<DefaultFile, String>> = OnceLock::new();
    FILE.get_or_init(|| {
        serde_json::from_str::<DefaultFile>(include_str!("keymap-default.json")).map_err(|e| e.to_string())
    })
}

/// The name of the bundled default set.
#[must_use]
pub fn default_set_name() -> &'static str {
    match default_file() {
        Ok(f) if f.version == 1 => f.set.as_str(),
        _ => "default",
    }
}

/// The bundled defaults for a platform, canonicalised. Entries that do not parse are skipped (and reported by
/// [`default_problems`], which a test keeps empty).
#[must_use]
pub fn defaults(platform: Platform) -> Vec<Binding> {
    let Ok(file) = default_file() else { return Vec::new() };
    file.bindings
        .iter()
        .filter(|b| b.platforms.as_ref().is_none_or(|p| p.iter().any(|x| x == platform.name())))
        .filter_map(|b| {
            let seq = parse(&b.keys, platform).ok()?;
            Some(Binding {
                command_id: b.command_id.clone(),
                params: b.params.clone(),
                keys: seq.canonical(),
                context: b.context,
                is_default: true,
            })
        })
        .collect()
}

/// Problems of the bundled defaults for a platform: unparsable sequences, reserved keys, conflicts.
#[must_use]
pub fn default_problems(platform: Platform) -> Vec<String> {
    let mut out = Vec::new();
    let file = match default_file() {
        Ok(f) => f,
        Err(e) => return vec![e.clone()],
    };
    for b in &file.bindings {
        if b.platforms.as_ref().is_some_and(|p| !p.iter().any(|x| x == platform.name())) {
            continue;
        }
        match parse(&b.keys, platform) {
            Ok(seq) => {
                if let Some(why) = reserved(&seq, platform) {
                    out.push(format!("{}: {} is reserved ({why})", b.command_id, b.keys));
                }
            }
            Err(e) => out.push(format!("{}: {}", b.command_id, e.message)),
        }
    }
    let d = defaults(platform);
    for (i, a) in d.iter().enumerate() {
        for b in &d[i + 1..] {
            if a.keys == b.keys && contexts_conflict(a.context, b.context) {
                out.push(format!("{} conflicts with {} on {}", a.command_id, b.command_id, a.keys));
            }
        }
    }
    out
}

/// User changes on top of the defaults (01 §8.1): bindings added and default bindings removed. Stored in
/// `keymap.json` as `{baseSet, added, removed}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// Bindings the user added.
    pub added: Vec<Binding>,
    /// Default bindings the user removed.
    pub removed: Vec<Binding>,
}

impl Overrides {
    /// Reads the stored form; entries that do not parse are dropped.
    #[must_use]
    pub fn from_stored(v: &Value) -> Self {
        let list = |name: &str, default: bool| -> Vec<Binding> {
            v.get(name)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|b| {
                            let mut b = b.clone();
                            if let Some(m) = b.as_object_mut() {
                                m.insert("isDefault".into(), json!(default));
                            }
                            serde_json::from_value::<Binding>(b).ok()
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        Self { added: list("added", false), removed: list("removed", true) }
    }

    /// The stored form.
    #[must_use]
    pub fn to_stored(&self) -> Value {
        let strip = |l: &[Binding]| -> Vec<Value> {
            l.iter()
                .map(|b| {
                    let mut v = serde_json::to_value(b).unwrap_or(Value::Null);
                    if let Some(m) = v.as_object_mut() {
                        m.shift_remove("isDefault");
                    }
                    v
                })
                .collect()
        };
        json!({ "baseSet": default_set_name(), "added": strip(&self.added), "removed": strip(&self.removed) })
    }

    /// Whether nothing differs from the defaults.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// The effective keymap: the platform's defaults minus the removed ones, plus the added ones, sorted.
#[must_use]
pub fn effective(overrides: &Overrides, platform: Platform) -> Vec<Binding> {
    let mut out: Vec<Binding> =
        defaults(platform).into_iter().filter(|d| !overrides.removed.iter().any(|r| r.same(d))).collect();
    out.extend(overrides.added.iter().cloned());
    out.sort_by_key(Binding::sort_key);
    out
}

/// Recomputes overrides from a desired effective keymap (the complement of [`effective`]).
#[must_use]
pub fn overrides_for(desired: &[Binding], platform: Platform) -> Overrides {
    let defaults = defaults(platform);
    let removed = defaults.iter().filter(|d| !desired.iter().any(|b| b.same(d))).cloned().collect();
    let added = desired
        .iter()
        .filter(|b| !defaults.iter().any(|d| d.same(b)))
        .map(|b| Binding { is_default: false, ..b.clone() })
        .collect();
    Overrides { added, removed }
}

/// The binding a key sequence runs in a mode (01 §8.1): the mode's context first, then `global`.
#[must_use]
pub fn resolve<'a>(bindings: &'a [Binding], keys: &str, mode: Option<Context>) -> Option<&'a Binding> {
    let find = |c: Context| bindings.iter().find(|b| b.keys == keys && b.context == c);
    mode.filter(|m| *m != Context::Global).and_then(find).or_else(|| find(Context::Global))
}

/// Bindings that conflict with `candidate` (same keys, conflicting context, another command reference).
#[must_use]
pub fn conflicts<'a>(bindings: &'a [Binding], candidate: &Binding) -> Vec<&'a Binding> {
    bindings
        .iter()
        .filter(|b| b.keys == candidate.keys && contexts_conflict(b.context, candidate.context))
        .filter(|b| !b.same(candidate))
        .collect()
}

/// The commands whose bindings differ between two keymaps.
#[must_use]
pub fn changed_commands(before: &[Binding], after: &[Binding]) -> Vec<String> {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for b in before {
        if !after.iter().any(|a| a.same(b)) {
            ids.insert(b.command_id.clone());
        }
    }
    for a in after {
        if !before.iter().any(|b| b.same(a)) {
            ids.insert(a.command_id.clone());
        }
    }
    ids.into_iter().collect()
}

/// Groups bindings by command (for the editor and `ui.help.get`).
#[must_use]
pub fn by_command(bindings: &[Binding]) -> BTreeMap<String, Vec<Binding>> {
    let mut m: BTreeMap<String, Vec<Binding>> = BTreeMap::new();
    for b in bindings {
        m.entry(b.command_id.clone()).or_default().push(b.clone());
    }
    m
}
