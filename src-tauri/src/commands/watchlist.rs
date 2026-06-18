use tauri::State;
use std::sync::Arc;
use crate::db::{Database, WatchItem};
use crate::datasource::DataSourceManager;

#[tauri::command]
pub fn get_watchlist(db: State<'_, Arc<Database>>) -> Result<Vec<WatchItem>, String> {
    db.get_watchlist().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_watch(
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
    name: String,
) -> Result<(), String> {
    db.add_watch(&code, &market, &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_watch(
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
) -> Result<(), String> {
    db.remove_watch(&code, &market)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_watch(
    db: State<'_, Arc<Database>>,
    ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_watch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_watch_top(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    let items = db.get_watchlist().map_err(|e| e.to_string())?;
    let mut ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    if let Some(pos) = ids.iter().position(|&x| x == id) {
        ids.remove(pos);
        ids.insert(0, id);
    }
    db.reorder_watch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_watch_up(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    let items = db.get_watchlist().map_err(|e| e.to_string())?;
    let mut ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    if let Some(pos) = ids.iter().position(|&x| x == id) {
        if pos > 0 {
            ids.swap(pos, pos - 1);
        }
    }
    db.reorder_watch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_watch_down(
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    let items = db.get_watchlist().map_err(|e| e.to_string())?;
    let mut ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    if let Some(pos) = ids.iter().position(|&x| x == id) {
        if pos + 1 < ids.len() {
            ids.swap(pos, pos + 1);
        }
    }
    db.reorder_watch(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_stocks(
    manager: State<'_, Arc<DataSourceManager>>,
    keyword: String,
) -> Result<Vec<crate::domain::StockBrief>, String> {
    // Try active source first
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

    // Fallback to other data source if active source returns empty
    if results.is_empty() {
        let fallback = if active_name == "sina" { "tencent" } else { "sina" };
        if let Some(fb) = manager.get_source(fallback) {
            match fb.search(&keyword, "CN").await {
                Ok(fb_results) => results = fb_results,
                Err(e) => log::warn!("Fallback search via {} failed: {}", fallback, e),
            }
        }
    }

    Ok(results)
}
