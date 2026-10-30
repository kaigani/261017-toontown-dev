//! The entry layout of packages and folders (10 §6.3, LAY-01/LAY-02): where each object entry lives, and what an
//! entry name is.

use tt_model::Id;
use tt_model::document::{Document, EntryKey};

/// `project.json`.
pub const PROJECT: &str = "project.json";
/// `manifest.json`.
pub const MANIFEST: &str = "manifest.json";
/// `mimetype`.
pub const MIMETYPE: &str = "mimetype";
/// `library/library.json`.
pub const LIBRARY: &str = "library/library.json";
/// `session.json`.
pub const SESSION: &str = "session.json";
/// `thumbnail.png`.
pub const THUMBNAIL: &str = "thumbnail.png";
/// The folder-form save journal (SAVE-02).
pub const JOURNAL: &str = ".save-journal.json";
/// The folder-form lock file (LOCK-01).
pub const FOLDER_LOCK: &str = ".lock";

/// What an entry name denotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// `mimetype`.
    Mimetype,
    /// `manifest.json`.
    Manifest,
    /// `project.json`.
    Project,
    /// `palettes/<id>.json`.
    Palette(String),
    /// `textures/<file>`.
    Texture,
    /// `scenes/<id>/scene.json`.
    Scene(String),
    /// `scenes/<sceneId>/elements/<id>.json`.
    Element {
        /// Scene ID from the path.
        scene: String,
        /// Element ID from the path.
        id: String,
    },
    /// `banks/<id>/bank.json`.
    Bank(String),
    /// `banks/<bankId>/<id>.json`.
    Drawing {
        /// Bank ID from the path.
        bank: String,
        /// Drawing ID from the path.
        id: String,
    },
    /// `banks/<bankId>/<drawingId>/<file>` (bitmap node pixels).
    Pixels,
    /// `media/<file>`.
    Media,
    /// `library/library.json`.
    LibraryIndex,
    /// `library/files/…`, `library/thumbnails/…`.
    LibraryFile,
    /// `thumbnail.png`.
    Thumbnail,
    /// `cache/…`.
    Cache,
    /// `session.json`.
    Session,
    /// A name inside a reserved namespace that matches no layout rule (not ours to keep).
    StrayReserved,
    /// A name outside the reserved names: preserved verbatim (ZIP-04).
    Unknown,
}

impl Kind {
    /// Whether the entry is a JSON object entry of the document.
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(
            self,
            Kind::Project
                | Kind::Palette(_)
                | Kind::Scene(_)
                | Kind::Element { .. }
                | Kind::Bank(_)
                | Kind::Drawing { .. }
                | Kind::LibraryIndex
        )
    }

    /// Whether the manifest lists the entry (package form; 10 §6.4: every entry except `mimetype`, `manifest.json`,
    /// `cache/` and `session.json`).
    #[must_use]
    pub fn listed(&self) -> bool {
        !matches!(self, Kind::Mimetype | Kind::Manifest | Kind::Cache | Kind::Session)
    }
}

fn json_stem(file: &str) -> Option<&str> {
    file.strip_suffix(".json").filter(|s| !s.is_empty())
}

/// Classifies an entry name.
#[must_use]
pub fn classify(name: &str) -> Kind {
    let parts: Vec<&str> = name.split('/').collect();
    match parts.as_slice() {
        ["mimetype"] => Kind::Mimetype,
        ["manifest.json"] => Kind::Manifest,
        ["project.json"] => Kind::Project,
        ["thumbnail.png"] => Kind::Thumbnail,
        ["session.json"] => Kind::Session,
        ["palettes", f] => json_stem(f).map_or(Kind::StrayReserved, |s| Kind::Palette(s.to_owned())),
        ["textures", ..] if parts.len() >= 2 => Kind::Texture,
        ["scenes", s, "scene.json"] => Kind::Scene((*s).to_owned()),
        ["scenes", s, "elements", f] => {
            json_stem(f).map_or(Kind::StrayReserved, |e| Kind::Element { scene: (*s).to_owned(), id: e.to_owned() })
        }
        ["banks", b, "bank.json"] => Kind::Bank((*b).to_owned()),
        ["banks", b, f] => {
            json_stem(f).map_or(Kind::StrayReserved, |d| Kind::Drawing { bank: (*b).to_owned(), id: d.to_owned() })
        }
        ["banks", _, _, _] => Kind::Pixels,
        ["media", _] => Kind::Media,
        ["library", "library.json"] => Kind::LibraryIndex,
        ["library", "files" | "thumbnails", ..] if parts.len() >= 3 => Kind::LibraryFile,
        ["cache", ..] => Kind::Cache,
        ["palettes" | "textures" | "scenes" | "banks" | "media" | "library", ..] => Kind::StrayReserved,
        _ => Kind::Unknown,
    }
}

