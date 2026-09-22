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
    Manager,
};
// `Emitter` is used by the tray menu's "check update" handler below, which is
// compile-gated out of store builds (the Microsoft Store distributes updates
// itself). The check_update/install_update IPC commands stay compiled and
// registered in store builds, guarded at runtime — see commands/updater.rs.
#[cfg(not(feature = "store"))]
use tauri::Emitter;
use db::{keys, Database};
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

/// 显示/隐藏行情条窗口，并把可见性落盘到 `ticker_visible`。
///
/// 托盘菜单的「显示/隐藏行情条」与设置页的 `set_ticker_visible` 命令共用这一份
/// 实现。显示时要补的几步(always_on_top / skip_taskbar / WS_EX_TOOLWINDOW /
/// 位置还原)一旦各写一份，迟早会漏掉一处 —— 而漏掉的症状(窗口跑进任务栏、
/// 或出现在屏幕外)在改动另一处时根本看不出来。
///
/// 顺序有依赖:Windows 上 `set_skip_taskbar` 走 ITaskbarList::DeleteTab，
/// 要求窗口已经真正显示过才生效，所以必须先 show 再设这个属性。
pub fn set_ticker_visible(
    app: &tauri::AppHandle,
    db: &Database,
    visible: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("ticker")
        .ok_or_else(|| "Ticker window not found".to_string())?;

    if !visible {
        window.hide().map_err(|e| e.to_string())?;
        let _ = db.set_setting(keys::TICKER_VISIBLE, "0");
        return Ok(());
    }

    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    apply_tool_window_style(&window);

    // 位置:优先恢复上次保存的坐标，越界或首次则回到右下角默认位。
    let mon = window.primary_monitor().ok().flatten();
    let (mon_w, mon_h) = mon
        .as_ref()
        .map(|m| { let s = m.size(); (s.width as i32, s.height as i32) })
        .unwrap_or((1920, 1080));
    let win_size = window.outer_size().unwrap_or_else(|_| {
        // TICKER_* 是逻辑像素，折算物理像素需乘窗口缩放系数，
        // 否则 DPI ≠ 100% 时兜底几何偏小
        let scale = window.scale_factor().unwrap_or(1.0);
        tauri::PhysicalSize::new(
            (crate::datasource::TICKER_WIDTH as f64 * scale) as u32,
            (crate::datasource::TICKER_HEIGHT as f64 * scale) as u32,
        )
    });
    let tw = win_size.width as i32;
    let th = win_size.height as i32;

    let mut restored = false;
    if let Ok(Some(x)) = db.get_setting(keys::TICKER_X) {
        if let Ok(Some(y)) = db.get_setting(keys::TICKER_Y) {
            if let (Ok(sx), Ok(sy)) = (x.parse::<i32>(), y.parse::<i32>()) {
                if sx + tw > 0 && sy + th > 0 && sx < mon_w && sy < mon_h {
                    let _ = window.set_position(tauri::PhysicalPosition::new(sx, sy));
                    restored = true;
                }
            }
        }
    }
    if !restored {
        let x = mon_w.saturating_sub(tw + 10);
        let y = mon_h.saturating_sub(th + 60);
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }

    let _ = db.set_setting(keys::TICKER_VISIBLE, "1");
    Ok(())
}

