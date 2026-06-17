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
    let source = manager.active_source();
    let mut results = source.search(&keyword, "CN").await.unwrap_or_default();

    // Fallback to Eastmoney search if active source (e.g. Sina) returns empty
    if results.is_empty() && source.name() != "eastmoney" {
        if let Some(em) = manager.get_source("eastmoney") {
            if let Ok(em_results) = em.search(&keyword, "CN").await {
                results = em_results;
            }
        }
    }

    Ok(results)
}
