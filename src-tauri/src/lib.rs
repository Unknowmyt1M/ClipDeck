use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use crate::commands::clipboard::AppState;
use crate::services::ai_service::{self, AiService};
use crate::services::clipboard_service;
use crate::services::database_service::DatabaseService;
use crate::services::hotkey_service;
use crate::services::image_service::ImageService;
use crate::services::logger;
use crate::services::settings_service::SettingsService;
use crate::services::windows_service;

pub mod ai;
pub mod commands;
pub mod db;
pub mod models;
pub mod security;
pub mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            // clipboard
            commands::clipboard::get_items,
            commands::clipboard::restore_item,
            commands::clipboard::restore_and_paste,
            commands::clipboard::set_item_pinned,
            commands::clipboard::set_item_favorite,
            commands::clipboard::delete_item,
            commands::clipboard::clear_history,
            commands::clipboard::get_item_image,
            commands::clipboard::get_item_count,
            // settings + privacy
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::set_api_key,
            commands::settings::has_api_key,
            commands::settings::clear_api_key,
            commands::system::set_startup_enabled,
            // ai
            commands::ai::ai_list_models,
            commands::ai::ai_test_connection,
            commands::ai::ai_get_status,
            commands::ai::ai_analyze_item,
            commands::ai::ai_analyze_pending,
            commands::ai::ai_transform,
            // stats / data management
            commands::system::get_stats,
            commands::system::export_history,
            commands::system::import_history,
            commands::system::wipe_data,
            // windows
            commands::system::toggle_quick_window,
            commands::system::hide_quick_window,
            commands::system::show_main_window,
        ])
        .on_window_event(|window, event| {
            // The quick picker behaves like a native popup: clicking
            // anywhere outside (focus loss) dismisses it. The close button
            // just hides; the main window hides to tray unless disabled.
            if window.label() == "quick"
                && let WindowEvent::Focused(false) = event
            {
                let _ = window.hide();
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "quick" {
                    let _ = window.hide();
                    api.prevent_close();
                    return;
                }
                let close_to_tray = window
                    .app_handle()
                    .state::<AppState>()
                    .settings
                    .get()
                    .close_to_tray;
                if close_to_tray {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    services::logger::init(data_dir.join("logs"));

    let db = Arc::new(DatabaseService::new(data_dir.join("clipdeck.db"))?);
    let images = Arc::new(ImageService::new(data_dir)?);
    let settings = Arc::new(SettingsService::new(Arc::clone(&db))?);
    let ai = AiService::start(Arc::clone(&db), Arc::clone(&settings));
    ai_service::set_app_handle(app.handle().clone());

    let write_guard = clipboard_service::start_monitor(
        app.handle().clone(),
        Arc::clone(&db),
        Arc::clone(&images),
        Arc::clone(&settings),
        Arc::clone(&ai),
    );

    // Start-up sweep: delete image files whose rows no longer exist (a
    // crash mid-capture, an old bug, or manual tampering). Runs before the
    // monitor's first poll (400 ms away), so no capture races it.
    match db.image_uuids() {
        Ok(keepers) => {
            let removed = images.cleanup_orphans(&keepers);
            if removed > 0 {
                logger::log(&format!("[images] swept {removed} orphaned file(s)"));
            }
        }
        Err(e) => logger::error("images", &e),
    }

    let s = settings.get();

    // Startup registration follows the persisted setting.
    if s.startup
        && let Err(e) = windows_service::set_startup_enabled(true)
    {
        logger::error("startup", &e);
    }

    // Global hotkey → quick-pick window.
    let hotkey = if s.global_hotkey.trim().is_empty() {
        hotkey_service::DEFAULT_HOTKEY
    } else {
        s.global_hotkey.trim()
    };
    if let Err(e) = hotkey_service::bind(app.handle(), hotkey) {
        logger::error("hotkey", &e);
    }

    app.manage(AppState {
        db,
        images,
        write_guard,
        settings,
        ai,
    });

    setup_tray(app)?;
    Ok(())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_i = MenuItem::with_id(app, "show", "Show ClipDeck", true, None::<&str>)?;
    let quick_i = MenuItem::with_id(
        app,
        "quick",
        "Quick pick (Ctrl+Shift+V)",
        true,
        None::<&str>,
    )?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quick_i, &quit_i])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("ClipDeck")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quick" => {
                let _ = commands::system::toggle_quick_window(app.clone());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
