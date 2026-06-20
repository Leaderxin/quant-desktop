use tauri::{Emitter, State};
use std::sync::Arc;
use crate::db::{Database, WatchItem};
use crate::datasource::DataSourceManager;

#[tauri::command]
pub fn get_watchlist(db: State<'_, Arc<Database>>) -> Result<Vec<WatchItem>, String> {
    db.get_watchlist().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_watch(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
    name: String,
) -> Result<(), String> {
    db.add_watch(&code, &market, &name)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn remove_watch(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
) -> Result<(), String> {
    db.remove_watch(&code, &market)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn reorder_watch(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_watch(&ids).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn move_watch_top(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    db.move_watch_top(id).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn move_watch_up(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    db.move_watch_up(id).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn move_watch_down(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    db.move_watch_down(id).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn search_stocks(
    manager: State<'_, Arc<DataSourceManager>>,
    keyword: String,
) -> Result<Vec<crate::domain::StockBrief>, String> {
    // ── Tier 1: Sina suggest API (name + fuzzy code search) ──
    match crate::datasource::search::suggest_search(&keyword).await {
        Ok(results) if !results.is_empty() => {
            return Ok(results);
        }
        Ok(_) => log::info!("Sina suggest returned empty for '{}', trying Tencent", keyword),
        Err(e) => log::warn!("Sina suggest failed: {}, falling back to Tencent", e),
    }

    // ── Tier 2: Tencent smartbox API (name + fuzzy code search) ──
    match crate::datasource::search::tencent_suggest_search(&keyword).await {
        Ok(results) if !results.is_empty() => {
            return Ok(results);
        }
        Ok(_) => log::info!("Tencent smartbox returned empty for '{}', falling back to DataSource", keyword),
        Err(e) => log::warn!("Tencent smartbox failed: {}, falling back to DataSource", e),
    }

    // ── Tier 3: DataSource-based exact-code search ──
    let mut results: Vec<crate::domain::StockBrief> = Vec::new();
    let active_name = if let Some(source) = manager.active_source() {
        match source.search(&keyword, "CN").await {
            Ok(r) => results = r,
            Err(e) => log::warn!("Search via {} failed: {}", source.name(), e),
        }
        source.name().to_string()
    } else {
        String::new()
    };

    if results.is_empty() {
        for (name, source) in manager.all_sources() {
            if name != active_name {
                match source.search(&keyword, "CN").await {
                    Ok(fb_results) if !fb_results.is_empty() => {
                        results = fb_results;
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => log::warn!("Fallback search via {} failed: {}", name, e),
                }
            }
        }
    }

    Ok(results)
}