/// Build the logger set for `app_dir`: terminal output plus a log file in the
/// data directory.
///
/// Neither the log file nor the data directory is essential to the app running,
/// so both failures degrade to a terminal-only logger instead of panicking. A
/// Store build writes through MSIX file-system virtualization, and a panic
/// during `setup` is a silent crash — no window, and no log explaining why.
fn build_loggers(app_dir: &std::path::Path) -> Vec<Box<dyn simplelog::SharedLogger>> {
    let mut loggers: Vec<Box<dyn simplelog::SharedLogger>> = vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    match std::fs::create_dir_all(app_dir)
        .and_then(|()| File::create(app_dir.join("quant-desktop.log")))
    {
        Ok(log_file) => {
            loggers.push(WriteLogger::new(LevelFilter::Info, Config::default(), log_file));
        }
        // Not `log::warn!` — the logger is what we are still building.
        Err(e) => eprintln!("QuantDesktop: file logging disabled, {app_dir:?} unusable: {e}"),
    }

    loggers
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Registry Run-key autostart (macOS: LaunchAgent). Store builds on
        // Windows don't use this — their autostart goes through the packaged
        // StartupTask API instead (see commands/autostart.rs); the plugin stays
        // registered for the remaining build configurations.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ));

    // Store builds skip the built-in updater — the Microsoft Store distributes
    // updates itself, so the updater plugin is not registered at all.
    #[cfg(not(feature = "store"))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
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

            // Detect local proxy (Clash/V2Ray) for updater downloads.
            // Store builds have no updater, so this is compiled out.
            #[cfg(not(feature = "store"))]
            detect_and_set_proxy();

            // Initialize logger — writes to both stderr (dev) and quant-desktop.log (file).
            // Every step degrades instead of panicking; see build_loggers.
            if let Err(e) = CombinedLogger::init(build_loggers(&app_dir)) {
                eprintln!("QuantDesktop: logger already initialized: {e}");
            }
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
            if let Ok(Some(active)) = db.get_setting(keys::ACTIVE_DATASOURCE) {
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
            app.manage(Arc::new(
                crate::datasource::market::MarketOverviewClient::new(),
            ));
            app.manage(PortableMode(is_portable));

            // Start background polling.
            //
            // 没有「基准轮询间隔」这个参数：节奏完全由 market_clock(交易时段) 与
            // 自适应状态机(probe → normal → idle) 决定。历史上曾传过一个
            // refresh_interval 设置值，但 Scheduler 从未引用它 —— 一个在设置表里、
            // 在启动流程里读写、却对行为毫无影响的死键，已连同 `init_defaults` 里的
            // 默认值一起删除。老库里残留的那行无害，不需要清理。
            crate::cache::Scheduler::spawn(
                ds_manager,
                cache,
                db.clone(),
                app.handle().clone(),
            );

            // ── System Tray ──
            let show_item = MenuItemBuilder::with_id("show", "显示主界面").build(app)?;
            let toggle_ticker = MenuItemBuilder::with_id("toggle_ticker", "显示/隐藏行情条").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

            // Portable mode and Store builds both skip the "check update" tray
            // item: portable updates are user-managed (download & replace the
            // zip), and Store updates are handled by the Microsoft Store itself.
            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&toggle_ticker)
                .separator();

            #[cfg(not(feature = "store"))]
            let menu = if is_portable {
                menu
            } else {
                let check_update_item =
                    MenuItemBuilder::with_id("check_update", "检查更新").build(app)?;
                menu.item(&check_update_item)
            };

            let menu = menu.item(&quit_item).build()?;

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
                            let was_visible = app
                                .get_webview_window("ticker")
                                .and_then(|w| w.is_visible().ok())
                                .unwrap_or(false);
                            // 显示/隐藏的全部细节(置顶、隐藏任务栏、位置还原、
                            // 可见性落盘)都在 set_ticker_visible 里，与设置页
                            // 的开关共用同一份实现。
                            if let Err(e) = set_ticker_visible(app, &db, !was_visible) {
                                log::warn!("Failed to toggle ticker window: {}", e);
                            }
                        }
                        #[cfg(not(feature = "store"))]
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
                                let _ = db_clone.set_setting(keys::WINDOW_MAXIMIZED, if is_max { "1" } else { "0" });
                                if !is_max {
                                    if let Ok(pos) = main_clone.outer_position() {
                                        if let Err(e) = db_clone.set_setting(keys::WINDOW_X, &pos.x.to_string()) {
                                            log::warn!("Failed to save window_x on close: {}", e);
                                        }
                                        if let Err(e) = db_clone.set_setting(keys::WINDOW_Y, &pos.y.to_string()) {
                                            log::warn!("Failed to save window_y on close: {}", e);
                                        }
                                    }
                                }
                                if let Ok(size) = main_clone.outer_size() {
                                    if let Err(e) = db_clone.set_setting(keys::WINDOW_WIDTH, &size.width.to_string()) {
                                        log::warn!("Failed to save window_width on close: {}", e);
                                    }
                                    if let Err(e) = db_clone.set_setting(keys::WINDOW_HEIGHT, &size.height.to_string()) {
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
                                    let _ = db.set_setting(keys::WINDOW_X, &pos.x.to_string());
                                    let _ = db.set_setting(keys::WINDOW_Y, &pos.y.to_string());
                                }
                                if let Ok(size) = main.outer_size() {
                                    if size.width > 0 && size.height > 0 {
                                        let _ = db.set_setting(keys::WINDOW_WIDTH, &size.width.to_string());
                                        let _ = db.set_setting(keys::WINDOW_HEIGHT, &size.height.to_string());
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

                if let Ok(Some(w)) = db.get_setting(keys::WINDOW_WIDTH) {
                    if let Ok(Some(h)) = db.get_setting(keys::WINDOW_HEIGHT) {
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
                if let Ok(Some(x)) = db.get_setting(keys::WINDOW_X) {
                    if let Ok(Some(y)) = db.get_setting(keys::WINDOW_Y) {
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

                let was_max = db.get_setting(keys::WINDOW_MAXIMIZED)
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
                let ticker_size = ticker.outer_size().unwrap_or_else(|_| {
                    // TICKER_* 是逻辑像素，折算物理像素需乘窗口缩放系数，
                    // 否则 DPI ≠ 100% 时兜底几何偏小
                    let scale = ticker.scale_factor().unwrap_or(1.0);
                    tauri::PhysicalSize::new(
                        (crate::datasource::TICKER_WIDTH as f64 * scale) as u32,
                        (crate::datasource::TICKER_HEIGHT as f64 * scale) as u32,
                    )
                });
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
                        if let Err(e) = db_clone.set_setting(keys::TICKER_X, &clamped_x.to_string()) {
                            log::warn!("Failed to save ticker_x: {}", e);
                        }
                        if let Err(e) = db_clone.set_setting(keys::TICKER_Y, &clamped_y.to_string()) {
                            log::warn!("Failed to save ticker_y: {}", e);
                        }
                    }
                });

                // Restore saved position, fall back to bottom-right
                let (mut saved_x, mut saved_y) = (0i32, 0i32);
                let mut has_pos = false;
                if let Ok(Some(x)) = db.get_setting(keys::TICKER_X) {
                    if let Ok(Some(y)) = db.get_setting(keys::TICKER_Y) {
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

                // Remove ticker from taskbar at both levels:
                //   set_skip_taskbar  → ITaskbarList::DeleteTab (immediate, one-shot)
                //   apply_tool_window → WS_EX_TOOLWINDOW (survives Explorer restart)
                let _ = ticker.set_skip_taskbar(true);
                apply_tool_window_style(&ticker);

                // Restore visibility from last session (config starts hidden).
                // Default to visible unless the user explicitly hid the ticker.
                let ticker_hidden = db
                    .get_setting(keys::TICKER_VISIBLE)
                    .ok()
                    .flatten()
                    .map(|v| v == "0")
                    .unwrap_or(false);
                if ticker_hidden {
                    let _ = ticker.hide();
                } else {
                    let _ = ticker.show();
                }
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
            commands::watchlist::remove_watch_from_group,
            commands::watchlist::set_watch_groups,
            commands::watchlist::move_group_member_top,
            commands::watchlist::move_group_member_up,
            commands::watchlist::move_group_member_down,
            commands::watchlist::reorder_group_members,
            commands::watchlist::add_watch_group,
            commands::watchlist::rename_watch_group,
            commands::watchlist::delete_watch_group,
            commands::watchlist::reorder_watch_groups,
            commands::watchlist::set_watch_ticker_enabled,
            commands::watchlist::set_ticker_enabled_bulk,
            commands::watchlist::reorder_ticker,
            commands::watchlist::search_stocks,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::switch_datasource,
            commands::settings::list_datasources,
            commands::settings::list_index_pool,
            commands::settings::get_portable_mode,
            commands::settings::is_store_build,
            commands::autostart::get_autostart,
            commands::autostart::set_autostart,
            commands::market::get_market_overview,
            commands::market::get_overview_interval,
            commands::window::show_main_window,
            commands::window::set_ticker_visible,
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
#[cfg(not(feature = "store"))]
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
                "sinajs.cn,sina.com.cn,gtimg.cn,gu.qq.com,qq.com,eastmoney.com,localhost,127.0.0.1",
            );
            log::info!("[proxy] {} (stock APIs excluded)", proxy);
            return;
        }
    }
    log::info!("[proxy] No local proxy detected");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// Unique temp path; no `tempfile` dependency, matching db/mod.rs.
    fn temp_path(tag: &str) -> PathBuf {
        let seq = SEQ.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "qd-logger-{}-{}-{}-{}",
            tag,
            std::process::id(),
            nanos,
            seq
        ))
    }

    #[test]
    fn writes_to_log_file_when_app_dir_is_writable() {
        let dir = temp_path("ok");
        let loggers = build_loggers(&dir);
        assert_eq!(loggers.len(), 2, "a writable app dir gets file + terminal loggers");
        assert!(
            dir.join("quant-desktop.log").exists(),
            "the log file should have been created"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 建目录或建日志文件失败时必须降级为仅 stderr，而不是 panic：store 构建
    /// 经 MSIX 文件系统虚拟化写盘，启动期 panic 是没有窗口、也没有日志可查的静默崩溃。
    #[test]
    fn falls_back_to_terminal_logger_when_app_dir_is_unwritable() {
        // A path whose parent is a regular file can never be created as a directory.
        let blocker = temp_path("blocked");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let loggers = build_loggers(&blocker.join("data"));
        assert_eq!(loggers.len(), 1, "an unusable app dir must degrade, not panic");
        std::fs::remove_file(&blocker).ok();
    }
}
