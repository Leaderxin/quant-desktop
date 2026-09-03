use tauri::State;
use std::sync::Arc;
use crate::cache::QuoteCache;
use crate::datasource::DataSourceManager;
use crate::domain::{Quote, IndexQuote, Depth, MinuteData, KLineData};

#[tauri::command]
pub fn get_quotes(cache: State<'_, Arc<QuoteCache>>) -> Vec<Quote> {
    cache.get_all_quotes()
}

#[tauri::command]
pub fn get_indices(cache: State<'_, Arc<QuoteCache>>) -> Vec<IndexQuote> {
    cache.get_indices()
}

#[tauri::command]
pub async fn get_depth(
    code: String,
    market: String,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Depth, String> {
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_depth(&code, &market).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_intraday(
    code: String,
    market: String,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Vec<MinuteData>, String> {
    // 优先用活跃源；返回空时回退到备用源（如腾讯对北交所 bj 代码无分时数据）。
    let active_name = manager.active_name();
    if let Some(source) = manager.get_source(&active_name) {
        if let Ok(data) = source.fetch_minute_data(&code, &market).await {
            if !data.is_empty() {
                return Ok(data);
            }
        }
    }
    for (name, source) in manager.all_sources() {
        if name == active_name {
            continue;
        }
        match source.fetch_minute_data(&code, &market).await {
            Ok(data) if !data.is_empty() => return Ok(data),
            Ok(_) => {}
            Err(e) => log::warn!("分时回退数据源 {} 失败: {}", name, e),
        }
    }
    Ok(vec![])
}

#[tauri::command]
pub async fn get_kline(
    code: String,
    market: String,
    period: String,
    end_date: Option<String>,
    count: Option<u32>,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Vec<KLineData>, String> {
    // 优先用活跃源；仅返回单根 K 线时视为数据缺失（如腾讯对北交所仅返回当日），回退备用源。
    let active_name = manager.active_name();
    let mut active_result: Vec<KLineData> = Vec::new();
    if let Some(source) = manager.get_source(&active_name) {
        if let Ok(data) = source
            .fetch_kline(&code, &market, &period, end_date.as_deref(), count)
            .await
        {
            active_result = data;
            if active_result.len() >= 2 {
                return Ok(active_result);
            }
        }
    }
    for (name, source) in manager.all_sources() {
        if name == active_name {
            continue;
        }
        match source
            .fetch_kline(&code, &market, &period, end_date.as_deref(), count)
            .await
        {
            Ok(data) if !data.is_empty() => return Ok(data),
            Ok(_) => {}
            Err(e) => log::warn!("K线回退数据源 {} 失败: {}", name, e),
        }
    }
    // 回退失败时返回活跃源的结果（可能为空或单根）。
    Ok(active_result)
}
