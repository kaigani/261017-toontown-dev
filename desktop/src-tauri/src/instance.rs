//! Single instance (01 §1.1 rule 1; M4 C4.1): one application instance per user session. The first instance listens
//! on a loopback port and records it, with a random secret, in `<settings>/instance.json` (owner-only). A second
//! instance sends the documents it was asked to open to that port, with the secret, and exits; the first opens them.
//! No new dependency: `std::net` only.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

/// The file that names the running instance.
fn record(settings: &Path) -> PathBuf {
    settings.join("instance.json")
}

fn secret() -> String {
    // 16 random bytes from the executor's token generator (no new dependency).
    tt_executor::random_token(16)
}

/// What starting found.
pub enum Start {
    /// This is the first instance; requests from later ones arrive on the listener (none when no loopback port could
    /// be bound: forwarding is then unavailable, the instance still runs).
    First(Option<TcpListener>, String),
    /// Another instance is running and took the documents; this one exits.
    Forwarded,
}

/// Becomes the first instance, or forwards `open` to the running one.
pub fn claim(settings: &Path, open: &[PathBuf]) -> Start {
    if let Ok(text) = std::fs::read_to_string(record(settings))
        && let Ok(v) = serde_json::from_str::<Value>(&text)
        && let (Some(port), Some(key)) = (v.get("port").and_then(Value::as_u64), v.get("secret").and_then(Value::as_str))
        && let Ok(port) = u16::try_from(port)
    {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            let msg = json!({ "secret": key, "open": open.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>() });
            if writeln!(s, "{msg}").is_ok() {
                let mut ack = String::new();
                if BufReader::new(&s).read_line(&mut ack).is_ok() && ack.trim() == "ok" {
                    return Start::Forwarded;
                }
            }
        }
    }
    // No live instance answered: become it (a stale record is replaced).
    match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))) {
        Ok(listener) => {
            let key = secret();
            if let Ok(addr) = listener.local_addr() {
                let _ = std::fs::create_dir_all(settings);
                let text = json!({ "port": addr.port(), "secret": key, "processId": std::process::id() }).to_string();
                let path = record(settings);
                let _ = std::fs::write(&path, text);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
            }
            Start::First(Some(listener), key)
        }
        Err(_) => Start::First(None, secret()),
    }
}

/// Serves forwarding requests: each verified request's documents go to `on_open`.
pub fn serve(listener: TcpListener, key: String, on_open: impl Fn(Vec<PathBuf>) + Send + 'static) {
    let _ = std::thread::Builder::new().name("tt-instance".into()).spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut s) = conn else { continue };
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            let mut line = String::new();
            if BufReader::new(&s).read_line(&mut line).is_err() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if v.get("secret").and_then(Value::as_str) != Some(key.as_str()) {
                continue;
            }
            let _ = writeln!(s, "ok");
            let open: Vec<PathBuf> = v
                .get("open")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect())
                .unwrap_or_default();
            on_open(open);
        }
    });
}

/// Removes the record at a clean exit (only if it is still this process's).
pub fn release(settings: &Path) {
    let path = record(settings);
    let mine = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("processId").and_then(Value::as_u64))
        == Some(u64::from(std::process::id()));
    if mine {
        let _ = std::fs::remove_file(path);
    }
}
