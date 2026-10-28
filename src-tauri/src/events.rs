//! 跨窗口事件的统一发送口。

use tauri::{AppHandle, Emitter};

/// 发一个事件；失败只记一条 warn。
///
/// 这些事件（尤其 `watchlist-changed`）是行情条窗口刷新列表的唯一信号：
/// emit 失败时没有哪个窗口会报错，界面只会静静停在旧状态。调用方用
/// `let _ =` 丢弃结果的话，现场连一行日志都没有。
pub fn emit_or_warn<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        log::warn!("Failed to emit {}: {}", event, e);
    }
}
