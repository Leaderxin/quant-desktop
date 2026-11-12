pub mod domain;
pub mod db;
pub mod datasource;
pub mod cache;
pub mod commands;
pub mod hotkey;
pub mod import;
pub mod events;
pub mod logging;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use simplelog::{CombinedLogger, WriteLogger, TermLogger, TerminalMode, ColorChoice};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
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

/// 直接向 xdg-desktop-portal 读系统配色方案（Linux 专用）。
///
/// 为什么不复用 `window.theme()`：窗口被 pin 过之后它返回的是 pin 值；而解 pin
/// （`set_theme(None)`）在 Linux 上会把进程级 GTK 偏好强制改成浅色，WebKitGTK
/// 会先按浅色重绘一帧 —— 这就是切换「跟随系统」时闪白的根因（issue #37）。
/// 所以必须在碰 GTK 之前把系统值读出来，之后只做一次 `set_theme(Some(..))`。
///
/// 与 tao 的 `platform_impl/linux/portal.rs` 是同一条 D-Bus 调用。dbus 已经在
/// 依赖树里（tao 的 `dbus` feature，tauri 默认开启），这里只是直接依赖它。
///
/// 返回 `None` 表示 portal 不可用（无 xdg-desktop-portal / dbus 连不上），
/// 调用方退回 tao 的旧路径。
#[cfg(target_os = "linux")]
fn linux_portal_theme() -> Option<tauri::Theme> {
    use dbus::arg::Variant;
    use dbus::blocking::Connection;
    use std::time::Duration;

    let connection = Connection::new_session().ok()?;
    let proxy = connection.with_proxy(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        Duration::from_secs(2),
    );
    let result: (Variant<Variant<u32>>,) = proxy
        .method_call(
            "org.freedesktop.portal.Settings",
            "Read",
            ("org.freedesktop.appearance", "color-scheme"),
        )
        .ok()?;

    Some(match result.0 .0 .0 {
        1 => tauri::Theme::Dark,
        2 => tauri::Theme::Light,
        _ => tauri::Theme::Light, // 0 = 无偏好
    })
}

