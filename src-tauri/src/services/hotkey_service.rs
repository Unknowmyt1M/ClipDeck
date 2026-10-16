//! Global hotkey registration (Ctrl+Shift+V quick-pick window).
//!
//! Shortcut strings are parsed by the plugin, so the setting accepts
//! anything it understands (e.g. `Ctrl+Shift+V`, `Ctrl+Alt+Space`).

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

/// Default hotkey baked into fresh installs.
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+V";

/// Single handler used for every hotkey the app registers.
fn on_shortcut(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        let _ = crate::commands::system::toggle_quick_window(app.clone());
    }
}

/// Register `shortcut`, replacing any previous registration first.
/// Returns a human-readable error for invalid or already-taken combos.
pub fn bind(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    if shortcut.trim().is_empty() {
        return Err("hotkey must not be empty".into());
    }
    // Best effort: the previous combo may not be registered.
    let _ = app.global_shortcut().unregister(shortcut);
    app.global_shortcut()
        .on_shortcut(shortcut, on_shortcut)
        .map_err(|e| format!("could not register hotkey '{shortcut}': {e}"))
}

/// Swap the active hotkey: register `new` first, then unregister
/// `previous`. The old combo is only dropped once the new one is live, so
/// an invalid or taken combo never leaves the user without a shortcut —
/// and the old registration can't linger and fire alongside the new one.
pub fn rebind(app: &AppHandle, new: &str, previous: &str) -> Result<(), String> {
    let new = new.trim();
    bind(app, new)?;
    let previous = previous.trim();
    if !previous.is_empty() && previous != new {
        // Best effort: the previous combo may not be registered.
        let _ = app.global_shortcut().unregister(previous);
    }
    Ok(())
}
