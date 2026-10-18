//! Path addressing (10 §2.4, 11 §5.1).
//!
//! ```text
//! path       := segment ( "/" segment )*
//! segment    := type ":" quotedName       element:"Bob"
//!             | type ":#" int              drawingNode:#2       1-based position in an ordered collection
//!             | type ":@" id               stroke:@s42          by id (local or project-wide)
//!             | type ":" keyword           library:global       reserved keywords: local, global (libraries)
//!             | type                       transition, camera    singleton shortcuts
//! quotedName := '"' ( any character except '"' and '\' | '\"' | '\\' )* '"'
//! ```
//!
//! A leading `project` segment is optional. The first segment may instead be a special reference (`@current`,
//! `@selection`, `@activeView`, `@activeCamera`; 11 §5.1). Besides the object types of 10 §2.1 and their aliases,
//! two segment types address non-objects: `cell:#n` (a cell of a cell-based element, by frame) and `camera` (the
//! current camera element of a scene).

use std::fmt;

use crate::error::{ModelError, codes};
use crate::id::{Id, IdRef};
use crate::types::ObjectType;

/// Maximum number of segments in a path. Every object the model accepts has a canonical path within this bound
/// (C11-R1-10): an object nested `n` levels deep sits at JSON depth ≥ `n`, and objects are at most
/// [`crate::input::MAX_MODEL_DEPTH`] deep.
pub const MAX_SEGMENTS: usize = crate::input::MAX_MODEL_DEPTH;
/// Maximum length of a path string accepted by the parser: [`MAX_SEGMENTS`] segments of the longest name (128
/// characters, each at most 4 UTF-8 bytes, escaped) plus type and quotes stay below it.
pub const MAX_PATH_LEN: usize = 64 * 1024;

/// A special reference allowed as the first segment (11 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialRef {
    /// `@current`: current scene / element / frame / drawing as appropriate.
    Current,
    /// `@selection`.
    Selection,
    /// `@activeView`.
    ActiveView,
    /// `@activeCamera`.
    ActiveCamera,
}

impl SpecialRef {
    /// The spelling with `@`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "@current",
            Self::Selection => "@selection",
            Self::ActiveView => "@activeView",
            Self::ActiveCamera => "@activeCamera",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "@current" => Self::Current,
            "@selection" => Self::Selection,
            "@activeView" => Self::ActiveView,
            "@activeCamera" => Self::ActiveCamera,
            _ => return None,
        })
    }
}

/// What a segment addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    /// An object type of 10 §2.1 (aliases are resolved to the type).
    Object(ObjectType),
    /// `cell`: a cell of a cell-based element (not an object; 10 §4.7).
    Cell,
    /// `camera`: the current camera element of a scene (10 §2.4).
    Camera,
}

impl SegmentType {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "cell" => Some(Self::Cell),
            "camera" => Some(Self::Camera),
            other => ObjectType::parse(other).map(Self::Object),
        }
    }

    /// The canonical spelling (primary type names, never aliases).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Object(t) => t.as_str(),
            Self::Cell => "cell",
            Self::Camera => "camera",
        }
    }
}

/// A library keyword (10 §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKeyword {
    /// The project-local library.
    Local,
    /// The user's global library.
    Global,
}

/// How a segment selects among candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// By quoted name (exact match first, then case-insensitive).
    Name(String),
    /// By 1-based position in an ordered collection (frame number for `cell`).
    Index(u32),
    /// By ID (local or project-wide).
    Id(Id),
    /// A reserved keyword (`library:local`, `library:global`).
    Keyword(LibraryKeyword),
    /// A singleton shortcut (`transition`, `camera`).
    Singleton,
}

/// One path segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    ty: SegmentType,
    selector: Selector,
}

impl Segment {
    /// A segment from its parts (used to build canonical paths).
    #[must_use]
    pub fn new(ty: SegmentType, selector: Selector) -> Self {
        Self { ty, selector }
    }

    /// The addressed type.
    #[must_use]
    pub fn ty(&self) -> SegmentType {
        self.ty
    }

    /// The selector.
    #[must_use]
    pub fn selector(&self) -> &Selector {
        &self.selector
    }
}

/// Writes a quoted name with `"` and `\` escaped (10 §2.4).
#[must_use]
pub fn quote(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    out.push('"');
    for c in name.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = self.ty.as_str();
        match &self.selector {
            Selector::Name(n) => write!(f, "{ty}:{}", quote(n)),
            Selector::Index(i) => write!(f, "{ty}:#{i}"),
            Selector::Id(id) => write!(f, "{ty}:@{id}"),
            Selector::Keyword(LibraryKeyword::Local) => write!(f, "{ty}:local"),
            Selector::Keyword(LibraryKeyword::Global) => write!(f, "{ty}:global"),
            Selector::Singleton => f.write_str(ty),
        }
    }
}

