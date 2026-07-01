pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;

use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use simplelog::{CombinedLogger, WriteLogger, TermLogger, LevelFilter, Config, TerminalMode, ColorChoice};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use db::Database;
use datasource::DataSourceManager;
use cache::QuoteCache;

/// Windows-only utility: add WS_EX_TOOLWINDOW to a window's extended style.
/// This permanently hides the window from the taskbar (survives Explorer
/// restarts), unlike the COM-based ITaskbarList::DeleteTab approach used by
/// Tauri's `set_skip_taskbar`.
#[cfg(target_os = "windows")]
mod windows_util {
    use std::ffi::c_void;
    type HWND = *mut c_void;

    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x80;

    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    extern "system" {
        fn GetWindowLongPtrW(hwnd: HWND, nIndex: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
        fn SetWindowPos(
            hwnd: HWND,
            hwndInsertAfter: HWND,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
    }

    /// Set WS_EX_TOOLWINDOW on a window identified by its raw HWND.
    /// Idempotent — skips if the style is already set.
    pub unsafe fn set_tool_window(hwnd: isize) {
        let hwnd_ptr = hwnd as HWND;
        let ex_style = GetWindowLongPtrW(hwnd_ptr, GWL_EXSTYLE);
        if ex_style == 0 {
            log::warn!("[ticker] GetWindowLongPtrW returned 0 — skipping WS_EX_TOOLWINDOW");
            return;
        }
        if ex_style & WS_EX_TOOLWINDOW != 0 {
            return; // already applied
        }
        SetWindowLongPtrW(hwnd_ptr, GWL_EXSTYLE, ex_style | WS_EX_TOOLWINDOW);
        SetWindowPos(
            hwnd_ptr,
            std::ptr::null_mut(),
            0, 0, 0, 0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        log::info!("[ticker] WS_EX_TOOLWINDOW applied — permanently hidden from taskbar");
    }
}

/// Apply WS_EX_TOOLWINDOW to a Tauri window so it stays hidden from the
/// Windows taskbar even after Explorer restarts.  No-op on non-Windows.
fn apply_tool_window_style(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::HasWindowHandle;
        if let Ok(handle) = window.window_handle() {
            if let raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                unsafe {
                    windows_util::set_tool_window(h.hwnd.get() as isize);
                }
            }
        }
    }
    let _ = window; // suppress unused warning on non-Windows
}

/// Runtime flag indicating whether the app is in portable mode
/// (triggered by the presence of `portable.dat` next to the executable).
#[derive(Debug, Clone, Copy)]
pub struct PortableMode(pub bool);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Data directory:
            // - Portable mode (portable.dat exists next to exe) → <exe_dir>/data/
            // - Normal mode → %APPDATA%/quant-desktop/
            let (app_dir, is_portable) = std::env::current_exe()
                .ok()
                .and_then(|exe| {
                    let marker = exe.with_file_name("portable.dat");
                    marker.exists().then(|| {
                        let dir = exe.parent()
                            .map(|p| p.join("data"))
                            .unwrap_or_else(|| std::path::PathBuf::from("data"));
                        (dir, true)
                    })
                })
                .unwrap_or_else(|| {
                    let dir = dirs::data_dir()
                        .expect("Failed to get system data directory")
                        .join("quant-desktop");
                    (dir, false)
                });

            // Detect local proxy (Clash/V2Ray) for updater downloads
            detect_and_set_proxy();

            // Initialize logger — writes to both stderr (dev) and quant-desktop.log (file)
            std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");
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
            log::info!(
                "Data directory: {:?} (portable: {})",
                app_dir, is_portable
            );

            let db = Arc::new(Database::open(app_dir).expect("Failed to open database"));
            log::info!("Database opened successfully");

            // Initialize data source manager (Sina registered first as default)
            let mut ds_manager = DataSourceManager::new();
            ds_manager.register(Box::new(
                crate::datasource::tencent::TencentAdapter::new(),
            ));
            ds_manager.register(Box::new(
                crate::datasource::sina::SinaAdapter::new(),
            ));

