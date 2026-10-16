use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::clipboard::AppState;
use crate::models::clipboard::ClipboardItem;
use crate::models::stats::Stats;
use crate::services::windows_service;

/// Window that was foreground before the quick picker opened — the target
/// for auto-paste after a pick. HWND as isize (Send-type across threads).
static PASTE_TARGET: std::sync::Mutex<Option<isize>> = std::sync::Mutex::new(None);

/// Remember the paste target (call on every quick-window show). `None`
/// (ClipDeck itself was focused) clears any stale target so a pick can
/// never paste into an old window by accident.
pub(crate) fn set_paste_target(hwnd: Option<isize>) {
    *PASTE_TARGET.lock().unwrap_or_else(|e| e.into_inner()) = hwnd;
}

/// Last remembered paste target, if any.
pub(crate) fn paste_target() -> Option<isize> {
    *PASTE_TARGET.lock().unwrap_or_else(|e| e.into_inner())
}

/// Aggregated usage statistics (content-free apart from previews).
#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> Result<Stats, String> {
    state.db.stats()
}

#[derive(Serialize)]
pub struct ExportResult {
    pub path: String,
    pub items: usize,
}

/// Write the full history to a local JSON file and return its path.
/// The file stays on-device; nothing is uploaded.
#[tauri::command]
pub fn export_history(app: AppHandle, state: State<'_, AppState>) -> Result<ExportResult, String> {
    let items = state.db.export_items()?;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create exports dir: {e}"))?;

    let ts = crate::db::queries::now_ms();
    let path = dir.join(format!("clipdeck-export-{ts}.json"));
    let json =
        serde_json::to_string_pretty(&items).map_err(|e| format!("serialize export: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write export: {e}"))?;

    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        items: items.len(),
    })
}

#[derive(Serialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped_duplicates: usize,
    pub skipped_sensitive: usize,
    pub skipped_images: usize,
    pub invalid: usize,
}

/// Hard cap so a malicious/broken file can't stall the UI thread parsing
/// millions of entries.
const MAX_IMPORT_ENTRIES: usize = 10_000;
/// Guard against loading a huge file into memory via the webview.
const MAX_IMPORT_BYTES: usize = 20 * 1024 * 1024;

/// Merge a ClipDeck export (JSON from `export_history`) back into the local
/// history. Runs entirely on-device; images are skipped (the export holds
/// no PNG bytes), duplicates and sensitive items are counted separately.
#[tauri::command]
pub fn import_history(
    app: AppHandle,
    state: State<'_, AppState>,
    json: String,
) -> Result<ImportResult, String> {
    if json.len() > MAX_IMPORT_BYTES {
        return Err("export file is too large (20 MB max)".into());
    }
    let entries: Vec<ClipboardItem> =
        serde_json::from_str(&json).map_err(|_| "not a valid ClipDeck export file".to_string())?;
    let entries: Vec<ClipboardItem> = entries.into_iter().take(MAX_IMPORT_ENTRIES).collect();

    let s = state.settings.get();
    let out = state
        .db
        .import_items(&entries, s.sensitive_detection, s.skip_sensitive)?;

    if out.imported > 0 {
        // Same housekeeping as a live capture (limits/retention), then let
        // the UI refresh via the history-changed event.
        if let Err(e) = state.db.prune(s.history_limit, s.retention_days) {
            crate::services::logger::error("prune", &e);
        }
        let _ = app.emit(
            crate::services::clipboard_service::CLIPBOARD_CHANGED_EVENT,
            (),
        );
    }

    Ok(ImportResult {
        imported: out.imported,
        skipped_duplicates: out.skipped_duplicates,
        skipped_sensitive: out.skipped_sensitive,
        skipped_images: out.skipped_images,
        invalid: out.invalid,
    })
}

#[derive(Serialize)]
pub struct WipeResult {
    pub deleted_rows: usize,
    pub deleted_images: usize,
}

/// Irreversibly delete ALL local clipboard data (history + images + AI
/// metadata). Settings and the API key are kept.
#[tauri::command]
pub fn wipe_data(state: State<'_, AppState>) -> Result<WipeResult, String> {
    let (deleted_rows, image_uuids) = state.db.wipe()?;
    // Delete the files the wipe actually knew about, then sweep the whole
    // directory so strays from deleted rows go too — report what was
    // really removed, not a fabricated number.
    state.images.delete_many(&image_uuids);
    let deleted_images = state.images.delete_all();
    Ok(WipeResult {
        deleted_rows,
        deleted_images,
    })
}

/// Toggle the compact quick-pick window (Ctrl+Shift+V).
#[tauri::command]
pub fn toggle_quick_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("quick")
        .ok_or_else(|| "quick window missing".to_string())?;
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        // Capture the paste target BEFORE taking foreground: this is the
        // window the user was working in and where the picked clip should
        // land. `None` (ClipDeck itself was focused) clears any stale
        // target so we never paste into an old window by accident.
        // (Skips ClipDeck's own windows — see `foreground_window`.)
        set_paste_target(windows_service::foreground_window());
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Hide the quick-pick window (after a selection or Esc).
#[tauri::command]
pub fn hide_quick_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick") {
        let _ = window.hide();
    }
    Ok(())
}

/// Focus the main window (used by the tray / quick window handoff).
#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Apply the "start with Windows" flag immediately (also covered by save).
#[tauri::command]
pub fn set_startup_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<crate::models::settings::Settings, String> {
    windows_service::set_startup_enabled(enabled)?;
    state.settings.update(|s| s.startup = enabled)
}