/// A parsed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    special: Option<SpecialRef>,
    segments: Vec<Segment>,
}

impl Path {
    /// A path from the project root.
    #[must_use]
    pub fn from_segments(segments: Vec<Segment>) -> Self {
        Self { special: None, segments }
    }

    /// The special reference the path starts at, if any.
    #[must_use]
    pub fn special(&self) -> Option<SpecialRef> {
        self.special
    }

    /// The segments after the root (a leading `project` segment is dropped).
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Parses a path.
    ///
    /// # Errors
    /// `arg.invalid` with `details.offset` (byte offset of the problem) for any syntax error.
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        Parser { s: input, pos: 0 }.parse()
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        if let Some(s) = self.special {
            f.write_str(s.as_str())?;
            first = false;
        }
        if first && self.segments.is_empty() {
            return f.write_str("project");
        }
        for seg in &self.segments {
            if !first {
                f.write_str("/")?;
            }
            write!(f, "{seg}")?;
            first = false;
        }
        Ok(())
    }
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, msg: impl Into<String>) -> ModelError {
        ModelError::with_details(
            codes::ARG_INVALID,
            format!("invalid path: {}", msg.into()),
            serde_json::json!({ "offset": self.pos }),
        )
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.pos..).and_then(|r| r.chars().next())
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn take_while(&mut self, f: impl Fn(char) -> bool) -> &str {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if !f(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        self.s.get(start..self.pos).unwrap_or("")
    }

    fn parse(mut self) -> Result<Path, ModelError> {
        if self.s.is_empty() {
            return Err(self.err("empty path"));
        }
        if self.s.len() > MAX_PATH_LEN {
            return Err(self.err(format!("longer than {MAX_PATH_LEN} bytes")));
        }
        let mut special = None;
        let mut segments = Vec::new();
        if self.peek() == Some('@') {
            let word = self.take_while(|c| c != '/').to_owned();
            special =
                Some(SpecialRef::parse(&word).ok_or_else(|| self.err(format!("unknown special reference `{word}`")))?);
            if self.peek().is_none() {
                return Ok(Path { special, segments });
            }
            self.expect_slash()?;
        }
        let mut first = special.is_none();
        loop {
            let ty_start = self.pos;
            let ty_name = self.take_while(|c| c.is_ascii_alphanumeric()).to_owned();
            if ty_name.is_empty() {
                return Err(self.err("expected a type name"));
            }
            if first && ty_name == "project" && self.peek() != Some(':') {
                first = false;
                if self.peek().is_none() {
                    break;
                }
                self.expect_slash()?;
                continue;
            }
            first = false;
            let ty = SegmentType::parse(&ty_name).ok_or_else(|| {
                ModelError::with_details(
                    codes::ARG_INVALID,
                    format!("invalid path: unknown type `{ty_name}`"),
                    serde_json::json!({ "offset": ty_start }),
                )
            })?;
            let selector = if self.peek() == Some(':') {
                self.bump();
                match self.peek() {
                    Some('"') => Selector::Name(self.quoted()?),
                    Some('#') => {
                        self.bump();
                        let digits = self.take_while(|c| c.is_ascii_digit());
                        if digits.is_empty() || digits.starts_with('0') {
                            return Err(self.err("expected a positive integer after `#`"));
                        }
                        let n: u32 = digits.parse().map_err(|_| self.err("index out of range"))?;
                        Selector::Index(n)
                    }
                    Some('@') => {
                        self.bump();
                        let id = self.take_while(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-').to_owned();
                        Selector::Id(Id::parse(&id).map_err(|_| self.err("expected an id after `@`"))?)
                    }
                    _ => {
                        let word = self.take_while(|c| c.is_ascii_alphanumeric()).to_owned();
                        let kw = match word.as_str() {
                            "local" => LibraryKeyword::Local,
                            "global" => LibraryKeyword::Global,
                            _ => {
                                return Err(
                                    self.err(format!("expected `\"name\"`, `#n`, `@id` or a keyword, found `{word}`"))
                                );
                            }
                        };
                        if ty != SegmentType::Object(ObjectType::Library) {
                            return Err(self.err("keywords `local` and `global` select libraries only"));
                        }
                        Selector::Keyword(kw)
                    }
                }
            } else {
                match ty {
                    SegmentType::Camera | SegmentType::Object(ObjectType::Transition) => Selector::Singleton,
                    _ => return Err(self.err(format!("`{ty_name}` needs a selector (`:\"name\"`, `:#n` or `:@id`)"))),
                }
            };
            if ty == SegmentType::Cell && !matches!(selector, Selector::Index(_)) {
                return Err(self.err("cells are selected by frame: `cell:#n`"));
            }
            segments.push(Segment { ty, selector });
            if segments.len() > MAX_SEGMENTS {
                return Err(self.err(format!("more than {MAX_SEGMENTS} segments")));
            }
            if self.peek().is_none() {
                break;
            }
            self.expect_slash()?;
        }
        Ok(Path { special, segments })
    }

    fn expect_slash(&mut self) -> Result<(), ModelError> {
        if self.bump() == Some('/') && self.peek().is_some() {
            Ok(())
        } else {
            Err(self.err("expected `/` followed by a segment"))
        }
    }

    fn quoted(&mut self) -> Result<String, ModelError> {
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(self.err("unterminated quoted name")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some(c @ ('"' | '\\')) => out.push(c),
                    _ => return Err(self.err("only `\\\"` and `\\\\` are escapes in a quoted name")),
                },
                Some(c) => out.push(c),
            }
        }
    }
}

