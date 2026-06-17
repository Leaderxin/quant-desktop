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
pub async fn search_stocks(
    manager: State<'_, Arc<DataSourceManager>>,
    keyword: String,
) -> Result<Vec<crate::domain::StockBrief>, String> {
    let source = manager.active_source();
    source.search(&keyword, "CN").await
}
