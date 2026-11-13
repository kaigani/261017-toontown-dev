//! Independent round-two desktop shell review, part B.
#![allow(clippy::unwrap_used, reason = "review assertions")]
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tt_executor::{ExecutorHandle, ExecutorOptions, Invocation, PersistOptions, UserDirs};
fn root() -> PathBuf {
    std::env::temp_dir().join(format!("tt-part-b-{}", tt_executor::random_token(8)))
}
fn over(r: &Path) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        persist: PersistOptions { user_dirs: Some(UserDirs::under(r)), ..Default::default() },
        ..Default::default()
    })
    .unwrap()
}
fn ok(h: &ExecutorHandle, cmd: &str, p: Value) -> Value {
    let e = h.invoke(Invocation::user(cmd, p));
    assert!(e.error.is_none(), "{cmd}: {e:?}");
    e.result.unwrap_or(Value::Null)
}
mod part_b_settings {
    use super::*;
    #[test]
    fn versions_and_once_notices() {
        for version in [Some(json!("1.0")), Some(json!("1.99")), Some(json!("0.9")), Some(json!(1)), None] {
            let r = root();
            let d = UserDirs::under(&r).settings;
            std::fs::create_dir_all(&d).unwrap();
            let mut v = json!({"format":"toontown-settings","kind":"preferences","data":{"interface.theme":"dark","future.extra":42}});
            if let Some(x) = &version {
                v["formatVersion"] = x.clone();
            }
            std::fs::write(d.join("preferences.json"), v.to_string()).unwrap();
            let h = over(&r);
            let ev = h.subscribe();
            let prefs = ok(&h, "prefs.get", json!({"keys":["interface.theme"]}));
            let valid = version.as_ref().and_then(Value::as_str).is_some_and(|s| s.starts_with("1."));
            assert_eq!(prefs["values"]["interface.theme"], if valid { "dark" } else { "system" });
            let notices: Vec<_> = ev.try_iter().filter(|e| e.payload["messageId"] == "prefs.corrupt").collect();
            assert_eq!(notices.len(), usize::from(!valid));
            for n in notices {
                assert!(Path::new(n.payload["details"]["keptAt"].as_str().unwrap()).exists());
            }
            drop(h);
            let h = over(&r);
            let ev = h.subscribe();
            ok(&h, "app.state.get", json!({}));
            assert!(ev.try_iter().all(|e| e.payload["messageId"] != "prefs.corrupt"));
            drop(h);
            std::fs::remove_dir_all(r).unwrap();
        }
    }
    #[test]
    fn cleanup_lock_is_nonblocking_and_released() {
        let r = root();
        std::fs::create_dir_all(&r).unwrap();
        let lock =
            std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(r.join(".settings.lock")).unwrap();
        lock.lock_shared().unwrap();
        let p = r.join(".preferences.json.abcd.tmp");
        std::fs::write(&p, b"live").unwrap();
        let start = std::time::Instant::now();
        assert_eq!(tt_persist::settings::clean_temporaries(&r), 0);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        assert!(p.exists());
        drop(lock);
        assert_eq!(tt_persist::settings::clean_temporaries(&r), 1);
        assert!(r.join(".settings.lock").exists());
        let mut copies = Vec::new();
        for text in ["first", "second"] {
            std::fs::write(r.join("preferences.json"), text).unwrap();
            copies.push(tt_persist::settings::set_aside(&r.join("preferences.json")).unwrap());
        }
        assert_ne!(copies[0], copies[1]);
        assert_eq!(std::fs::read_to_string(&copies[0]).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(&copies[1]).unwrap(), "second");
        std::fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn failed_write_retries_after_folder_repair() {
        let r = root();
        std::fs::create_dir_all(&r).unwrap();
        std::fs::write(r.join("Settings"), b"blocked").unwrap();
        let h = over(&r);
        let ev = h.subscribe();
        for theme in ["dark", "light"] {
            ok(&h, "prefs.set", json!({"values":{"interface.theme":theme}}));
        }
        let notices: Vec<_> = ev.try_iter().filter(|e| e.payload["messageId"] == "prefs.saveFailed").collect();
        let failed: Vec<_> = notices.iter().filter(|e| e.payload["details"]["kind"] == "preferences").collect();
        assert_eq!(failed.len(), 1);
        assert!(failed[0].payload["details"]["path"].as_str().unwrap().ends_with("preferences.json"));
        assert!(!failed[0].payload["details"]["message"].as_str().unwrap().is_empty());
        std::fs::remove_file(r.join("Settings")).unwrap();
        ok(&h, "prefs.set", json!({"values":{"interface.theme":"dark"}}));
        let disk: Value = serde_json::from_slice(&std::fs::read(r.join("Settings/preferences.json")).unwrap()).unwrap();
        assert_eq!(disk["data"]["interface.theme"], "dark");
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
    }
}
