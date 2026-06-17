pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;

use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use db::Database;
use datasource::DataSourceManager;
use cache::QuoteCache;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize database
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let db = Arc::new(Database::open(app_dir).expect("Failed to open database"));

            // Initialize data source manager (Sina registered first as default)
            let mut ds_manager = DataSourceManager::new();
            ds_manager.register(Box::new(
                crate::datasource::sina::SinaAdapter::new(),
            ));
            ds_manager.register(Box::new(
                crate::datasource::eastmoney::EastmoneyAdapter::new(),
            ));
            ds_manager.register(Box::new(
                crate::datasource::tencent::TencentAdapter::new(),
            ));

            // Restore last used data source from settings
            if let Ok(Some(active)) = db.get_setting("active_datasource") {
                let _ = ds_manager.set_active(&active);
            }

            let ds_manager = Arc::new(ds_manager);

            // Initialize cache and restore from SQLite
            let cache = Arc::new(QuoteCache::new(db.clone()));
            cache.restore_from_db();

            // Manage state
            app.manage(db.clone());
            app.manage(ds_manager.clone());
            app.manage(cache.clone());

            // Start background polling
            let interval: u64 = db
                .get_setting("refresh_interval")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3);

            crate::cache::Scheduler::spawn(
                ds_manager,
                cache,
                db.clone(),
                app.handle().clone(),
                interval,
            );

            // ── System Tray ──
            let show_item = MenuItemBuilder::with_id("show", "显示主界面").build(app)?;
            let toggle_ticker = MenuItemBuilder::with_id("toggle_ticker", "显示/隐藏行情条").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&toggle_ticker)
                .separator()
                .item(&quit_item)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .expect("Default window icon not embedded — check tauri.conf.json icons config"),
                )
                .tooltip("QuantDesktop")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "toggle_ticker" => {
                            if let Some(window) = app.get_webview_window("ticker") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_always_on_top(true);
                                    // Reposition to bottom-right
                                    if let Ok(Some(monitor)) = window.primary_monitor() {
                                        let size = monitor.size();
                                        let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(250, 38));
                                        let x = (size.width as i32).saturating_sub(win_size.width as i32 + 10);
                                        let y = (size.height as i32).saturating_sub(win_size.height as i32 + 46);
                                        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
                                    }
                                }
                            }
                        }
                        "quit" => {
                            // Close windows gracefully before exit
                            if let Some(w) = app.get_webview_window("main") { let _ = w.close(); }
                            if let Some(w) = app.get_webview_window("ticker") { let _ = w.close(); }
                            // Use async sleep to avoid blocking the event loop
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                handle.exit(0);
                            });
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Main window: hide on close, save/restore position and size
            if let Some(main) = app.get_webview_window("main") {
                let main_clone = main.clone();
                let db_clone = db.clone();
                let _ = main.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            // Save position before hiding
                            if let Ok(pos) = main_clone.outer_position() {
                                let _ = db_clone.set_setting("window_x", &pos.x.to_string());
                                let _ = db_clone.set_setting("window_y", &pos.y.to_string());
                            }
                            if let Ok(size) = main_clone.outer_size() {
                                let _ = db_clone.set_setting("window_width", &size.width.to_string());
                                let _ = db_clone.set_setting("window_height", &size.height.to_string());
                            }
                            let _ = main_clone.hide();
                        }
                        tauri::WindowEvent::Moved(pos) => {
                            let _ = db_clone.set_setting("window_x", &pos.x.to_string());
                            let _ = db_clone.set_setting("window_y", &pos.y.to_string());
                        }
                        tauri::WindowEvent::Resized(size) => {
                            let _ = db_clone.set_setting("window_width", &size.width.to_string());
                            let _ = db_clone.set_setting("window_height", &size.height.to_string());
                        }
                        _ => {}
                    }
                });

                // Restore saved window position and size
                if let Ok(Some(w)) = db.get_setting("window_width") {
                    if let Ok(Some(h)) = db.get_setting("window_height") {
                        if let (Ok(w_val), Ok(h_val)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            let _ = main.set_size(tauri::PhysicalSize::new(w_val, h_val));
                        }
                    }
                }
                if let Ok(Some(x)) = db.get_setting("window_x") {
                    if let Ok(Some(y)) = db.get_setting("window_y") {
                        if let (Ok(x_val), Ok(y_val)) = (x.parse::<i32>(), y.parse::<i32>()) {
                            let _ = main.set_position(tauri::PhysicalPosition::new(x_val, y_val));
                        }
                    }
                }
            }

            // Position ticker window at bottom-right of screen
            if let Some(ticker) = app.get_webview_window("ticker") {
                let _ = ticker.set_always_on_top(true);
                if let Ok(Some(monitor)) = ticker.primary_monitor() {
                    let size = monitor.size();
                    let ticker_size = ticker.outer_size().unwrap_or(tauri::PhysicalSize::new(250, 38));
                    let x = (size.width as i32).saturating_sub(ticker_size.width as i32 + 10);
                    let y = (size.height as i32).saturating_sub(ticker_size.height as i32 + 60);
                    let _ = ticker.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::quote::get_quotes,
            commands::quote::get_indices,
            commands::quote::get_depth,
            commands::quote::get_intraday,
            commands::watchlist::get_watchlist,
            commands::watchlist::add_watch,
            commands::watchlist::remove_watch,
            commands::watchlist::reorder_watch,
            commands::watchlist::move_watch_top,
            commands::watchlist::move_watch_up,
            commands::watchlist::move_watch_down,
            commands::watchlist::search_stocks,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::switch_datasource,
            commands::settings::list_datasources,
            commands::window::show_main_window,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start application");
}
