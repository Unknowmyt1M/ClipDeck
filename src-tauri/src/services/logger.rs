//! Minimal on-disk logger.
//!
//! Rules: never log clipboard content, API keys, or window titles — only
//! scope + error kind. Log file lives at `<app_data>/logs/clipdeck.log` and
//! is capped/rotated so it can never grow unbounded.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Rotate the log once it passes this size.
const MAX_LOG_BYTES: u64 = 1_024 * 1_024;

static LOG_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Point the logger at `<app_data>/logs` and install a panic hook that
/// records panics (message + location only — no clipboard data).
pub fn init(logs_dir: PathBuf) {
    let _ = std::fs::create_dir_all(&logs_dir);
    let path = logs_dir.join("clipdeck.log");
    if let Ok(meta) = std::fs::metadata(&path)
        && meta.len() > MAX_LOG_BYTES
    {
        let _ = std::fs::rename(&path, logs_dir.join("clipdeck.log.old"));
    }
    *LOG_PATH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(path);
    install_panic_hook();
}

/// Append one line to the log (and mirror to stderr).
pub fn log(line: &str) {
    let stamp = chrono_free_timestamp();
    eprintln!("[{stamp}] {line}");
    let slot = LOG_PATH.get();
    let Some(slot) = slot else { return };
    // Recover instead of panicking: a poisoned logger must not take the
    // panic hook (or any caller) down with it.
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = guard.as_ref() else { return };

    // Rotate here, not only at init: a long-running session could grow the
    // file past the cap unbounded between restarts. Writes are rare (error
    // scope only), so the metadata check is free in practice.
    if let Ok(meta) = std::fs::metadata(path)
        && meta.len() > MAX_LOG_BYTES
    {
        let _ = std::fs::rename(path, path.with_extension("log.old"));
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{stamp}] {line}");
    }
}

/// Log an error with a scope tag (used in place of `eprintln!`).
pub fn error(scope: &str, msg: &str) {
    // Error strings are constructed by us/infra and never embed clipboard
    // content (see working rules in docs/architecture.md).
    log(&format!("[{scope}] {msg}"));
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Location + message only.
        log(&format!("PANIC {info}"));
        previous(info);
    }));
}

/// `YYYY-MM-DD HH:MM:SS` without pulling in a time crate.
fn chrono_free_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Howard Hinnant's civil_from_days (UTC).
    let days = (now / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    let secs_of_day = now.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_is_well_formed() {
        let ts = chrono_free_timestamp();
        // The clock is UTC — mark it so log timestamps aren't misread as
        // local time.
        assert!(ts.ends_with('Z'));
        let ts = ts.trim_end_matches('Z');
        assert_eq!(ts.len(), 19);
        assert_eq!(ts.chars().filter(|c| *c == '-').count(), 2);
        assert_eq!(ts.chars().filter(|c| *c == ':').count(), 2);
    }

    #[test]
    fn log_without_init_does_not_panic() {
        log("test line before init");
    }
}
