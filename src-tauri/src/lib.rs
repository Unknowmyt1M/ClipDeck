use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use crate::commands::clipboard::AppState;
use crate::services::clipboard_service;
use crate::services::database_service::DatabaseService;
use crate::services::image_service::ImageService;

pub mod commands;
pub mod db;
pub mod models;
pub mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            commands::clipboard::get_items,
            commands::clipboard::restore_item,
            commands::clipboard::set_item_pinned,
            commands::clipboard::set_item_favorite,
            commands::clipboard::delete_item,
            commands::clipboard::clear_history,
            commands::clipboard::get_item_image,
            commands::clipboard::get_item_count,
        ])
        .on_window_event(|window, event| {
            // Closing the window hides it; the tray icon keeps the app alive.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;

    let db = Arc::new(DatabaseService::new(data_dir.join("clipdeck.db"))?);
    let images = Arc::new(ImageService::new(data_dir)?);
    let write_guard = clipboard_service::start_monitor(
        app.handle().clone(),
        Arc::clone(&db),
        Arc::clone(&images),
    );

    app.manage(AppState {
        db,
        images,
        write_guard,
    });

    setup_tray(app)?;
    Ok(())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_i = MenuItem::with_id(app, "show", "Show ClipDeck", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("ClipDeck")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
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
