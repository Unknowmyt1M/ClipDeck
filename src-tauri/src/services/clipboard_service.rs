use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::models::clipboard::{detect_type, make_preview};
use crate::security::sensitive_detector;
use crate::services::ai_service::AiService;
use crate::services::database_service::{CaptureMeta, DatabaseService};
use crate::services::image_service::ImageService;
use crate::services::settings_service::SettingsService;
use crate::services::source_service;

/// Event emitted whenever the clipboard history changes.
pub const CLIPBOARD_CHANGED_EVENT: &str = "clipboard-changed";

/// Poll interval for clipboard change detection.
const POLL_INTERVAL: Duration = Duration::from_millis(400);

/// Cap text content so a single huge copy can't bloat the database.
const MAX_TEXT_BYTES: usize = 1_000_000;

/// Monitor state for writes ClipDeck itself makes to the clipboard.
///
/// Two-phase on purpose: a single "did write" flag could be consumed by a
/// poll tick that woke up *between* arming and the write landing — the
/// monitor would then sync to the *old* clipboard hash and re-capture the
/// restored item on the next tick (a phantom use-count bump).
const GUARD_IDLE: u8 = 0;
/// A restore is writing right now: the poller skips the cycle.
const GUARD_WRITING: u8 = 1;
/// Write finished: the next poll syncs `last_hash` instead of capturing.
const GUARD_ARMED: u8 = 2;

pub struct SelfWriteGuard(Arc<AtomicU8>);

impl SelfWriteGuard {
    /// Call right before writing to the system clipboard.
    pub fn start_write(&self) {
        self.0.store(GUARD_WRITING, Ordering::SeqCst);
    }
    /// Call after a successful write; the next poll adopts the new hash.
    pub fn finish_write(&self) {
        self.0.store(GUARD_ARMED, Ordering::SeqCst);
    }
    /// Call when the write failed: the clipboard is unchanged.
    pub fn cancel_write(&self) {
        self.0.store(GUARD_IDLE, Ordering::SeqCst);
    }
}

/// Polls the Windows clipboard and persists new content.
///
/// Runs on a dedicated thread; a failure to read the clipboard one cycle
/// (e.g. another app holds it open) is skipped, not fatal.
pub fn start_monitor(
    app: AppHandle,
    db: Arc<DatabaseService>,
    images: Arc<ImageService>,
    settings: Arc<SettingsService>,
    ai: Arc<AiService>,
) -> SelfWriteGuard {
    let flag = Arc::new(AtomicU8::new(GUARD_IDLE));
    let guard = SelfWriteGuard(flag.clone());

    std::thread::spawn(move || {
        let mut last_hash: Option<String> = None;
        loop {
            std::thread::sleep(POLL_INTERVAL);
            if let Err(e) = poll_once(&app, &db, &images, &settings, &ai, &flag, &mut last_hash) {
                // Transient clipboard access errors are expected; only log
                // the error kind, never clipboard content.
                crate::services::logger::error("clipboard", &e);
            }
        }
    });

    guard
}

