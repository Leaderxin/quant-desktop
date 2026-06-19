pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;

use std::fs::File;
use std::sync::Arc;
use simplelog::{CombinedLogger, WriteLogger, TermLogger, LevelFilter, Config, TerminalMode, ColorChoice};
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

            // Initialize logger — writes to both stderr (dev) and quant-desktop.log (file)
            let log_file = File::create(app_dir.join("quant-desktop.log"))
                .expect("Failed to create log file");
            CombinedLogger::init(vec![
                TermLogger::new(
                    LevelFilter::Info,
                    Config::default(),
                    TerminalMode::Mixed,
                    ColorChoice::Auto,
                ),
                WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
            ])
            .expect("Failed to initialize logger");
            log::info!("QuantDesktop v{} starting", env!("CARGO_PKG_VERSION"));

            let db = Arc::new(Database::open(app_dir).expect("Failed to open database"));
            log::info!("Database opened successfully");

            // Initialize data source manager (Sina registered first as default)
            let mut ds_manager = DataSourceManager::new();
            ds_manager.register(Box::new(
                crate::datasource::sina::SinaAdapter::new(),
            ));
            ds_manager.register(Box::new(
                crate::datasource::tencent::TencentAdapter::new(),
            ));

            // Restore last used data source from settings
            if let Ok(Some(active)) = db.get_setting("active_datasource") {
                match ds_manager.set_active(&active) {
                    Ok(()) => log::info!("Restored data source: {}", active),
                    Err(e) => log::warn!("Failed to restore data source '{}': {}", active, e),
                }
            }

            let ds_manager = Arc::new(ds_manager);

            // Initialize cache and restore from SQLite
            let cache = Arc::new(QuoteCache::new(db.clone()));
            cache.restore_from_db();
            log::info!("Quote cache initialized and restored from DB");

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
                .on_menu_event({
                    let db = db.clone();
                    move |app, event| {
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
                                    // Try saved position first, fall back to bottom-right
                                    let mon = window.primary_monitor().ok().flatten();
                                    let (mon_w, mon_h) = mon
                                        .as_ref()
                                        .map(|m| { let s = m.size(); (s.width as i32, s.height as i32) })
                                        .unwrap_or((1920, 1080));
                                    let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(
                                        crate::datasource::TICKER_WIDTH,
                                        crate::datasource::TICKER_HEIGHT,
                                    ));
                                    let tw = win_size.width as i32;
                                    let th = win_size.height as i32;

                                    let mut restored = false;
                                    if let Ok(Some(x)) = db.get_setting("ticker_x") {
                                        if let Ok(Some(y)) = db.get_setting("ticker_y") {
                                            if let (Ok(sx), Ok(sy)) = (x.parse::<i32>(), y.parse::<i32>()) {
                                                if sx + tw > 0 && sy + th > 0 && sx < mon_w && sy < mon_h {
                                                    let _ = window.set_position(
                                                        tauri::PhysicalPosition::new(sx, sy),
                                                    );
                                                    restored = true;
                                                }
                                            }
                                        }
                                    }
                                    if !restored {
                                        let x = (mon_w).saturating_sub(tw + 10);
                                        let y = (mon_h).saturating_sub(th + 60);
                                        let _ = window.set_position(
                                            tauri::PhysicalPosition::new(x, y),
                                        );
                                    }
                                }
                            }
                        }
                        "quit" => {
                            if let Some(w) = app.get_webview_window("main") { let _ = w.close(); }
                            if let Some(w) = app.get_webview_window("ticker") { let _ = w.close(); }
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                handle.exit(0);
                            });
                        }
                        _ => {}
                    }
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
                            let is_min = main_clone.is_minimized().unwrap_or(false);
                            let is_vis = main_clone.is_visible().unwrap_or(false);
                            if is_vis && !is_min {
                                if let Ok(pos) = main_clone.outer_position() {
                                    if let Err(e) = db_clone.set_setting("window_x", &pos.x.to_string()) {
                                        log::warn!("Failed to save window_x on close: {}", e);
                                    }
                                    if let Err(e) = db_clone.set_setting("window_y", &pos.y.to_string()) {
                                        log::warn!("Failed to save window_y on close: {}", e);
                                    }
                                }
                                if let Ok(size) = main_clone.outer_size() {
                                    if let Err(e) = db_clone.set_setting("window_width", &size.width.to_string()) {
                                        log::warn!("Failed to save window_width on close: {}", e);
                                    }
                                    if let Err(e) = db_clone.set_setting("window_height", &size.height.to_string()) {
                                        log::warn!("Failed to save window_height on close: {}", e);
                                    }
                                }
                            }
                            let _ = main_clone.hide();
                        }
                        tauri::WindowEvent::Moved(pos) => {
                            if !main_clone.is_minimized().unwrap_or(false)
                                && main_clone.is_visible().unwrap_or(false)
                            {
                                if let Err(e) = db_clone.set_setting("window_x", &pos.x.to_string()) {
                                    log::warn!("Failed to save window_x on move: {}", e);
                                }
                                if let Err(e) = db_clone.set_setting("window_y", &pos.y.to_string()) {
                                    log::warn!("Failed to save window_y on move: {}", e);
                                }
                            }
                        }
                        tauri::WindowEvent::Resized(size) => {
                            if let Err(e) = db_clone.set_setting("window_width", &size.width.to_string()) {
                                log::warn!("Failed to save window_width on resize: {}", e);
                            }
                            if let Err(e) = db_clone.set_setting("window_height", &size.height.to_string()) {
                                log::warn!("Failed to save window_height on resize: {}", e);
                            }
                        }
                        _ => {}
                    }
                });

                // Restore saved window position and size.
                // Validate against actual monitor geometry — skip saved values
                // that would place the window off-screen.
                let (mon_w, mon_h) = main.primary_monitor()
                    .ok()
                    .flatten()
                    .map(|m| {
                        let s = m.size();
                        (s.width as i32, s.height as i32)
                    })
                    .unwrap_or((1920, 1080));

                let default_w: u32 = 1100;
                let default_h: u32 = 680;
                let (mut saved_w, mut saved_h) = (0u32, 0u32);
                if let Ok(Some(w)) = db.get_setting("window_width") {
                    if let Ok(Some(h)) = db.get_setting("window_height") {
                        if let (Ok(w_val), Ok(h_val)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            saved_w = w_val;
                            saved_h = h_val;
                        }
                    }
                }
                let w = if saved_w >= 400 && saved_w <= mon_w as u32 { saved_w } else { default_w };
                let h = if saved_h >= 300 && saved_h <= mon_h as u32 { saved_h } else { default_h };

                let (mut saved_x, mut saved_y) = (0i32, 0i32);
                let mut has_pos = false;
                if let Ok(Some(x)) = db.get_setting("window_x") {
                    if let Ok(Some(y)) = db.get_setting("window_y") {
                        if let (Ok(x_val), Ok(y_val)) = (x.parse::<i32>(), y.parse::<i32>()) {
                            saved_x = x_val;
                            saved_y = y_val;
                            has_pos = true;
                        }
                    }
                }

                // Show first so the native NSWindow is realized before applying
                // geometry (required for correct sizing on macOS).
                let _ = main.show();
                let _ = main.set_size(tauri::PhysicalSize::new(w, h));
                if has_pos
                    && saved_x + 200 < mon_w
                    && saved_y + 100 < mon_h
                    && saved_x > -200
                    && saved_y > -50
                {
                    let _ = main.set_position(tauri::PhysicalPosition::new(saved_x, saved_y));
                }
                let _ = main.set_focus();
            }

            // Ticker window: save position on move (clamped), restore on startup
            if let Some(ticker) = app.get_webview_window("ticker") {
                let _ = ticker.set_always_on_top(true);

                // Capture monitor bounds and ticker size for clamping on move
                let mon = ticker.primary_monitor().ok().flatten();
                let (mon_w, mon_h) = mon
                    .as_ref()
                    .map(|m| { let s = m.size(); (s.width as i32, s.height as i32) })
                    .unwrap_or((1920, 1080));
                let ticker_size = ticker.outer_size().unwrap_or(tauri::PhysicalSize::new(
                    crate::datasource::TICKER_WIDTH,
                    crate::datasource::TICKER_HEIGHT,
                ));
                let tw = ticker_size.width as i32;
                let th = ticker_size.height as i32;

                // Save ticker position on move — clamp to monitor bounds so
                // the entire window stays on screen and is always restorable.
                let db_clone = db.clone();
                let _ = ticker.on_window_event(move |event| {
                    if let tauri::WindowEvent::Moved(pos) = event {
                        let clamped_x = pos.x.max(0).min(mon_w - tw);
                        let clamped_y = pos.y.max(0).min(mon_h - th);
                        if let Err(e) = db_clone.set_setting("ticker_x", &clamped_x.to_string()) {
                            log::warn!("Failed to save ticker_x: {}", e);
                        }
                        if let Err(e) = db_clone.set_setting("ticker_y", &clamped_y.to_string()) {
                            log::warn!("Failed to save ticker_y: {}", e);
                        }
                    }
                });

                // Restore saved position, fall back to bottom-right
                let (mut saved_x, mut saved_y) = (0i32, 0i32);
                let mut has_pos = false;
                if let Ok(Some(x)) = db.get_setting("ticker_x") {
                    if let Ok(Some(y)) = db.get_setting("ticker_y") {
                        if let (Ok(x_val), Ok(y_val)) = (x.parse::<i32>(), y.parse::<i32>()) {
                            saved_x = x_val;
                            saved_y = y_val;
                            has_pos = true;
                        }
                    }
                }
                if has_pos
                    && saved_x + tw > 0
                    && saved_y + th > 0
                    && saved_x < mon_w
                    && saved_y < mon_h
                {
                    let _ = ticker.set_position(tauri::PhysicalPosition::new(saved_x, saved_y));
                } else {
                    let x = (mon_w).saturating_sub(tw + 10);
                    let y = (mon_h).saturating_sub(th + 60);
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
