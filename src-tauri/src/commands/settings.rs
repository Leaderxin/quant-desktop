use tauri::{Emitter, State};
use std::collections::HashMap;
use std::sync::Arc;
use crate::db::Database;
use crate::datasource::DataSourceManager;

#[tauri::command]
pub fn get_settings(db: State<'_, Arc<Database>>) -> Result<HashMap<String, String>, String> {
    let pairs = db.get_all_settings().map_err(|e| e.to_string())?;
    Ok(pairs.into_iter().collect())
}

#[tauri::command]
pub fn set_setting(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    key: String,
    value: String,
) -> Result<(), String> {
    db.set_setting(&key, &value).map_err(|e| e.to_string())?;
    if key == "theme" {
        let _ = app.emit("theme-changed", &value);
    }
    Ok(())
}

#[tauri::command]
pub fn switch_datasource(
    manager: State<'_, Arc<DataSourceManager>>,
    db: State<'_, Arc<Database>>,
    name: String,
) -> Result<(), String> {
    manager.set_active(&name)?;
    db.set_setting("active_datasource", &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_datasources(manager: State<'_, Arc<DataSourceManager>>) -> Vec<(String, String)> {
    manager
        .list_sources()
        .into_iter()
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect()
}
