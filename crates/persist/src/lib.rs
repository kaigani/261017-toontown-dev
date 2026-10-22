//! # tt-persist — persistence
//!
//! Project package (`.animpkg`, zip with ZIP64) and its folder form, manifest, atomic save, backups, autosave,
//! crash recovery, locks, schema migrations; `.animtpl`, `.animpal` and `.animlib` containers; per-user settings.
//! Reads and writes go through the model's single serializer (10 DM-1). Loading produces a *candidate* document and a
//! load report; it never touches an open document: the executor alone installs a candidate, after its own integrity
//! gate (ADR-0013 §16, ADR-0014).
//!
//! - [`load::load`] runs the load sequence of 10 §7.1 with the repairs of §7.2 ([`repair`]).
//! - [`save::save`] writes either form atomically (SAVE-01, SAVE-02) with backups ([`backup`]).
//! - [`recovery`] writes and lists autosave copies; [`lock`] takes and refreshes lock files.
//! - [`zip`] is the hostile-input-safe container; [`container`] reads either form; [`manifest`] holds versions.
//! - [`library`] reads and writes library folders, templates and palette files; [`settings`] the user settings.
//!
//! Spec sections implemented: 10 §6–§8; 01 §3 (projects, autosave, backups), §11 (persisted user settings).
//! Milestone: M1 (C1.3). Decisions: ADR-0014.

pub mod b64;
pub mod backup;
pub mod codes;
pub mod container;
pub mod fsx;
pub mod host;
pub mod layout;
pub mod library;
pub mod load;
pub mod lock;
pub mod manifest;
pub mod migrate;
pub mod recovery;
pub mod repair;
pub mod report;
pub mod save;
pub mod settings;
pub mod time;
pub mod zip;

pub use container::{BlobData, Blobs, Container, Form};
pub use load::{LoadOptions, Loaded, Preserved, load};
pub use report::{LoadIssue, LoadReport};
pub use save::{SaveInput, Saved, save};

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["10 §6–§8", "01 §3 (projects, autosave, backups)", "01 §11 (persisted user settings)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.3)";
