use tauri::State;

use crate::commands::clipboard::AppState;
use crate::models::settings::Settings;
use crate::security::secrets;
use crate::services::windows_service;

/// Read current settings (never includes secrets).
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.get())
}

/// Persist settings and apply OS-level side effects (startup entry, hotkey).
#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let previous = state.settings.get();

    // Registry first: if Windows refuses, surface the error and keep the
    // old value instead of persisting a lie.
    if settings.startup != previous.startup {
        windows_service::set_startup_enabled(settings.startup)?;
    }

    // Rebind the global hotkey before persisting so an invalid combo is
    // reported while the old one still works.
    let mut settings = settings;
    if settings.global_hotkey != previous.global_hotkey {
        crate::services::hotkey_service::bind(&app, settings.global_hotkey.trim())?;
    }

    // Sensible bounds — a bad UI value shouldn't wipe history.
    settings.history_limit = settings.history_limit.clamp(10, 100_000);
    settings.retention_days = settings.retention_days.clamp(0, 3650);
    if settings.theme != "light" && settings.theme != "dark" && settings.theme != "system" {
        settings.theme = "system".into();
    }
    if settings.ai_provider != "openrouter" {
        settings.ai_provider = "openrouter".into();
    }

    let saved = state.settings.replace(settings)?;
    apply_now(&state, &saved);
    Ok(saved)
}

/// Side effects that must take effect without an app restart.
fn apply_now(state: &State<'_, AppState>, s: &Settings) {
    if s.ai_auto_analyze && s.ai_enabled && !s.ai_model.is_empty() {
        state.ai.enqueue_pending();
    }
}

/// Save the OpenRouter API key to the OS credential vault (never logged).
#[tauri::command]
pub fn set_api_key(key: String) -> Result<bool, String> {
    secrets::store_openrouter_api_key(&key)?;
    Ok(secrets::has_openrouter_api_key())
}

/// Whether an API key is configured (only a boolean crosses the IPC line).
#[tauri::command]
pub fn has_api_key() -> bool {
    secrets::has_openrouter_api_key()
}

/// Remove the stored API key.
#[tauri::command]
pub fn clear_api_key() -> Result<bool, String> {
    secrets::clear_openrouter_api_key()?;
    Ok(false)
}
