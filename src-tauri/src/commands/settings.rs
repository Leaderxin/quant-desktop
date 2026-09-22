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
    // 键名取自 db::keys —— 这里和 lib.rs 的启动恢复是同一个键的两端，
    // 任一处拼错都会表现为"切换了数据源但重启后变回去"。
    db.set_setting(crate::db::keys::ACTIVE_DATASOURCE, &name)
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

/// 指数候选池 (代码, 中文名)，供设置页列出可勾选的指数。
///
/// 名字在后端写死一份的原因见 `datasource::INDEX_POOL_NAMES` 的注释 ——
/// 前端再抄一份字面量迟早会与候选池漂移。
#[tauri::command]
pub fn list_index_pool() -> Vec<(String, String)> {
    crate::datasource::INDEX_POOL_NAMES
        .iter()
        .map(|(code, name)| (code.to_string(), name.to_string()))
        .collect()
}

/// Query whether the app is running in portable mode (portable.dat next to exe).
#[tauri::command]
pub fn get_portable_mode(portable: State<'_, PortableMode>) -> bool {
    portable.0
}

/// Whether this is a Microsoft Store build. Store builds disable the built-in
/// updater (the Store distributes updates itself); the frontend uses this to
/// hide the "check update" UI.
#[tauri::command]
pub fn is_store_build() -> bool {
    cfg!(feature = "store")
}