/// A reference as accepted by every `ref` parameter (10 AG-03): an ID, a compound ID or a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ref {
    /// A plain or compound ID.
    Id(IdRef),
    /// A path.
    Path(Path),
}

impl Ref {
    /// Classifies and parses a reference string. Strings that are valid IDs or compound IDs are IDs, except the
    /// singleton words `project` and `transition`, which are paths; everything else is parsed as a path.
    ///
    /// # Errors
    /// `arg.invalid` when the string is neither.
    pub fn parse(s: &str) -> Result<Self, ModelError> {
        if s != "project"
            && s != "transition"
            && !s.contains(':')
            && !s.starts_with('@')
            && let Ok(r) = IdRef::parse(s)
        {
            return Ok(Self::Id(r));
        }
        Path::parse(s).map(Self::Path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(s: &str) -> String {
        Path::parse(s).unwrap().to_string()
    }

    #[test]
    fn spec_examples_parse_and_print() {
        for s in [
            "scene:\"Intro\"/element:\"Bob\"/drawing:\"3\"",
            "palette:\"Main\"/swatch:\"Skin light\"",
            "scene:\"Intro\"/element:\"Bob\"/drawing:\"3\"/drawingNode:#2/stroke:@s42",
            "scene:\"Intro\"/element:\"Bob\"/cell:#12",
            "pen:\"Pen 1\"",
            "library:global/libraryFolder:\"Props\"/libraryItem:\"Walk\"",
            "scene:\"Intro\"/camera",
            "scene:\"Intro\"/transition",
            "scene:\"Intro\"/element:\"Bob-P\"/channel:\"rotation\"/key:\"24\"",
        ] {
            assert_eq!(rt(s), s);
        }
        // Aliases print as primary type names; a leading `project` is dropped.
        assert_eq!(rt("palette:\"Main\"/style:\"Night\""), "palette:\"Main\"/paletteStyle:\"Night\"");
        assert_eq!(rt("project/scene:\"A\""), "scene:\"A\"");
        assert_eq!(rt("project"), "project");
        assert_eq!(rt("transition"), "transition");
        assert_eq!(rt("@current/element:\"Bob\""), "@current/element:\"Bob\"");
        assert_eq!(rt("@selection"), "@selection");
    }

    #[test]
    fn quoted_names_escape() {
        let p = Path::parse(r#"element:"a\"b\\c/d""#).unwrap();
        assert_eq!(p.segments()[0].selector(), &Selector::Name("a\"b\\c/d".into()));
        assert_eq!(p.to_string(), r#"element:"a\"b\\c/d""#);
    }

    #[test]
    fn rejects_malformed_paths() {
        for bad in [
            "",
            "/",
            "scene",
            "scene:",
            "scene:\"x",
            "scene:#0",
            "scene:#01",
            "scene:#-1",
            "scene:@",
            "scene:@a b",
            "scene:\"x\"/",
            "scene:\"x\"//element:\"y\"",
            "nope:\"x\"",
            "scene:local",
            "cell:\"3\"",
            "@bogus",
            "scene:\"a\\x\"",
            "scene:#99999999999",
        ] {
            let e = Path::parse(bad).unwrap_err();
            assert_eq!(e.code(), "arg.invalid", "{bad}");
        }
    }

    #[test]
    fn refs_classify() {
        assert!(matches!(Ref::parse("el_01JA").unwrap(), Ref::Id(_)));
        assert!(matches!(Ref::parse("dr_01JA/s42").unwrap(), Ref::Id(_)));
        assert!(matches!(Ref::parse("project").unwrap(), Ref::Path(_)));
        assert!(matches!(Ref::parse("scene:\"A\"").unwrap(), Ref::Path(_)));
        assert!(Ref::parse("not an id").is_err());
    }
}
