/// 前端日志落盘。
///
/// release 包里 webview 没有 devtools，前端 `console.*` 只存在于内存 —— 用户
/// 报障时拿不到任何界面侧的证据。这条命令把前端的 error/warn 与全局未捕获
/// 异常接进同一份日志文件（`[web]` 前缀）。
#[tauri::command]
pub fn log_event(level: String, message: String) {
    crate::logging::log_from_web(&level, &message);
}
