use tauri::{Emitter, State};
use std::sync::Arc;
use crate::db::{Database, WatchGroup, WatchlistSnapshot};
use crate::datasource::DataSourceManager;

/// 自选表的一次性全量状态:股票池 + 分组(含组内有序成员) + 默认分组。
///
/// 不用 `get_watchlist` + `get_watch_groups` 两个命令:分组标签栏切换必须瞬时，
/// 分两次拉就必然出现「分组已到、成员还没到」的中间态。一次拿全，前端切分组是
/// 纯本地过滤。
#[tauri::command]
pub fn get_watchlist(db: State<'_, Arc<Database>>) -> Result<WatchlistSnapshot, String> {
    db.get_watchlist_snapshot().map_err(|e| e.to_string())
}

/// 新增自选:股票入池(已在池中则复用) + 加入勾选的分组。
/// `group_ids` 为空/None 时落进默认分组。多归属下已在其它分组也能直接加入,不做移动。
#[tauri::command]
pub fn add_watch(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    code: String,
    market: String,
    name: String,
    group_ids: Option<Vec<i64>>,
) -> Result<(), String> {
    db.add_watch(&code, &market, &name, group_ids.as_deref().unwrap_or(&[]))
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

/// 彻底删除自选:解除全部分组关联并从池中移除。
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

/// 把自选移出某个分组。返回 true 表示它因此不属于任何分组、已被连根删除
/// —— 前端据此在菜单里把文案换成「删除自选」，所以这个返回值是行为契约的一部分。
#[tauri::command]
pub fn remove_watch_from_group(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    group_id: i64,
    watch_id: i64,
) -> Result<bool, String> {
    let deleted = db
        .remove_watch_from_group(group_id, watch_id)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(deleted)
}

/// 全量覆盖一只自选的分组归属(右键「添加到分组」多选提交)。
/// 空集合会被后端拒绝 —— 勾选框全清空是很容易误触的状态，静默删数据代价太大。
#[tauri::command]
pub fn set_watch_groups(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    watch_id: i64,
    group_ids: Vec<i64>,
) -> Result<(), String> {
    db.set_watch_groups(watch_id, &group_ids)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

// ── 组内排序 ──
// 全部按 (group_id, watch_id) 定位:多归属下同一只股票在不同分组里位置不同，
// 只给 watch_id 无法确定该动哪个分组。

#[tauri::command]
pub fn move_group_member_top(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    group_id: i64,
    watch_id: i64,
) -> Result<(), String> {
    db.move_group_member_top(group_id, watch_id)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn move_group_member_up(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    group_id: i64,
    watch_id: i64,
) -> Result<(), String> {
    db.move_group_member_up(group_id, watch_id)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn move_group_member_down(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    group_id: i64,
    watch_id: i64,
) -> Result<(), String> {
    db.move_group_member_down(group_id, watch_id)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub fn reorder_group_members(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    group_id: i64,
    watch_ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_group_members(group_id, &watch_ids)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

// ── 分组 CRUD ──

/// 新建分组。返回完整分组行，前端可直接切到新分组，不必再拉一次列表。
#[tauri::command]
pub fn add_watch_group(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    name: String,
) -> Result<WatchGroup, String> {
    let group = db.add_watch_group(&name).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(group)
}

#[tauri::command]
pub fn rename_watch_group(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
    name: String,
) -> Result<(), String> {
    db.rename_watch_group(id, &name).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

/// 删除分组。不删自选 —— 仅解除该分组关联，因此变成孤儿的股票并入默认分组。
/// 返回被救回的孤儿数量，供前端提示影响面。
#[tauri::command]
pub fn delete_watch_group(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<usize, String> {
    let rescued = db.delete_watch_group(id).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(rescued)
}

#[tauri::command]
pub fn reorder_watch_groups(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_watch_groups(&ids).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

// ── 行情条播报范围 ──

#[tauri::command]
pub fn set_watch_ticker_enabled(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    db.set_watch_ticker_enabled(id, enabled)
        .map_err(|e| e.to_string())?;
    // 复用已有的 watchlist-changed 事件：行情条窗口正是靠它刷新列表，
    // 因此开关一拨即在行情条生效，无需新增事件。
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

/// 批量开关播报 —— 设置页「按分组全选/全不选」用。
#[tauri::command]
pub fn set_ticker_enabled_bulk(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    ids: Vec<i64>,
    enabled: bool,
) -> Result<(), String> {
    db.set_ticker_enabled_bulk(&ids, enabled)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

/// 重排行情条轮播顺序。传入的是设置页「全部」视图下播报范围列表的顺序。
#[tauri::command]
pub fn reorder_ticker(
    app_handle: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_ticker(&ids).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("watchlist-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn search_stocks(
    manager: State<'_, Arc<DataSourceManager>>,
    keyword: String,
) -> Result<Vec<crate::domain::StockBrief>, String> {
    Ok(crate::datasource::search::search_cn(manager.inner().as_ref(), &keyword).await)
}
