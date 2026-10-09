use tauri::State;
use std::sync::Arc;
use crate::db::Database;

/// 恢复并聚焦主窗口(行情条点击、托盘菜单)。
///
/// 与托盘菜单的两处入口共用 `crate::show_main_window` —— 关键是其中
/// `unminimize()` 那一步： `show()` 单独用对最小化的窗口无效，漏掉的话
/// 症状是「点了没反应」，而另一处照旧正常，改动时看不出来。
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::show_main_window(&app)
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

/// 同步窗口标题栏的主题（设置页与状态栏的主题开关）。
///
/// 与启动时那次调用共用 `crate::set_app_theme`：要做的判断完全一样，而各写一份的话
/// 漏掉的那处症状是「启动时标题栏是对的、切换主题后不跟」，正好不容易被发现。
#[tauri::command]
pub fn set_window_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    crate::set_app_theme(&app, &theme)
}
