use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::commands::clipboard::AppState;
use crate::models::stats::Stats;
use crate::services::windows_service;

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
pub struct WipeResult {
    pub deleted_rows: usize,
    pub deleted_images: usize,
}

/// Irreversibly delete ALL local clipboard data (history + images + AI
/// metadata). Settings and the API key are kept.
#[tauri::command]
pub fn wipe_data(state: State<'_, AppState>) -> Result<WipeResult, String> {
    let (deleted_rows, image_uuids) = state.db.wipe()?;
    let before = image_uuids.len();
    state.images.delete_all();
    Ok(WipeResult {
        deleted_rows,
        deleted_images: before.max(deleted_rows),
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
