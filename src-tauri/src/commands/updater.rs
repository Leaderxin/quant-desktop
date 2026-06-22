use crate::datasource::market_clock::MarketSession;
use crate::domain::UpdateInfo;
use std::sync::atomic::{AtomicU64, Ordering};
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

    log::info!(
        "[updater] Checking for updates (current: {})...",
        current_version
    );

    let updater = app
        .updater()
        .map_err(|e| format!("Updater init failed: {}", e))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|e| {
            log::error!("[updater] Check failed: {}", e);
            format!("Update check failed: {}", e)
        })?
    else {
        log::info!("[updater] No update available (current: {})", current_version);
        return Ok(None);
    };

    let latest_version = update.version.clone();
    let body = update.body.clone().unwrap_or_default();
    let date = update.date.map(|d| d.to_string()).unwrap_or_default();

    log::info!(
        "[updater] Update found: {} -> {} (date: {}, notes length: {})",
        current_version,
        latest_version,
        date,
        body.len()
    );

    let info = UpdateInfo {
        current_version,
        latest_version: latest_version.clone(),
        release_date: date,
        notes: body,
        release_url: format!(
            "https://github.com/Leaderxin/quant-desktop/releases/tag/v{}",
            latest_version.strip_prefix('v').unwrap_or(&latest_version)
        ),
        download_size: None,
    };

    Ok(Some(info))
}

/// Download and install the update.
///
/// **Important**: We use `app.updater()` (NOT `updater_builder()` with a custom
/// `on_before_exit`). The plugin's default `on_before_exit` only runs
/// `cleanup_before_exit()`. The installer is launched via `ShellExecuteW` AFTER
/// `on_before_exit`, and the plugin calls `std::process::exit(0)` AFTER that.
/// If we set a custom `on_before_exit` that calls `std::process::exit(0)`, we
/// kill the process BEFORE the installer is launched — the update silently fails.
///
/// **Note on progress**: The plugin's `on_chunk` callback receives the individual
/// chunk size (not cumulative bytes). We accumulate them ourselves to compute
/// actual download progress.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();
    let proxy_state = (
        std::env::var("HTTPS_PROXY").ok(),
        std::env::var("HTTP_PROXY").ok(),
    );
    log::info!(
        "[updater] install_update called, HTTPS_PROXY={:?}, HTTP_PROXY={:?}",
        proxy_state.0,
        proxy_state.1
    );

    // IMPORTANT: Use the default updater (app.updater()) — the default
    // on_before_exit only does cleanup_before_exit(), it does NOT call
    // std::process::exit(). The plugin handles the exit AFTER launching
    // the installer via ShellExecuteW.
    let updater = app
        .updater()
        .map_err(|e| {
            log::error!("[updater] Failed to init updater: {}", e);
            format!("Updater init failed: {}", e)
        })?;

    log::info!("[updater] Checking for update before download...");
    let Some(update) = updater
        .check()
        .await
        .map_err(|e| {
            log::error!("[updater] Pre-download check failed: {}", e);
            format!("Update check failed: {}", e)
        })?
    else {
        log::warn!("[updater] No update available for download");
        return Err("No update available".into());
    };

    let target_version = update.version.clone();
    let current_version = app.config().version.clone().unwrap_or_default();
    log::info!(
        "[updater] Starting download for v{} (current: {})",
        target_version,
        current_version
    );

    // The plugin's progress callback receives chunk sizes (not cumulative).
    // We accumulate them to track real progress.
    let cumulative_bytes = std::sync::Arc::new(AtomicU64::new(0));
    let cum_bytes = cumulative_bytes.clone();
    let total_size = std::sync::Arc::new(AtomicU64::new(0));
    let total_sz = total_size.clone();
    let call_count = std::sync::Arc::new(AtomicU64::new(0));
    let cc = call_count.clone();
    // Track last logged percentage milestone
    let last_logged_pct = std::sync::Arc::new(AtomicU64::new(0));
    let llp = last_logged_pct.clone();

    let result = update
        .download_and_install(
            move |chunk_size, total| {
                // chunk_size is the size of this individual chunk, NOT cumulative.
                // Accumulate to get real downloaded bytes.
                let cumulative = cum_bytes.fetch_add(chunk_size as u64, Ordering::Relaxed)
                    + chunk_size as u64;

                let count = cc.fetch_add(1, Ordering::Relaxed);

                if let Some(t) = total {
                    total_sz.store(t as u64, Ordering::Relaxed);
                }

                let total_for_pct = total.unwrap_or(1);
                let pct = if total_for_pct > 0 {
                    ((cumulative as f64 / total_for_pct as f64) * 100.0) as u64
                } else {
                    0
                };

                // Log at milestones: first call, every 10% increment, every 200th call
                let last = llp.load(Ordering::Relaxed);
                let milestone = pct / 10;
                let last_milestone = last / 10;
                let should_log = count == 0
                    || milestone > last_milestone
                    || (count > 0 && count % 200 == 0);

                if should_log {
                    llp.store(pct, Ordering::Relaxed);
                    let total_str = total
                        .map(|t| format!("{:.1} MB", t as f64 / 1_048_576.0))
                        .unwrap_or_else(|| "unknown".to_string());
                    log::info!(
                        "[updater] Download progress: {:.1} KB / {} ({}%, {} chunks)",
                        cumulative as f64 / 1024.0,
                        total_str,
                        pct,
                        count + 1
                    );
                }

                // Emit to frontend using cumulative bytes
                let percent = match total {
                    Some(t) if t > 0 => (cumulative as f64 / t as f64) * 100.0,
                    _ => 0.0,
                };
                let _ = handle.emit(
                    "update-download-progress",
                    serde_json::json!({
                        "downloaded": cumulative,
                        "total": total,
                        "percent": (percent * 10.0).round() / 10.0,
                    }),
                );
            },
            || {
                let final_bytes = cumulative_bytes.load(Ordering::Relaxed);
                let total = total_size.load(Ordering::Relaxed);
                let chunks = call_count.load(Ordering::Relaxed);
                log::info!(
                    "[updater] Download complete. Downloaded: {:.1} KB / {:.1} MB ({} chunks)",
                    final_bytes as f64 / 1024.0,
                    total as f64 / 1_048_576.0,
                    chunks
                );
                if final_bytes > 0 && total > 0 && final_bytes < total * 9 / 10 {
                    log::warn!(
                        "[updater] WARNING: only {:.1}% of the file was downloaded!",
                        (final_bytes as f64 / total as f64) * 100.0
                    );
                }
            },
        )
        .await
        .map_err(|e| {
            log::error!("[updater] Download/install failed: {}", e);
            format!("Download/install failed: {}", e)
        });

    match &result {
        Ok(()) => {
            // Note: we won't actually reach here for a successful install because
            // the plugin calls std::process::exit(0) after launching the installer.
            log::info!("[updater] download_and_install returned Ok (installer launched)");
        }
        Err(e) => {
            log::error!("[updater] install_update error: {}", e);
        }
    }

    result
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