/// 把主题同步到窗口的标题栏。
///
/// `<html data-theme>` 只管网页内容 —— **标题栏是操作系统画的**，两者互不相干，
/// 所以不显式告诉窗口的话，暗色主题下标题栏会照旧是白的。tao 收到 theme 后会调
/// `DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)`（见 tao 的
/// platform_impl/windows/dark_mode.rs），Windows 10 1809+ 生效；macOS 走 NSWindow
/// 的 appearance；其余平台是空操作。所以这一个调用就是「标题栏跟随主题」的全部。
///
/// 传进来的 `theme` 是**模式**（"dark" / "light" / "system"），不是解析好的主题：
/// "system" 要传 `None` 给 tao，表示「这个窗口不表态、由系统定」。这一条是
/// 「跟随系统」整个功能的支点，不能简化成传解析后的值：
///
/// - tao 一旦被显式设过主题（`preferred_theme.is_some()`），就会**忽略系统主题的
///   变化**（见 tao 的 event_loop::update_theme，来自系统设置变更时直接 return），
///   tauri-runtime-wry 也就不会再往 WebView 转发，WebView2 的
///   `PreferredColorScheme` 于是停在应用自己的主题上；
/// - 而 `prefers-color-scheme` 正是由它决定的，于是前端读 `matchMedia` 读到的是
///   **应用自己的主题**而不是系统的，系统切换时那个媒体查询也不动。
///
/// 合起来就是：显式主题会把 webview 的深浅判定一起钉死。所以「跟随系统」必须真的
/// 把窗口交还给系统（`None`），前端那条 `matchMedia` 订阅才有意义。
///
/// ⚠ **Linux/GTK 是例外**：tao 把 `set_theme(None)` 实现成
/// `set_gtk_application_prefer_dark_theme(false)`（见 tao 的
/// platform_impl/linux/event_loop.rs），也就是**强制浅色**，而不是解 pin 交还给
/// 系统；而 WebKitGTK 的 `prefers-color-scheme` 读的正是这个 GTK 设置。更糟的是
/// 它是**进程级**的：建窗时 tao 从 xdg-desktop-portal 读到的系统偏好会被这里一句
/// `None` 覆盖掉，症状就是深色桌面上应用永远浅色（issue #35）。所以 Linux 的
/// system 模式必须**绕开 `set_theme(None)`**：先用 [`linux_portal_theme`] 直接读
/// portal 的系统值，再 `set_theme(Some(..))` 设回去。先解 pin 再读的话，GTK 会被
/// 强制改成浅色，WebKitGTK 先按浅色重绘一帧，深色桌面上就是切换「跟随系统」时
/// 闪一下白（issue #37）。
///
/// 只作用于 main：ticker 是 `decorations: false`，根本没有标题栏可画。Windows 上
/// 窗口主题是每窗口独立的，ticker 的 matchMedia 永远是系统真值；Linux 上 GTK 设置
/// 是进程级的、会跟着一起被 pin，但 ticker 的主题由自己的 `themeMode` 解析，
/// 不依赖那里的系统值。
///
/// 启动时必须在 `main.show()` **之前**调。窗口是先 show 出来、webview 才加载完的，
/// 若等前端跑到这里才变色，暗色主题下会先闪一帧白色标题栏。设置页的开关走
/// `commands::window::set_window_theme`，两边共用这一份实现。
pub fn set_app_theme(app: &tauri::AppHandle, theme: &str) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    // 只有 "dark"/"light" 是显式的；"system" 与缺键、手改坏的值一律交给系统 ——
    // 与 DEFAULT_SETTINGS 里 theme 的默认值、以及前端 parseThemeMode 的兜底一致。
    // 三处分家的话症状是「库里写的是跟随系统，界面上却按亮色渲染」。
    let theme = match theme {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None,
    };

    #[cfg(target_os = "linux")]
    {
        use std::sync::atomic::Ordering;

        if theme.is_none() {
            // 见函数头注释：tao 的 None 在 Linux 上是「强制浅色」，不是「跟随系统」。
            // 没 pin 过就什么都不用做：建窗时 tao 已经按 portal 把 GTK 偏好设好了，
            // 这里再 set_theme(None) 只会把它改回浅色（issue #35）。
            if !THEME_PINNED.swap(false, Ordering::Relaxed) {
                return Ok(());
            }

            // 先直接读 portal，再一次性设回去。**不要先 set_theme(None)**：
            // 那会把进程级 GTK 偏好强制改成浅色，WebKitGTK 先按浅色重绘一帧，
            // 再被下面这句设回深色 —— 深色桌面上就是切换「跟随系统」时闪一下白
            // （issue #37）。这条路径不经过 None，`preferred_theme` 会停在系统值
            // 上；它只影响 `window.theme()` 的返回值，而系统主题变化由 tao 的
            // portal 监听直接改 GTK，不看 `preferred_theme`，所以没有副作用。
            if let Some(system) = linux_portal_theme() {
                log::debug!("Linux: system theme resolved to {system:?}");
                if let Err(e) = window.set_theme(Some(system)) {
                    THEME_PINNED.store(true, Ordering::Relaxed);
                    return Err(e.to_string());
                }
                return Ok(());
            }

            // portal 不可用（无 xdg-desktop-portal / dbus 连不上）：退回 tao 的
            // 旧路径（解 pin → 读 portal → 设回去）。这条路径会经过「强制浅色」，
            // 可能闪一下，但没有别的信息来源。
            log::debug!("Linux: portal theme unavailable, falling back to tao");
            let result = window
                .set_theme(None)
                .and_then(|_| window.theme())
                .and_then(|system| {
                    log::debug!("Linux: system theme resolved to {system:?} (tao fallback)");
                    window.set_theme(Some(system))
                });
            if let Err(e) = result {
                // 恢复标志让下次切到 system 重试：否则 GTK 可能停在强制浅色上，
                // 而这里以为已经交还给系统了。
                THEME_PINNED.store(true, Ordering::Relaxed);
                return Err(e.to_string());
            }
            return Ok(());
        }

        // 显式主题：记下本进程 pin 过，切回 system 时才知道要去问 portal。
        THEME_PINNED.store(true, Ordering::Relaxed);
    }

    window.set_theme(theme).map_err(|e| e.to_string())
}

