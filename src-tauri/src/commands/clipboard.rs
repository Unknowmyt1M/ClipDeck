use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::models::clipboard::ClipboardItem;
use crate::services::clipboard_service;
use crate::services::clipboard_service::SelfWriteGuard;
use crate::services::database_service::DatabaseService;
use crate::services::image_service::ImageService;

/// Shared state managed by Tauri.
pub struct AppState {
    pub db: Arc<DatabaseService>,
    pub images: Arc<ImageService>,
    pub write_guard: SelfWriteGuard,
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

/// List clipboard history with optional type filter and search query.
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
    let item = state
        .db
        .get_item(id)?
        .ok_or_else(|| format!("item {id} not found"))?;

    // Arm the guard before writing so the monitor ignores our own write.
    state.write_guard.arm();

    if item.item_type == crate::models::clipboard::ClipboardItemType::Image {
        let path = state.images.full_path(&item.uuid);
        clipboard_service::write_image(&path)?;
    } else {
        clipboard_service::write_text(&item.content)?;
    }

    // Reuse the touch path to bump use_count / last_used_at.
    state.db.touch_item(id)?;
    state
        .db
        .get_item(id)?
        .ok_or_else(|| format!("item {id} not found after restore"))
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
    if let Some(item) = state.db.get_item(id)? {
        state.images.delete(&item.uuid);
    }
    state.db.delete_item(id)
}

/// Clear all history except pinned items.
#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<ClearResult, String> {
    let deleted = state.db.clear_history()?;
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
