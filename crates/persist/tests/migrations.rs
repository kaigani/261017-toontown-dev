//! Migrations (10 §6.6 VER-03/VER-04): every step of the product chain has a fixture (`tests/fixtures/migrations/`)
//! and turns its `before.json` into its `after.json`; the chain is ordered and ends at the current format; the engine
//! applies steps in order, refuses a gap, reports a failing step and keeps values without a new home.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_persist::manifest::Version;
use tt_persist::migrate::{CHAIN, Entries, Migration, keep_migrated, migrate};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/migrations")
}

fn entries(path: &Path) -> Entries {
    let v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn folder_name(m: &Migration) -> String {
    format!("{}-{}", m.from, m.to)
}

/// Applies each step of `chain` to its fixture under `root`; returns the number of fixture folders found.
fn check_fixtures(root: &Path, chain: &[Migration]) -> usize {
    for m in chain {
        let dir = root.join(folder_name(m));
        assert!(dir.is_dir(), "step `{}` has no fixture folder {}", m.name, dir.display());
        let got = (m.apply)(entries(&dir.join("before.json"))).unwrap();
        assert_eq!(got, entries(&dir.join("after.json")), "step `{}`", m.name);
        let got = migrate(entries(&dir.join("before.json")), m.from, m.to, chain).unwrap();
        assert_eq!(got, entries(&dir.join("after.json")), "step `{}` through migrate", m.name);
    }
    // Every fixture folder names a step.
    let names: Vec<String> = chain.iter().map(folder_name).collect();
    let mut folders = 0;
    for e in std::fs::read_dir(root).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !e.path().is_dir() || name == "synthetic" {
            continue;
        }
        assert!(names.contains(&name), "fixture folder `{name}` names no step of the chain");
        folders += 1;
    }
    folders
}

fn check_chain(chain: &[Migration], last: Version) {
    for w in chain.windows(2) {
        assert_eq!(w[0].to, w[1].from, "the chain is contiguous");
    }
    for m in chain {
        assert!(m.from < m.to, "`{}` moves forward", m.name);
    }
    if let Some(m) = chain.last() {
        assert_eq!(m.to, last, "the chain ends at the current format");
    }
}

#[test]
fn every_product_step_has_a_fixture_and_the_chain_ends_at_the_current_format() {
    let current = Version::parse(tt_model::FORMAT_VERSION).unwrap();
    check_chain(CHAIN, current);
    assert_eq!(check_fixtures(&fixtures(), CHAIN), CHAIN.len());
    // 1.0 is the first format: the product chain is empty, and a current file is returned unchanged.
    assert!(CHAIN.is_empty());
    let e = entries(&fixtures().join("synthetic/0.9-1.0/after.json"));
    assert_eq!(migrate(e.clone(), current, current, CHAIN).unwrap(), e);
}

const V0_8: Version = Version { major: 0, minor: 8 };
const V0_9: Version = Version { major: 0, minor: 9 };
const V1_0: Version = Version { major: 1, minor: 0 };

fn up_0_9(mut e: Entries) -> Result<Entries, String> {
    // The synthetic 0.9 format called the project's frame rate `fps`; 1.0 calls it `frameRate`.
    let p = e.get_mut("project.json").ok_or("no project")?;
    let fps = p.as_object_mut().ok_or("not an object")?.shift_remove("fps").ok_or("no fps")?;
    p["frameRate"] = fps;
    Ok(e)
}

static SYNTHETIC: &[Migration] = &[Migration { from: V0_9, to: V1_0, name: "0.9-fps", apply: up_0_9 }];

#[test]
fn the_synthetic_chain_matches_its_fixtures() {
    check_chain(SYNTHETIC, V1_0);
    assert_eq!(check_fixtures(&fixtures().join("synthetic"), SYNTHETIC), 1);
}

fn up_0_8(mut e: Entries) -> Result<Entries, String> {
    // The synthetic 0.8 format had `title` (0.9: `name`) and a `legacyFlag` with no new home (kept, VER-03).
    let p = e.get_mut("project.json").ok_or("no project")?;
    let m = p.as_object_mut().ok_or("not an object")?;
    let title = m.shift_remove("title").ok_or("no title")?;
    m.insert("name".into(), title);
    if let Some(flag) = m.shift_remove("legacyFlag") {
        keep_migrated(p, "legacyFlag", flag);
    }
    Ok(e)
}

fn broken(_: Entries) -> Result<Entries, String> {
    Err("cannot read the timing table".into())
}

#[test]
fn the_engine_applies_steps_in_order_and_reports_gaps_and_failures() {
    // Declared out of order: the engine follows versions, not positions.
    let chain = [
        Migration { from: V0_9, to: V1_0, name: "0.9-fps", apply: up_0_9 },
        Migration { from: V0_8, to: V0_9, name: "0.8-title", apply: up_0_8 },
    ];
    let old: Entries =
        [("project.json".to_owned(), json!({ "title": "Old", "fps": 12, "legacyFlag": 3 }))].into_iter().collect();
    let got = migrate(old.clone(), V0_8, V1_0, &chain).unwrap();
    assert_eq!(
        got["project.json"],
        json!({ "name": "Old", "ext": { "app.migrated": { "legacyFlag": 3 } }, "frameRate": 12 })
    );
    // Part of the way.
    let half = migrate(old.clone(), V0_8, V0_9, &chain).unwrap();
    assert_eq!(half["project.json"]["fps"], 12);
    // A gap: no step reads 0.7.
    let e = migrate(old.clone(), Version { major: 0, minor: 7 }, V1_0, &chain).unwrap_err();
    assert_eq!(e.code(), "project.unsupportedVersion");
    assert_eq!(e.details()["formatVersion"], "0.7");
    // A failing step names itself.
    let chain = [Migration { from: V0_8, to: V0_9, name: "0.8-timing", apply: broken }];
    let e = migrate(old, V0_8, V0_9, &chain).unwrap_err();
    assert_eq!(e.code(), "project.invalidFormat");
    assert_eq!((e.details()["reason"].clone(), e.details()["step"].clone()), (json!("migration"), json!("0.8-timing")));
}

#[test]
fn values_without_a_new_home_are_kept_under_ext() {
    let mut o = json!({ "id": "x", "ext": "not an object" });
    keep_migrated(&mut o, "a", json!(1));
    keep_migrated(&mut o, "b", json!([2]));
    assert_eq!(o["ext"], json!({ "app.migrated": { "a": 1, "b": [2] } }));
    let mut o = json!({ "id": "y", "ext": { "com.other": true } });
    keep_migrated(&mut o, "a", json!(1));
    assert_eq!(o["ext"], json!({ "com.other": true, "app.migrated": { "a": 1 } }));
}
