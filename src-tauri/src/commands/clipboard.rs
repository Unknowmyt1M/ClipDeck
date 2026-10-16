use std::sync::Arc;

use serde::Serialize;
use tauri::{Manager, State};

use crate::models::clipboard::ClipboardItem;
use crate::services::ai_service::AiService;
use crate::services::clipboard_service;
use crate::services::clipboard_service::SelfWriteGuard;
use crate::services::database_service::DatabaseService;
use crate::services::image_service::ImageService;
use crate::services::settings_service::SettingsService;

/// Shared state managed by Tauri.
pub struct AppState {
    pub db: Arc<DatabaseService>,
    pub images: Arc<ImageService>,
    pub write_guard: SelfWriteGuard,
    pub settings: Arc<SettingsService>,
    pub ai: Arc<AiService>,
}

#[derive(Serialize)]
pub struct SaveResult {
    pub item: ClipboardItem,
    pub duplicate: bool,
}

#[derive(Serialize)]
pub struct ClearResult {
    pub deleted: usize,
}

/// List clipboard history with optional filter and search query.
///
/// `type_filter` accepts content types plus `pinned`, `favorites`, `today`
/// and `week`.
#[tauri::command]
pub fn get_items(
    state: State<'_, AppState>,
    type_filter: Option<String>,
    query: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
    let limit = limit.unwrap_or(200).clamp(1, 1000);
    state
        .db
        .list_items(type_filter.as_deref(), query.as_deref(), limit)
}

/// Restore an item: copy it back to the system clipboard and bump usage.
#[tauri::command]
pub fn restore_item(state: State<'_, AppState>, id: i64) -> Result<ClipboardItem, String> {
    restore_inner(&state, id)
}

/// Core restore logic shared by `restore_item` and `restore_and_paste`.
fn restore_inner(state: &AppState, id: i64) -> Result<ClipboardItem, String> {
    let item = state
        .db
        .get_item(id)?
        .ok_or_else(|| format!("item {id} not found"))?;

    // Two-phase guard: mark the write in flight so the monitor skips the
    // cycle, then arm it so the next tick adopts our content instead of
    // re-capturing it (a single flag could be consumed before the write
    // landed, producing a phantom duplicate).
    state.write_guard.start_write();
    let write_result = if item.item_type == crate::models::clipboard::ClipboardItemType::Image {
        let path = state.images.full_path(&item.uuid);
        clipboard_service::write_image(&path)
    } else {
        clipboard_service::write_text(&item.content)
    };
    if let Err(e) = write_result {
        state.write_guard.cancel_write();
        return Err(e);
    }
    state.write_guard.finish_write();

    // Reuse the touch path to bump use_count / last_used_at.
    state.db.touch_item(id)?;
    state
        .db
        .get_item(id)?
        .ok_or_else(|| format!("item {id} not found after restore"))
}

/// Quick-pick restore: copy to the clipboard, hide the quick window, then —
/// when enabled — focus the window that was active before the picker opened
/// and send Ctrl+V there. Paste failures never fail the restore (the clip
/// is already on the clipboard); they are logged without content.
#[tauri::command]
pub fn restore_and_paste(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<ClipboardItem, String> {
    let item = restore_inner(&state, id)?;

    // Hide first so the quick window doesn't hold foreground focus.
    if let Some(window) = app.get_webview_window("quick")
        && window.is_visible().unwrap_or(false)
    {
        let _ = window.hide();
    }

    if state.settings.get().auto_paste
        && let Some(target) = crate::commands::system::paste_target()
    {
        std::thread::spawn(move || {
            // Give Windows a beat to process the hide (and shell focus
            // hand-back) before we explicitly focus the target.
            std::thread::sleep(std::time::Duration::from_millis(150));
            if let Err(e) = crate::services::windows_service::paste_into(target) {
                crate::services::logger::error("paste", &e);
            }
        });
    }

    Ok(item)
}

/// Toggle the pinned flag.
#[tauri::command]
pub fn set_item_pinned(
    state: State<'_, AppState>,
    id: i64,
    pinned: bool,
) -> Result<ClipboardItem, String> {
    state.db.set_pinned(id, pinned)
}

/// Toggle the favorite flag.
#[tauri::command]
pub fn set_item_favorite(
    state: State<'_, AppState>,
    id: i64,
    favorite: bool,
) -> Result<ClipboardItem, String> {
    state.db.set_favorite(id, favorite)
}

/// Delete a single item (and its image files, if any).
#[tauri::command]
pub fn delete_item(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    // DB row first, files second: deleting files before a failing DB delete
    // would lose the image while keeping a row whose thumbnail is gone.
    let item = state.db.get_item(id)?;
    let removed = state.db.delete_item(id)?;
    if removed && let Some(item) = item {
        state.images.delete(&item.uuid);
    }
    Ok(removed)
}

/// Clear all history except pinned items.
#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<ClearResult, String> {
    let (deleted, image_uuids) = state.db.clear_history()?;
    // The PNGs of removed image items must go too — otherwise "clear" left
    // every screenshot on disk indefinitely.
    state.images.delete_many(&image_uuids);
    Ok(ClearResult { deleted })
}

/// Thumbnail image for an item as a base64 data URL.
#[tauri::command]
pub fn get_item_image(
    state: State<'_, AppState>,
    id: i64,
    thumb: Option<bool>,
) -> Result<Option<String>, String> {
    let item = match state.db.get_item(id)? {
        Some(item) => item,
        None => return Ok(None),
    };
    let path = if thumb.unwrap_or(true) {
        state.images.thumb_path(&item.uuid)
    } else {
        state.images.full_path(&item.uuid)
    };
    state.images.read_as_data_url(&path)
}

/// Total number of items in history.
#[tauri::command]
pub fn get_item_count(state: State<'_, AppState>) -> Result<i64, String> {
    state.db.count_items()
}
