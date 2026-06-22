use tauri::State;
use std::collections::HashMap;
use std::sync::Arc;
use crate::db::Database;
use crate::datasource::DataSourceManager;
use crate::PortableMode;

#[tauri::command]
pub fn get_settings(db: State<'_, Arc<Database>>) -> Result<HashMap<String, String>, String> {
    let pairs = db.get_all_settings().map_err(|e| e.to_string())?;
    Ok(pairs.into_iter().collect())
}

#[tauri::command]
pub fn set_setting(
    db: State<'_, Arc<Database>>,
    key: String,
    value: String,
) -> Result<(), String> {
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn switch_datasource(
    manager: State<'_, Arc<DataSourceManager>>,
    db: State<'_, Arc<Database>>,
    name: String,
) -> Result<(), String> {
    // Persist to DB first so a restart doesn't revert to the old source.
    db.set_setting("active_datasource", &name)
        .map_err(|e| e.to_string())?;
    manager.set_active(&name)
}

#[tauri::command]
pub fn list_datasources(manager: State<'_, Arc<DataSourceManager>>) -> Vec<(String, String)> {
    manager
        .list_sources()
        .into_iter()
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect()
}

/// Query whether the app is running in portable mode (portable.dat next to exe).
#[tauri::command]
pub fn get_portable_mode(portable: State<'_, PortableMode>) -> bool {
    portable.0
}
