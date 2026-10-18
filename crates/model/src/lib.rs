//! # tt-model — object model
//!
//! The document object model shared by the project files, the in-memory document and the agent interface (10 DM-1):
//! object types and the common envelope (`id`, `type`, `rev`, `name`, `meta`, `ext`), ULID-style identifiers,
//! revisions and change sets, path addressing, canonical JSON, JSON Schemas (draft 2020-12, generated with
//! `schemars`) and validation.
//!
//! Boundary rule (plan §3.2): model types expose **no public mutators**. Fields are crate-private and read through
//! accessors; values are built only by validated parsing or by constructors that return new values. Every document
//! mutation goes through `tt-executor` (C1.2), which alone holds the document.
//!
//! Spec sections implemented: 10 §1–§5; 10 §6.5 (canonical JSON); 10 §2.8 (schemas); 11 §5.1 (addressing).
//! Milestone: M1 (C1.1). Decisions: ADR-0010 (object model and identifiers), ADR-0011 (canonical JSON and schemas).

pub mod canonical;
pub mod change;
pub mod document;
pub mod entities;
pub mod error;
pub mod fixture;
pub mod id;
pub mod name;
pub mod object;
pub mod path;
pub mod pointer;
pub mod schema;
pub mod schema_ts;
pub mod types;
pub mod validate;
pub mod value;

pub use error::{ModelError, codes};
pub use id::{Id, IdGenerator, IdRef};
pub use path::{Path, Ref};
pub use types::ObjectType;

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["10 §1–§5", "10 §6.5 (canonical JSON)", "10 §2.8 (schemas)", "11 §5 (addressing, units, coordinate spaces)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.1)";

/// Project format / document schema version (`MAJOR.MINOR`, 10 §6.6 VER-01). Reported in the MCP handshake (11 §4)
/// and by `app.info.get` (01); every schema carries it as `x-since` baseline.
pub const FORMAT_VERSION: &str = "1.0";

/// Maximum nesting depth of JSON accepted or written by the model (objects and arrays).
pub const MAX_JSON_DEPTH: usize = 128;
