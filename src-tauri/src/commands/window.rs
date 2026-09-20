use tauri::Manager;

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    if window.is_minimized().map_err(|e| format!("检查最小化状态失败: {e}"))? {
        window.unminimize().map_err(|e| format!("恢复最小化窗口失败: {e}"))?;
    }
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