/// Whether a folder-form file is a control file of the folder, not an entry: names whose first segment starts with
/// `.` (`.lock`, `.save-journal.json`, `.gitignore`, `.gitattributes`, `.git/`, temporaries; ADR-0014 §2).
#[must_use]
pub fn is_folder_control(name: &str) -> bool {
    name.split('/').next().is_some_and(|first| first.starts_with('.'))
}

/// The entry path of an object entry (10 §6.3). `None` when the element or drawing has no scene or bank.
#[must_use]
pub fn entry_path(doc: &Document, key: &EntryKey) -> Option<String> {
    Some(match key {
        EntryKey::Project => PROJECT.to_owned(),
        EntryKey::Library => LIBRARY.to_owned(),
        EntryKey::Palette(id) => format!("palettes/{id}.json"),
        EntryKey::Bank(id) => format!("banks/{id}/bank.json"),
        EntryKey::Drawing(id) => format!("banks/{}/{id}.json", doc.bank_of(id)?),
        EntryKey::Scene(id) => format!("scenes/{id}/scene.json"),
        EntryKey::Element(id) => format!("scenes/{}/elements/{id}.json", doc.scene_of(id)?),
    })
}

/// The entry key of an object entry path, when it parses as an ID.
#[must_use]
pub fn entry_key(kind: &Kind) -> Option<EntryKey> {
    let id = |s: &str| Id::parse(s).ok();
    Some(match kind {
        Kind::Project => EntryKey::Project,
        Kind::LibraryIndex => EntryKey::Library,
        Kind::Palette(p) => EntryKey::Palette(id(p)?),
        Kind::Bank(b) => EntryKey::Bank(id(b)?),
        Kind::Drawing { id: d, .. } => EntryKey::Drawing(id(d)?),
        Kind::Scene(s) => EntryKey::Scene(id(s)?),
        Kind::Element { id: e, .. } => EntryKey::Element(id(e)?),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(classify("project.json"), Kind::Project);
        assert_eq!(classify("palettes/pal_1.json"), Kind::Palette("pal_1".into()));
        assert_eq!(classify("scenes/sc_1/scene.json"), Kind::Scene("sc_1".into()));
        assert_eq!(
            classify("scenes/sc_1/elements/el_2.json"),
            Kind::Element { scene: "sc_1".into(), id: "el_2".into() }
        );
        assert_eq!(classify("banks/db_1/bank.json"), Kind::Bank("db_1".into()));
        assert_eq!(classify("banks/db_1/dr_2.json"), Kind::Drawing { bank: "db_1".into(), id: "dr_2".into() });
        assert_eq!(classify("banks/db_1/dr_2/nd_3.png"), Kind::Pixels);
        assert_eq!(classify("media/as_1.wav"), Kind::Media);
        assert_eq!(classify("textures/a.png"), Kind::Texture);
        assert_eq!(classify("library/library.json"), Kind::LibraryIndex);
        assert_eq!(classify("library/files/x.animtpl"), Kind::LibraryFile);
        assert_eq!(classify("cache/peaks/x"), Kind::Cache);
        assert_eq!(classify("vendor/notes.txt"), Kind::Unknown);
        assert_eq!(classify("scenes/x.txt"), Kind::StrayReserved);
        assert_eq!(classify("palettes/.json"), Kind::StrayReserved);
        assert!(is_folder_control(".git/HEAD") && is_folder_control(".lock") && !is_folder_control("a/.b"));
    }
}