/// Linux 上本进程上一次是否把 GTK 主题 pin 成了显式值。
///
/// 启动时没 pin 过就什么都不用做：tao 建窗时已经按 portal 把 GTK 偏好设好了，
/// 这里再动它反而会出问题（旧实现里那句 `set_theme(None)` 会强制浅色，见 #35）。
/// 切成显式主题时置位，切回 system 时据此决定要不要去问 portal。只在 Linux 编译。
#[cfg(target_os = "linux")]
static THEME_PINNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Runtime flag indicating whether the app is in portable mode
/// (triggered by the presence of `portable.dat` next to the executable).
#[derive(Debug, Clone, Copy)]
pub struct PortableMode(pub bool);

/// 把主窗口显示出来并聚焦 —— 托盘(菜单项与左键)和行情条的
/// `show_main_window` 命令共用这一份实现。
///
/// **`show()` 不会退出最小化**：Windows 上它走 `SW_SHOW`，对最小化的窗口
/// 只是「在当前大小和位置显示」，窗口照旧缩在任务栏里，后面的 `set_focus()`
/// 于是聚焦了一个最小化的窗口 —— 用户看到的是任务栏图标闪一下，界面不出来。
/// 恢复最小化要显式 `unminimize()`(走 `SW_RESTORE`)。行情条点一下打不开主
/// 窗口、托盘「显示主界面」没反应，都是漏了这一步的症状，所以这三处必须共用。
///
/// 顺序也有依赖：先 `show()`(窗口可能是隐藏的)再 `unminimize()`，两者都经
/// tao 的 `execute_in_thread` 排到窗口线程，FIFO 保证了这个顺序。
pub fn show_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    // is_minimized() 读的是真实的 IsIconic(hwnd) 而非缓存的标志位，
    // 所以紧跟在异步的 show() 后面判断也不会读到过期状态。
    if window.is_minimized().map_err(|e| e.to_string())? {
        window.unminimize().map_err(|e| e.to_string())?;
    }
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// 设行情条窗口尺寸，并把尺寸钉成「不可由用户/WM 改」。
///
/// 三步的顺序都是必需的，两两之间各有一个坑：
///
/// 1. **先清 min/max 提示**：Linux/GTK 下非 resizable 窗口的尺寸提示会被
///    `gtk_window_compute_hints()` 钉成同一个值（MAX_SIZE = MIN_SIZE），且那是
///    **建窗时**的值 —— 不清掉，改高度就会被旧的 max 夹住：窗口只能撑大、收不回
///    内容高度，行情条底部永久多出一截。
/// 2. **改完再钉回同一个值**：这才是「锁尺寸」的落实处。不能只靠
///    `set_resizable(false)` —— GNOME/Mutter 实测仍能给这种无边框窗口提供缩放
///    手势；而此前放开 `set_resizable(true)` 更是直接把拖拽改大小打开了。
///    WM 看到 min == max 就不给缩放手柄、边缘拖拽也无效。
/// 3. 三步都在同一个调用里做完，用户手势插不进「已清提示、还没钉回去」的中间态。
///
/// 行情条所有改尺寸的路径都必须走这里：启动、显示（`set_ticker_visible`）、以及
/// 前端按内容量出的高度（`commands::window::set_ticker_size`）。非 Linux 上钉
/// min/max 同样是禁止缩放的标准做法，三个平台的行为因此一致。
pub(crate) fn set_ticker_size(window: &tauri::WebviewWindow, width: u32, height: u32) {
    let size: tauri::Size = tauri::LogicalSize::new(width as f64, height as f64).into();
    let _ = window.set_min_size(None::<tauri::Size>);
    let _ = window.set_max_size(None::<tauri::Size>);
    let _ = window.set_size(size);
    let _ = window.set_min_size(Some(size));
    let _ = window.set_max_size(Some(size));
}

