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
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_minute_data(&code, &market).await.map_err(|e| e.to_string())
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
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_kline(&code, &market, &period, end_date.as_deref(), count).await.map_err(|e| e.to_string())
}
