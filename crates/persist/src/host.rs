//! The identity of this process on this machine (locks, recovery data): user, host, process ID, and whether another
//! process is still running (LOCK-02 "process known dead on the same host", REC-01).
//!
//! Process liveness is asked of the operating system's own tool (`tasklist` on Windows, `kill -0` elsewhere), so no
//! native calls are needed; when the tool cannot answer, liveness is unknown and only the heartbeat decides.

use std::process::Command;
use std::sync::OnceLock;

/// The user name (`USERNAME` / `USER`), or `unknown`.
#[must_use]
pub fn user() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "unknown".to_owned())
}

/// The host name (`COMPUTERNAME` / `HOSTNAME` / `hostname`), or `unknown`.
#[must_use]
pub fn host() -> String {
    static HOST: OnceLock<String> = OnceLock::new();
    HOST.get_or_init(|| {
        if let Ok(h) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME"))
            && !h.trim().is_empty()
        {
            return h.trim().to_owned();
        }
        Command::new("hostname")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    })
    .clone()
}

/// This process's ID.
#[must_use]
pub fn pid() -> u32 {
    std::process::id()
}

/// Whether a process with this ID runs on this machine: `Some(true)`, `Some(false)`, or `None` when unknown.
#[must_use]
pub fn process_alive(pid: u32) -> Option<bool> {
    if pid == std::process::id() {
        return Some(true);
    }
    if cfg!(windows) {
        let out =
            Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        Some(text.contains(&format!("\"{pid}\"")))
    } else {
        let out = Command::new("kill").args(["-0", &pid.to_string()]).output().ok()?;
        if out.status.success() {
            return Some(true);
        }
        // EPERM: the process exists but belongs to someone else.
        let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
        if err.contains("not permitted") {
            return Some(true);
        }
        Some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_alive_and_a_bogus_one_is_not() {
        assert_eq!(process_alive(pid()), Some(true));
        // PID 4_000_000_000 does not exist on any supported system.
        assert_ne!(process_alive(4_000_000_000), Some(true));
        assert!(!host().is_empty() && !user().is_empty());
    }
}
