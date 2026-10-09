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

/// 设置老板键（设置页 → 通用）。空串表示取消。
///
/// 没有走 `set_setting`：改热键不只是写一行库 —— 还要把旧键注销、新键注册
/// 上去，而注册会失败（组合键已被别的程序占着），用户需要看到这个失败。
///
/// 顺序是「先注册、成功了再落库」，失败时**回滚到旧键**：`hotkey::apply`
/// 内部是先注销全部再注册，新键注册不上时旧键已经没了 —— 不回滚的话，用户
/// 试着换一个已被占用的键，代价是连原来能用的那个也一起丢掉。
#[tauri::command]
pub fn set_boss_key(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    accelerator: String,
) -> Result<(), String> {
    let previous = db
        .get_setting(crate::db::keys::BOSS_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();

    if let Err(e) = crate::hotkey::apply(&app, &accelerator) {
        if let Err(rollback) = crate::hotkey::apply(&app, &previous) {
            log::warn!("[hotkey] Failed to restore previous boss key: {}", rollback);
        }
        return Err(e);
    }

    db.set_setting(crate::db::keys::BOSS_KEY, &accelerator)
        .map_err(|e| e.to_string())
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
/// choose *which* update UI to show, not whether to show one — the About page
/// swaps the in-app check button for a Store link, and only the status bar
/// hides its button outright.
#[tauri::command]
pub fn is_store_build() -> bool {
    cfg!(feature = "store")
}
