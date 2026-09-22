use tauri::{Manager, State};
use std::sync::Arc;
use crate::db::Database;

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// 显示/隐藏行情条窗口(设置页「显示行情条」开关)。
///
/// 实现与托盘菜单的「显示/隐藏行情条」共用 `crate::set_ticker_visible` ——
/// 显示时要补的 always_on_top / skip_taskbar / WS_EX_TOOLWINDOW / 位置还原
/// 几步一旦各写一份，迟早会漏掉一处，而漏掉的症状(窗口跑进任务栏、或出现在
/// 屏幕外)在改动另一处时看不出来。
#[tauri::command]
pub fn set_ticker_visible(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    visible: bool,
) -> Result<(), String> {
    crate::set_ticker_visible(&app, &db, visible)
}
