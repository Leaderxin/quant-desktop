//! OS-level autostart management, exposed as two IPC commands.
//!
//! - Store builds (Windows): the packaged-app StartupTask WinRT API. The task
//!   is declared in store/AppxManifest.xml (`uap5:StartupTask`), so Windows
//!   resolves the launch path by package identity — it survives Store updates
//!   (unlike a registry Run key storing the versioned WindowsApps path) and
//!   appears in Task Manager → Startup for the user to manage.
//! - All other builds: tauri-plugin-autostart's registry Run-key mechanism.

use tauri::AppHandle;

/// Whether OS-level autostart is currently enabled.
#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    #[cfg(all(feature = "store", target_os = "windows"))]
    {
        let _ = &app;
        startup_task::is_enabled()
    }
    #[cfg(not(all(feature = "store", target_os = "windows")))]
    {
        use tauri_plugin_autostart::ManagerExt;
        match app.autolaunch().is_enabled() {
            Ok(enabled) => enabled,
            Err(e) => {
                log::warn!("[autostart] is_enabled failed: {}", e);
                false
            }
        }
    }
}

/// Enable or disable OS-level autostart.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let result: Result<(), String> = {
        #[cfg(all(feature = "store", target_os = "windows"))]
        {
            let _ = &app;
            startup_task::set_enabled(enabled)
        }
        #[cfg(not(all(feature = "store", target_os = "windows")))]
        {
            use tauri_plugin_autostart::ManagerExt;
            let manager = app.autolaunch();
            if enabled {
                manager.enable().map_err(|e| e.to_string())
            } else {
                manager.disable().map_err(|e| e.to_string())
            }
        }
    };

    match &result {
        Ok(()) => log::info!(
            "[autostart] {}",
            if enabled { "enabled" } else { "disabled" }
        ),
        Err(e) => log::warn!("[autostart] failed to set enabled={}: {}", enabled, e),
    }
    result
}

/// MSIX packaged-app startup task (Windows store builds only).
///
/// Requires package identity: running a store build unpackaged (e.g. plain
/// `cargo run --features store`) makes the WinRT calls fail, which surfaces as
/// "disabled" / an error message instead of a silent no-op.
#[cfg(all(feature = "store", target_os = "windows"))]
mod startup_task {
    use windows::ApplicationModel::{StartupTask, StartupTaskState};
    use windows::core::HSTRING;

    /// Must match the TaskId in store/AppxManifest.xml.
    const TASK_ID: &str = "QuantDesktop";

    fn get_task() -> windows::core::Result<StartupTask> {
        // IAsyncOperation::get() blocks on an event handle — safe from any
        // thread, including the IPC thread these commands run on.
        StartupTask::GetAsync(&HSTRING::from(TASK_ID))?.get()
    }

    pub fn is_enabled() -> bool {
        match get_task().and_then(|task| task.State()) {
            Ok(StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy) => true,
            Ok(_) => false,
            Err(e) => {
                log::warn!("[autostart] StartupTask state query failed: {}", e);
                false
            }
        }
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        let task = get_task()
            .map_err(|e| format!("StartupTask 初始化失败（商店版需通过 MSIX 安装运行）: {}", e))?;
        if enabled {
            let state = task
                .RequestEnableAsync()
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
            match state {
                StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
                StartupTaskState::DisabledByUser => Err(
                    "开机自启已被系统禁用，请在 任务管理器 → 启动 应用 中重新允许".to_string(),
                ),
                StartupTaskState::DisabledByPolicy => {
                    Err("开机自启已被组织策略禁用".to_string())
                }
                _ => Err(format!("开机自启启用失败（状态: {:?}）", state)),
            }
        } else {
            task.Disable().map_err(|e| e.to_string())
        }
    }
}