/// 行情条窗口的持久化高度（逻辑像素）。
///
/// 行情条高度由前端按内容测量（见 `useTickerWindowHeight`），配置里的
/// `TICKER_HEIGHT` 只是首启兜底。启动/显示时必须**先**按这里的高度设尺寸，
/// 再恢复 `ticker_y`：若先以 40px 建窗、等前端量完再长高，前端把「40 → 内容
/// 高度」的差当成一次位移来锚定下边缘，而保存的 y 来自上一次会话的最终高度，
/// 结果就是每次重启都向上漂移同样的距离。
fn saved_ticker_height(db: &Database) -> u32 {
    db.get_setting(keys::TICKER_HEIGHT)
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u32>().ok())
        .map(|h| h.clamp(24, 600))
        .unwrap_or(crate::datasource::TICKER_HEIGHT)
}

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

    // 先按上次内容高度设尺寸，再 show / 恢复位置：位置是按「最终高度」存下的，
    // 用配置里的 40px 建窗会让前端的长高补偿把窗口一路往上推（见
    // saved_ticker_height 的注释）。
    let height = saved_ticker_height(db);
    set_ticker_size(&window, crate::datasource::TICKER_WIDTH, height);

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
    // 按即将设置的高度算物理尺寸：set_size 是投递到事件循环的请求，紧接着
    // outer_size() 读到的可能还是旧值，位置校验就会用错高度。
    let scale = window.scale_factor().unwrap_or(1.0);
    let tw = (crate::datasource::TICKER_WIDTH as f64 * scale).round() as i32;
    let th = (height as f64 * scale).round() as i32;

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
///
/// 文件用追加模式、超过 2MB 先轮转，时间戳用本机时区 —— 三者都是「事后能查」
/// 的前提，具体见 `logging` 模块。
fn build_loggers(app_dir: &std::path::Path) -> Vec<Box<dyn simplelog::SharedLogger>> {
    let level = logging::requested_level();
    let config = logging::config();
    let mut loggers: Vec<Box<dyn simplelog::SharedLogger>> = vec![TermLogger::new(
        level,
        config.clone(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    match logging::open_log_file(app_dir) {
        Ok(log_file) => {
            loggers.push(WriteLogger::new(level, config, log_file));
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
        // 原生「另存为」对话框：自选导入模板让用户自己挑目录。
        .plugin(tauri_plugin_dialog::init())
        // Registry Run-key autostart (macOS: LaunchAgent). Store builds on
        // Windows don't use this — their autostart goes through the packaged
        // StartupTask API instead (see commands/autostart.rs); the plugin stays
        // registered for the remaining build configurations.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        // 全局快捷键(老板键)。注册的值在下面的 setup 里从库里读 —— 插件本身
        // 只是把事件循环和 handler 支起来，注册/注销都由 hotkey::apply 负责。
        .plugin(hotkey::plugin());

    // Store builds skip the built-in updater — the Microsoft Store distributes
    // updates itself, so the updater plugin is not registered at all.
    #[cfg(not(feature = "store"))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .setup(|app| {
            // 先装 panic hook：setup 里任何 `expect` 都要在日志文件里留下证据。
            // release 包是 GUI 进程，stderr 不可见，否则崩溃看起来就是「双击没反应」。
            logging::install_panic_hook();

            // Data directory:
            // - Portable mode (portable.dat exists next to exe) → <exe_dir>/data/
            // - Normal mode → %APPDATA%/quant-desktop/
            //
            // 取不到系统数据目录时退回临时目录而不是 panic：日志得有地方落，
            // 应用也还能起来；真正的警告在 logger 就绪后补记。
            let mut data_dir_fallback = false;
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
                        .unwrap_or_else(|| {
                            data_dir_fallback = true;
                            std::env::temp_dir()
                        })
                        .join("quant-desktop");
                    (dir, false)
                });

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
            log::info!(
                "Log level: {:?} (override with {})",
                logging::requested_level(),
                logging::LEVEL_ENV
            );
            if data_dir_fallback {
                log::warn!(
                    "System data directory unavailable — using {:?}",
                    app_dir
                );
            }

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
            log::info!("Quote cache initialized");

            // Manage state
            app.manage(db.clone());
            app.manage(ds_manager.clone());
            app.manage(cache.clone());
            app.manage(Arc::new(
                crate::datasource::market::MarketOverviewClient::new(),
            ));
            app.manage(PortableMode(is_portable));

            // 启动顺序：托盘 → 主窗口 show → 代理探测 → 调度器 → 行情条 → 老板键。
            // 代理探测（本机实测 ~265ms）与调度器启动都不影响首屏，故意放在
            // main.show() 之后；但两者之间顺序有依赖 —— `detect_and_set_proxy`
            // 必须在 `Scheduler::spawn` 之前完成，否则调度器的首个请求可能读到
            // 尚未设置的代理环境变量。

            // ── System Tray ──
            let show_item = MenuItemBuilder::with_id("show", "显示主界面").build(app)?;
            let settings_item = MenuItemBuilder::with_id("settings", "设置").build(app)?;
            let toggle_ticker = MenuItemBuilder::with_id("toggle_ticker", "显示/隐藏行情条").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

            // 「设置」放在分隔线之下、「检查更新」之后：线上面是两条窗口操作，
            // 线下面都是应用级操作（检查更新 / 设置 / 退出）—— 分组按这个分，
            // 而不是按菜单项的创建顺序。
            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&toggle_ticker)
                .separator();

            // Portable mode and Store builds both skip the "check update" tray
            // item: portable updates are user-managed (download & replace the
            // zip), and Store updates are handled by the Microsoft Store itself.
            #[cfg(not(feature = "store"))]
            let menu = if is_portable {
                menu
            } else {
                let check_update_item =
                    MenuItemBuilder::with_id("check_update", "检查更新").build(app)?;
                menu.item(&check_update_item)
            };

            let menu = menu.item(&settings_item).item(&quit_item).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .expect("Default window icon not embedded — check tauri.conf.json icons config"),
                )
                .tooltip("QuantDesktop")
                .menu(&menu)
                // 左键只切换主窗口，菜单留给右键。tray-icon 在 Windows 上的默认值是
                // `menu_on_left_click = true`：每次 WM_LBUTTONUP 之后都会先派发
                // click 事件、再 TrackPopupMenu。于是关窗口那一次会「窗口隐藏了、
                // 菜单留在屏幕上」；显示那次看起来正常，只是因为 show_main_window
                // 把窗口拉到前台顺带把菜单顶掉了。Linux(gtk) 忽略这个开关，不受影响。
                .show_menu_on_left_click(false)
                .on_menu_event({
                    let db = db.clone();
                    move |app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Err(e) = show_main_window(app) {
                                log::warn!("Failed to show main window: {}", e);
                            }
                        }
                        "settings" => {
                            // 先把窗口恢复出来再发事件：设置页要有个窗口承载，
                            // 而窗口若是最小化的，事件到了也看不见。
                            if let Err(e) = show_main_window(app) {
                                log::warn!("Failed to show main window for settings: {}", e);
                            }
                            // 主窗口的 webview 是常驻的(隐藏不等于卸载)，AppLayout
                            // 挂载时就注册了监听，所以隐藏状态下这次 emit 也收得到。
                            events::emit_or_warn(app, "open-settings", ());
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
                                        events::emit_or_warn(&handle, "update-available", &info);
                                    }
                                    Ok(None) => {
                                        log::info!("[updater] Manual check: already up to date");
                                        events::emit_or_warn(
                                            &handle,
                                            "update-check-complete",
                                            "up-to-date",
                                        );
                                    }
                                    Err(e) => {
                                        log::warn!("[updater] Manual check failed: {}", e);
                                        events::emit_or_warn(
                                            &handle,
                                            "update-check-complete",
                                            "error",
                                        );
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
                            // 最小化的窗口在 Win32 眼里仍然是 visible(WS_VISIBLE
                            // 还在)，只判 is_visible() 会落进「隐藏」分支 —— 用户
                            // 点托盘本想把窗口叫回来，窗口反倒不见了。
                            let minimized = window.is_minimized().unwrap_or(false);
                            if window.is_visible().unwrap_or(false) && !minimized {
                                let _ = window.hide();
                            } else if let Err(e) = show_main_window(app) {
                                log::warn!("Failed to show main window: {}", e);
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

                // 标题栏要先于 show 上色：窗口是先 show 出来、webview 才加载完的，
                // 等前端跑起来再变色会先闪一帧白条（见 set_app_theme）。
                // 读不到键时按「跟随系统」——与 DEFAULT_SETTINGS 的默认值一致，
                // 也是 set_app_theme 对未知值的归类。
                let theme = db
                    .get_setting(keys::THEME)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "system".to_string());
                if let Err(e) = set_app_theme(app.handle(), &theme) {
                    log::warn!("Failed to apply window theme at startup: {e}");
                }

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

            // Detect local proxy (Clash/V2Ray) for updater downloads.
            // Store builds have no updater, so this is compiled out.
            // 放在 logger 之后：探测结果本身就要记日志，之前它在日志里永远是空的。
            #[cfg(not(feature = "store"))]
            detect_and_set_proxy();

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

            // Ticker window: save position on move (clamped), restore on startup
            if let Some(ticker) = app.get_webview_window("ticker") {
                let _ = ticker.set_always_on_top(true);

                // Linux/GTK 曾经在这里 `set_resizable(true)`：非 resizable 窗口的
                // min/max 提示会被 GTK 钉成建窗时的同一个值
                // (gtk_window_compute_hints() 里 MAX_SIZE = MIN_SIZE)，之后
                // set_size 只能撑大、收不回去，行情条底部永久多出一截。
                //
                // 现在不需要了：改尺寸一律走 `set_ticker_size`，它「先清提示 →
                // 改尺寸 → 把新尺寸钉回 min/max」。清掉那一步就是上面那个坑的解法，
                // 而钉回去那一步同时把「用户/WM 能拖大拖小」彻底关掉 —— resizable
                // 留着 true 的话，GNOME/Mutter 仍会给这种无边框窗口提供缩放手势
                // （用户实测能拖动大小），锁尺寸必须靠这里两个条件一起成立。
                //
                // 顺序依赖：这一步之后才是首次 set_ticker_size（见下）。

                // Windows：无边框窗口开着原生阴影时，DWM 会在窗口最顶上留 1px
                // 白边（tao 在 Win10 上把 top inset 设成 0，白边正好压在客户区
                // 第一行后面）。底板在 body 上时它被不透明画布盖住，看不出；
                // 现在圆角处的画布是透明的，左上/右上角就会露出这条白边。
                // 行情条的圆角由页面自己画，不需要原生阴影，直接关掉。
                #[cfg(target_os = "windows")]
                {
                    let _ = ticker.set_shadow(false);
                    // 关掉原生阴影后，WM_NCCALCSIZE 不再扣掉 shadow insets，客户区
                    // 会比设定值多出左右/下的一圈。尺寸在下面统一设，必须在
                    // set_shadow(false) 之后，否则会被这圈 inset 覆盖。
                }

                // 先按上次的内容高度建窗，再恢复位置（见 saved_ticker_height）。
                // 顺序有依赖：Windows 上必须在 set_shadow(false) 之后（否则
                // outer 尺寸会多出一圈 inset）。尺寸与 min/max 提示一起在
                // `set_ticker_size` 里落地 —— GTK 那个「旧 max 夹住新高度」的坑
                // 由它「先清提示」解决，所以这里不再需要放开 resizable。
                let initial_ticker_height = saved_ticker_height(&db);
                set_ticker_size(
                    &ticker,
                    crate::datasource::TICKER_WIDTH,
                    initial_ticker_height,
                );

                // 钳制与「可见比例」都按事件发生时的真实尺寸算：行情条高度会随
                // 内容（图表开关、轮播条数）变化，用启动时捕获的高度会把底部
                // 越界的位置当成合法值存下来。
                fn ticker_physical_size(window: &tauri::WebviewWindow) -> (i32, i32) {
                    let size = window.outer_size().unwrap_or_else(|_| {
                        // TICKER_* 是逻辑像素，折算物理像素需乘窗口缩放系数，
                        // 否则 DPI ≠ 100% 时兜底几何偏小
                        let scale = window.scale_factor().unwrap_or(1.0);
                        tauri::PhysicalSize::new(
                            (crate::datasource::TICKER_WIDTH as f64 * scale) as u32,
                            (crate::datasource::TICKER_HEIGHT as f64 * scale) as u32,
                        )
                    });
                    (size.width as i32, size.height as i32)
                }

                // Capture monitor bounds and ticker size for clamping on move.
                // 这里按「即将设置的高度」算，而不是 outer_size()：set_size 是
                // 投递到事件循环的请求，紧接着读到的可能还是旧的 40px。
                let mon = ticker.primary_monitor().ok().flatten();
                let (mon_w, mon_h) = mon
                    .as_ref()
                    .map(|m| { let s = m.size(); (s.width as i32, s.height as i32) })
                    .unwrap_or((1920, 1080));
                let scale = ticker.scale_factor().unwrap_or(1.0);
                let tw = (crate::datasource::TICKER_WIDTH as f64 * scale).round() as i32;
                let th = (initial_ticker_height as f64 * scale).round() as i32;

                // Save ticker position on move.  Only persist if enough of the
                // ticker is actually visible — if the user drags it way off
                // screen, we skip saving so the next launch falls back to the
                // default bottom-right position.
                let ticker_for_move = ticker.clone();
                let db_clone = db.clone();
                let _ = ticker.on_window_event(move |event| {
                    if let tauri::WindowEvent::Moved(pos) = event {
                        let (tw, th) = ticker_physical_size(&ticker_for_move);
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

            // 老板键：库里存了值才注册。放在窗口都就绪之后，让「按下去要藏的
            // 那个窗口」确定存在。
            hotkey::register_from_db(app.handle());

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
            commands::import::import_watchlist,
            commands::import::save_import_template,
            commands::import::read_spreadsheet,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::set_boss_key,
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
            commands::window::set_ticker_size,
            commands::window::set_window_theme,
            commands::updater::check_update,
            commands::updater::install_update,
            commands::updater::is_trading_session,
            commands::log::log_event,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start application");
}

/// Auto-detect local proxy (Clash/V2Ray) and set env vars for updater downloads.
///
/// # Safety
///
/// `std::env::set_var` is NOT thread-safe per Rust's documentation. Call this only
/// from `setup` on the main thread, and keep it BEFORE `Scheduler::spawn` and before
/// the updater's first request (which builds its own proxy-aware client).
///
/// 共享的行情 HTTP client 用 `.no_proxy()` 构建（见 `datasource::shared_client`），
/// 不读这些环境变量，所以把它挪到 `main.show()` 之后是安全的。不要再让行情适配器
/// 在构造时初始化共享 client —— 那会把「代理设置」与「client 构造顺序」重新绑定。
#[cfg(not(feature = "store"))]
fn detect_and_set_proxy() {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    // Skip if proxy already set
    if std::env::var("HTTPS_PROXY").is_ok()
        || std::env::var("HTTP_PROXY").is_ok()
        || std::env::var("https_proxy").is_ok()
        || std::env::var("http_proxy").is_ok()
    {
        return;
    }

    // 端口按优先级排列。**并行**探测：逐个 connect_timeout 在「端口被防火墙静默
    // 丢弃」的机器上每个都要等满超时 —— 实测 Win10 无代理时 7 个端口串起来是 8 秒。
    // 本地回环上有代理时连接是毫秒级的，150ms 足够区分。
    const PORTS: [u16; 7] = [7890, 10809, 10808, 7891, 1080, 8118, 8080];
    let detected = std::thread::scope(|scope| {
        let handles: Vec<_> = PORTS
            .iter()
            .map(|&port| {
                scope.spawn(move || {
                    let addr = SocketAddr::from(([127, 0, 0, 1], port));
                    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
                })
            })
            .collect();
        handles
            .into_iter()
            .zip(PORTS)
            .find_map(|(handle, port)| handle.join().ok().filter(|ok| *ok).map(|_| port))
    });

    if let Some(port) = detected {
        let proxy = format!("http://127.0.0.1:{}", port);
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

    /// 行情条启动高度：没存过时用配置兜底，坏值/越界值必须夹到可用范围。
    /// 这里读错会让窗口以错误高度建出来，前端的长高补偿把行情条往上推。
    #[test]
    fn saved_ticker_height_defaults_and_clamps() {
        let dir = temp_path("ticker-height");
        let db = Database::open(dir.clone()).unwrap();

        assert_eq!(
            saved_ticker_height(&db),
            crate::datasource::TICKER_HEIGHT,
            "首次启动没有持久化高度时用配置兜底"
        );

        db.set_setting(keys::TICKER_HEIGHT, "5").unwrap();
        assert_eq!(saved_ticker_height(&db), 24, "过小夹到下限");

        db.set_setting(keys::TICKER_HEIGHT, "9999").unwrap();
        assert_eq!(saved_ticker_height(&db), 600, "过大夹到上限");

        db.set_setting(keys::TICKER_HEIGHT, "not-a-number").unwrap();
        assert_eq!(
            saved_ticker_height(&db),
            crate::datasource::TICKER_HEIGHT,
            "坏值退回配置兜底"
        );

        db.set_setting(keys::TICKER_HEIGHT, "99").unwrap();
        assert_eq!(saved_ticker_height(&db), 99);

        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }
}
