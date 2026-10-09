//! 全局快捷键。
//!
//! 目前只有一个：老板键 —— 按下即隐藏主窗口。
//!
//! 注册放在 Rust 侧而不是用配套的 JS 插件：老板键要在主窗口已经隐藏、整个
//! 应用没有任何焦点的时候仍然响应，而那正是 webview 里的 JS 拿不到执行时机
//! 的情形。

use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::db::{keys, Database};

/// 按设置里的值重新注册老板键（空串 = 不设）。
///
/// **先全部注销、再注册**：改键时若只加不减，旧键会继续响应，而设置页上已经
/// 看不到它了 —— 用户既关不掉也查不到那个全局热键。`unregister_all` 对空表
/// 是安全的，所以「不设」这条路径也走同一套流程。
pub fn apply(app: &AppHandle, accelerator: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts.unregister_all().map_err(|e| e.to_string())?;

    let accelerator = accelerator.trim();
    if accelerator.is_empty() {
        return Ok(());
    }

    // Shortcut 的字符串语法是 '+'-分隔的修饰键 + 主键（见 global-hotkey 的
    // parse_hotkey）："Ctrl+Shift+H"、"Alt+F4"、"F5" 都认，主键必须写在最后。
    // 具体的词表在 src/utils/hotkey.ts 里有一份，录制端与解析端必须对齐。
    let shortcut = Shortcut::try_from(accelerator)
        .map_err(|_| format!("无法识别的快捷键：{}", accelerator))?;
    shortcuts
        .register(shortcut)
        .map_err(|e| format!("注册快捷键失败（可能已被其他程序占用）：{}", e))
}

/// 启动时按库里存的值注册一次。
pub fn register_from_db(app: &AppHandle) {
    let accelerator = match app.state::<Arc<Database>>().get_setting(keys::BOSS_KEY) {
        Ok(Some(v)) => v,
        Ok(None) => return,
        Err(e) => {
            log::warn!("Failed to read {}: {}", keys::BOSS_KEY, e);
            return;
        }
    };
    if accelerator.trim().is_empty() {
        return;
    }
    match apply(app, &accelerator) {
        Ok(()) => log::info!("[hotkey] Boss key registered: {}", accelerator),
        // 不致命：热键注册失败（组合键被别的程序占着）不该拦住启动。用户在设置
        // 页里改一次就会重新走 apply，届时会把失败原因回显给他。
        Err(e) => log::warn!("[hotkey] Failed to register boss key '{}': {}", accelerator, e),
    }
}

/// 老板键的动作：藏起主窗口，不碰行情条。
///
/// 只藏不切 —— 恢复走托盘图标或点一下行情条。做成来回切的键，在「人真的站在
/// 身后」的那一刻就有被按亮的风险，而那一秒正是这个键唯一存在的理由。
/// 行情条也不动：这是一次临时躲避，不是把配置改掉（`ticker_visible` 不参与）。
pub fn hide_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(e) = window.hide() {
        log::warn!("[hotkey] Failed to hide main window: {}", e);
    }
}

/// 构造插件。handler 收所有已注册的热键，而我们只注册老板键这一个。
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            // 只认按下：按下和释放各触发一次的话，按住不放会重复执行。
            if event.state == ShortcutState::Pressed {
                hide_main_window(app);
            }
        })
        .build()
}
