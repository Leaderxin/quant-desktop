use crate::datasource::market_clock::MarketSession;
use crate::domain::UpdateInfo;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Check for update. Returns UpdateInfo if a newer version is available,
/// or null if the current version is already the latest.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current_version = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let updater = app
        .updater()
        .map_err(|e| format!("Updater init failed: {}", e))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?
    else {
        log::info!("[updater] No update available (current: {})", current_version);
        return Ok(None);
    };

    let latest_version = update.version.clone();
    log::info!(
        "[updater] Update found: {} -> {}",
        current_version,
        latest_version
    );

    let info = UpdateInfo {
        current_version,
        latest_version: latest_version.clone(),
        release_date: update
            .date
            .map(|d| d.to_string())
            .unwrap_or_default(),
        notes: update.body.clone().unwrap_or_default(),
        release_url: format!(
            "https://github.com/Leaderxin/QuantDesktopRelease/releases/tag/v{}",
            latest_version.strip_prefix('v').unwrap_or(&latest_version)
        ),
        download_size: None,
    };

    Ok(Some(info))
}

/// Download and install the update. Handles download, signature verification,
/// and launches the installer on completion.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();

    let updater = app
        .updater_builder()
        .on_before_exit(move || {
            log::info!("[updater] Installer launched, exiting app");
            std::process::exit(0);
        })
        .build()
        .map_err(|e| format!("Updater init failed: {}", e))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?
    else {
        return Err("No update available".into());
    };

    log::info!("[updater] Downloading update v{}...", update.version);

    update
        .download_and_install(
            |downloaded, total| {
                let total = total.unwrap_or(0);
                let percent = if total > 0 {
                    ((downloaded as f64 / total as f64) * 100.0) as u32
                } else {
                    0
                };
                let _ = handle.emit(
                    "update-download-progress",
                    serde_json::json!({
                        "downloaded": downloaded,
                        "total": total,
                        "percent": percent.min(100),
                    }),
                );
            },
            || {
                log::info!("[updater] Download complete, launching installer");
            },
        )
        .await
        .map_err(|e| format!("Download/install failed: {}", e))?;

    Ok(())
}

/// Check if currently in an active A-share trading session (9:30-11:30 or 13:00-15:00)
#[tauri::command]
pub fn is_trading_session() -> bool {
    let session = MarketSession::current();
    matches!(
        session,
        MarketSession::MorningTrade | MarketSession::AfternoonTrade
    )
}