            // Restore last used data source from settings.
            // Use set_active_initial to avoid triggering a duplicate wakeup fetch
            // on startup (the scheduler's main loop handles the first fetch).
            if let Ok(Some(active)) = db.get_setting("active_datasource") {
                match ds_manager.set_active_initial(&active) {
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
            app.manage(PortableMode(is_portable));

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

            // Portable mode: skip the "check update" tray item — updates
            // are managed by the user (download & replace the zip).
            let menu = if is_portable {
                MenuBuilder::new(app)
                    .item(&show_item)
                    .item(&toggle_ticker)
                    .separator()
                    .item(&quit_item)
                    .build()?
            } else {
                let check_update_item =
                    MenuItemBuilder::with_id("check_update", "检查更新").build(app)?;
                MenuBuilder::new(app)
                    .item(&show_item)
                    .item(&toggle_ticker)
                    .separator()
                    .item(&check_update_item)
                    .item(&quit_item)
                    .build()?
            };

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
                                    // Re-hide from taskbar after show
                                    let _ = window.set_skip_taskbar(true);
                                    apply_tool_window_style(&window);
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
                        "check_update" => {
                            // Portable mode: the "check update" tray item is hidden,
                            // but guard here as a safety net.
                            if app.state::<PortableMode>().0 {
                                log::info!("[updater] Tray check_update ignored — portable mode");
                                return;
                            }
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                match crate::commands::updater::do_check_update(&handle).await
                                {
                                    Ok(Some(info)) => {
                                        let _ = handle.emit("update-available", &info);
                                    }
                                    Ok(None) => {
                                        log::info!("[updater] Manual check: already up to date");
                                        let _ = handle.emit("update-check-complete", "up-to-date");
                                    }
                                    Err(e) => {
                                        log::warn!("[updater] Manual check failed: {}", e);
                                        let _ = handle.emit("update-check-complete", "error");
                                    }
                                }
                            });
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
                // Debounced geometry save: Moved/Resized fire on every pixel
                // during drag, but we only persist once the user stops moving
                // the window for 800ms (drag-end behaviour).  This avoids
                // hundreds of DB writes during a single resize/move gesture.
                let save_counter = Arc::new(AtomicU64::new(0));
                let _ = main.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let is_min = main_clone.is_minimized().unwrap_or(false);
                            let is_vis = main_clone.is_visible().unwrap_or(false);
                            if is_vis && !is_min {
                                let is_max = main_clone.is_maximized().unwrap_or(false);
                                let _ = db_clone.set_setting("window_maximized", if is_max { "1" } else { "0" });
                                if !is_max {
                                    if let Ok(pos) = main_clone.outer_position() {
                                        if let Err(e) = db_clone.set_setting("window_x", &pos.x.to_string()) {
                                            log::warn!("Failed to save window_x on close: {}", e);
                                        }
                                        if let Err(e) = db_clone.set_setting("window_y", &pos.y.to_string()) {
                                            log::warn!("Failed to save window_y on close: {}", e);
                                        }
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
                        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                            if main_clone.is_minimized().unwrap_or(false)
                                || !main_clone.is_visible().unwrap_or(false)
                            {
                                return;
                            }
                            // fetch_add returns the PREVIOUS value, so +1 to get
                            // the value WE just set (checked by the debounce task).
                            let count = save_counter.fetch_add(1, Ordering::SeqCst) + 1;
                            let main = main_clone.clone();
                            let db = db_clone.clone();
                            let counter = save_counter.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                                // If counter changed, another event fired during
                                // the wait — the user is still dragging, skip.
                                if counter.load(Ordering::SeqCst) != count {
                                    return;
                                }
                                if let Ok(pos) = main.outer_position() {
                                    let _ = db.set_setting("window_x", &pos.x.to_string());
                                    let _ = db.set_setting("window_y", &pos.y.to_string());
                                }
                                if let Ok(size) = main.outer_size() {
                                    if size.width > 0 && size.height > 0 {
                                        let _ = db.set_setting("window_width", &size.width.to_string());
                                        let _ = db.set_setting("window_height", &size.height.to_string());
                                    }
                                }
                            });
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

                // Read default window size from tauri.conf.json
                let (default_w, default_h) = app.config()
                    .app
                    .windows
                    .iter()
                    .find(|w| w.label == "main")
                    .map(|w| (w.width as u32, w.height as u32))
                    .unwrap_or((1388, 1009));

                // Restore saved geometry if valid
                let (mut saved_w, mut saved_h) = (0u32, 0u32);
                let (mut saved_x, mut saved_y) = (0i32, 0i32);
                let mut has_size = false;
                let mut has_pos = false;

                if let Ok(Some(w)) = db.get_setting("window_width") {
                    if let Ok(Some(h)) = db.get_setting("window_height") {
                        if let (Ok(w_val), Ok(h_val)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            if w_val >= 400 && w_val <= mon_w as u32
                                && h_val >= 300 && h_val <= mon_h as u32
                            {
                                saved_w = w_val;
                                saved_h = h_val;
                                has_size = true;
                            }
                        }
                    }
                }
                if let Ok(Some(x)) = db.get_setting("window_x") {
                    if let Ok(Some(y)) = db.get_setting("window_y") {
                        if let (Ok(x_val), Ok(y_val)) = (x.parse::<i32>(), y.parse::<i32>()) {
                            if x_val + 200 < mon_w && x_val > -50
                                && y_val + 100 < mon_h && y_val > -50
                            {
                                saved_x = x_val.max(0);
                                saved_y = y_val.max(0);
                                has_pos = true;
                            }
                        }
                    }
                }

                let was_max = db.get_setting("window_maximized")
                    .ok()
                    .flatten()
                    .map(|v| v == "1")
                    .unwrap_or(false);

                // Show first so the native NSWindow is realized before applying
                // geometry (required for correct sizing on macOS).
                let _ = main.show();
                if was_max {
                    let w = if has_size { saved_w } else { default_w };
                    let h = if has_size { saved_h } else { default_h };
                    let _ = main.set_size(tauri::PhysicalSize::new(w, h));
                    let _ = main.maximize();
                } else if has_pos {
                    let w = if has_size { saved_w } else { default_w };
                    let h = if has_size { saved_h } else { default_h };
                    let _ = main.set_size(tauri::PhysicalSize::new(w, h));
                    let _ = main.set_position(tauri::PhysicalPosition::new(saved_x, saved_y));
                } else {
                    // No saved geometry: use config defaults and center
                    let _ = main.set_size(tauri::PhysicalSize::new(default_w, default_h));
                    let _ = main.center();
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

                // Save ticker position on move.  Only persist if enough of the
                // ticker is actually visible — if the user drags it way off
                // screen, we skip saving so the next launch falls back to the
                // default bottom-right position.
                let db_clone = db.clone();
                let _ = ticker.on_window_event(move |event| {
                    if let tauri::WindowEvent::Moved(pos) = event {
                        // How much of the ticker is inside the monitor bounds?
                        let visible_left = pos.x.max(0);
                        let visible_right = (pos.x + tw).min(mon_w);
                        let visible_w = (visible_right - visible_left).max(0);
                        let visible_top = pos.y.max(0);
                        let visible_bottom = (pos.y + th).min(mon_h);
                        let visible_h = (visible_bottom - visible_top).max(0);

                        // Require at least 50×20 px visible — otherwise it's
                        // too far off-screen to be easily found.
                        if visible_w < 50 || visible_h < 20 {
                            return;
                        }

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

                // Now show the ticker at the correct position (config has visible: false)
                let _ = ticker.show();
                // Remove ticker from taskbar at both levels:
                //   set_skip_taskbar  → ITaskbarList::DeleteTab (immediate, one-shot)
                //   apply_tool_window → WS_EX_TOOLWINDOW (survives Explorer restart)
                let _ = ticker.set_skip_taskbar(true);
                apply_tool_window_style(&ticker);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::quote::get_quotes,
            commands::quote::get_indices,
            commands::quote::get_depth,
            commands::quote::get_intraday,
            commands::quote::get_kline,
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
            commands::settings::get_portable_mode,
            commands::window::show_main_window,
            commands::updater::check_update,
            commands::updater::install_update,
            commands::updater::is_trading_session,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start application");
}

/// Auto-detect local proxy (Clash/V2Ray) and set env vars for updater downloads.
/// NO_PROXY excludes domestic stock API hosts so quotes/K-line still go direct.
///
/// # Safety
///
/// `std::env::set_var` is NOT thread-safe per Rust's documentation. This function
/// MUST be called during startup, on the main thread, BEFORE any background tasks
/// (scheduler, updater checks, etc.) are spawned. Concurrent reads of the affected
/// env vars from other threads while this function runs is undefined behavior.
fn detect_and_set_proxy() {
    use std::net::TcpStream;
    use std::time::Duration;

    // Skip if proxy already set
    if std::env::var("HTTPS_PROXY").is_ok()
        || std::env::var("HTTP_PROXY").is_ok()
        || std::env::var("https_proxy").is_ok()
        || std::env::var("http_proxy").is_ok()
    {
        return;
    }

    let ports = [7890u16, 10809, 10808, 7891, 1080, 8118, 8080];
    for &port in &ports {
        let addr = format!("127.0.0.1:{}", port);
        if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(1)).is_ok() {
            let proxy = format!("http://{}", addr);
            std::env::set_var("HTTPS_PROXY", &proxy);
            std::env::set_var("HTTP_PROXY", &proxy);
            // Exclude domestic stock APIs from proxy
            std::env::set_var(
                "NO_PROXY",
                "sinajs.cn,sina.com.cn,gtimg.cn,gu.qq.com,qq.com,localhost,127.0.0.1",
            );
            log::info!("[proxy] {} (stock APIs excluded)", proxy);
            return;
        }
    }
    log::info!("[proxy] No local proxy detected");
}
