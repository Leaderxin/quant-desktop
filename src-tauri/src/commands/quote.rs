use tauri::State;
use std::sync::Arc;
use crate::cache::QuoteCache;
use crate::domain::{Quote, IndexQuote};

#[tauri::command]
pub fn get_quotes(cache: State<'_, Arc<QuoteCache>>) -> Vec<Quote> {
    cache.get_all_quotes()
}

#[tauri::command]
pub fn get_indices(cache: State<'_, Arc<QuoteCache>>) -> Vec<IndexQuote> {
    cache.get_indices()
}