fn poll_once(
    app: &AppHandle,
    db: &DatabaseService,
    images: &ImageService,
    settings: &SettingsService,
    ai: &AiService,
    flag: &AtomicU8,
    last_hash: &mut Option<String>,
) -> Result<(), String> {
    // Our own write is still in flight → do nothing this cycle (reading
    // now would sync the pre-write hash and re-capture the restore next
    // tick). Once it landed, adopt the new clipboard hash instead of
    // treating it as an external copy.
    match flag.load(Ordering::SeqCst) {
        GUARD_WRITING => return Ok(()),
        GUARD_ARMED
            if flag
                .compare_exchange(GUARD_ARMED, GUARD_IDLE, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok() =>
        {
            if let Some(hash) = read_clipboard_hash() {
                *last_hash = Some(hash);
            }
            return Ok(());
        }
        _ => {}
    }

    // Monitoring paused → track the clipboard silently so resuming doesn't
    // import a stale entry.
    if settings.get().pause_monitoring {
        *last_hash = read_clipboard_hash().or_else(|| last_hash.clone());
        return Ok(());
    }

    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;

    // Text (and URLs) take priority: most copies are text, and many apps
    // leave a stale image format alongside new text.
    match clipboard.get_text() {
        Ok(text) => {
            let text = text.trim().to_string();
            if text.is_empty() {
                return Ok(());
            }
            let hash = hash_bytes(text.as_bytes());
            if last_hash.as_deref() == Some(hash.as_str()) {
                return Ok(());
            }
            *last_hash = Some(hash.clone());

            let s = settings.get();
            let source = source_service::detect_source();
            if is_excluded(&s, source.as_ref()) {
                return Ok(());
            }

            let sensitive = s.sensitive_detection && sensitive_detector::detect(&text).is_some();
            if sensitive && s.skip_sensitive {
                // Detected as secret AND user asked not to store secrets.
                return Ok(());
            }

            let truncated = if text.len() > MAX_TEXT_BYTES {
                let mut end = MAX_TEXT_BYTES;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                &text[..end]
            } else {
                &text
            };
            let item_type = detect_type(truncated);
            let preview = make_preview(truncated);
            let (item, duplicate) = db.save_item(
                item_type.as_str(),
                truncated,
                &hash,
                &preview,
                &CaptureMeta { sensitive, source },
            )?;
            if !duplicate {
                after_new_item(
                    db,
                    images,
                    ai,
                    &s,
                    item.id,
                    sensitive,
                    item.item_type.as_str(),
                )?;
            }
            emit_changed(app);
        }
        Err(_) => {
            // No text on the clipboard — try an image.
            let img = match clipboard.get_image() {
                Ok(img) => img,
                Err(_) => return Ok(()), // nothing new to capture
            };
            let hash = hash_bytes(&img.bytes);
            if last_hash.as_deref() == Some(hash.as_str()) {
                return Ok(());
            }
            *last_hash = Some(hash.clone());

            let s = settings.get();
            let source = source_service::detect_source();
            if is_excluded(&s, source.as_ref()) {
                return Ok(());
            }

            let preview = format!("[Image {}×{}]", img.width, img.height);
            // Content holds a hash marker; the actual PNG files are keyed by
            // the item UUID returned from the insert.
            let (item, duplicate) = db.save_item(
                "image",
                &format!("image:{hash}"),
                &hash,
                &preview,
                &CaptureMeta {
                    sensitive: false,
                    source,
                },
            )?;
            if !duplicate {
                let _ =
                    images.save_rgba(&item.uuid, img.width as u32, img.height as u32, &img.bytes);
                after_new_item(db, images, ai, &s, item.id, false, "image")?;
            }
            emit_changed(app);
        }
    }

    Ok(())
}

/// Post-capture housekeeping: retention prune + optional auto AI analysis.
fn after_new_item(
    db: &DatabaseService,
    images: &ImageService,
    ai: &AiService,
    s: &crate::models::settings::Settings,
    id: i64,
    sensitive: bool,
    item_type: &str,
) -> Result<(), String> {
    let keepers = db.prune(s.history_limit, s.retention_days)?;
    images.cleanup_orphans(&keepers);

    if s.ai_enabled && s.ai_auto_analyze && !sensitive && item_type != "image" {
        ai.enqueue_analyze(id);
    }
    Ok(())
}

/// Exclusion list check (executable name or window-title substring).
fn is_excluded(
    s: &crate::models::settings::Settings,
    source: Option<&source_service::SourceInfo>,
) -> bool {
    let Some(src) = source else {
        return false;
    };
    let exe = src.application_executable.to_lowercase();
    let title = src.window_title.to_lowercase();
    s.excluded_apps_list()
        .iter()
        .any(|app| !app.is_empty() && exe.contains(app.as_str()))
        || s.excluded_titles_list()
            .iter()
            .any(|t| !t.is_empty() && title.contains(t.as_str()))
}

fn read_clipboard_hash() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    match clipboard.get_text() {
        Ok(t) if !t.trim().is_empty() => Some(hash_bytes(t.as_bytes())),
        _ => match clipboard.get_image() {
            Ok(img) => Some(hash_bytes(&img.bytes)),
            Err(_) => None,
        },
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn emit_changed(app: &AppHandle) {
    let _ = app.emit(CLIPBOARD_CHANGED_EVENT, ());
}

/// Write text to the system clipboard (used by restore).
pub fn write_text(text: &str) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("failed to set clipboard text: {e}"))
}

/// Write an image (from an RGBA PNG file) back to the system clipboard.
pub fn write_image(path: &std::path::Path) -> Result<(), String> {
    let img = image::open(path).map_err(|e| format!("failed to load image: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into_raw().into(),
        })
        .map_err(|e| format!("failed to set clipboard image: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard must stay in WRITING while a restore is mid-flight (so
    /// the poller can't consume it early) and only arm after completion.
    #[test]
    fn guard_write_lifecycle() {
        let flag = Arc::new(AtomicU8::new(GUARD_IDLE));
        let guard = SelfWriteGuard(Arc::clone(&flag));

        guard.start_write();
        assert_eq!(flag.load(Ordering::SeqCst), GUARD_WRITING);

        guard.finish_write();
        assert_eq!(flag.load(Ordering::SeqCst), GUARD_ARMED);

        // The poller consumes ARMED → IDLE atomically.
        let consumed =
            flag.compare_exchange(GUARD_ARMED, GUARD_IDLE, Ordering::SeqCst, Ordering::SeqCst);
        assert!(consumed.is_ok());
        assert_eq!(flag.load(Ordering::SeqCst), GUARD_IDLE);

        // A failed write returns the guard to IDLE instead of sticking.
        guard.start_write();
        guard.cancel_write();
        assert_eq!(flag.load(Ordering::SeqCst), GUARD_IDLE);
    }
}
