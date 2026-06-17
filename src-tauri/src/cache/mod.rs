use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;
use crate::domain::{Quote, IndexQuote};

/// Quote cache (in-memory + SQLite dual-write)
pub struct QuoteCache {
    quotes: Mutex<HashMap<String, CachedQuote>>,
    indices: Mutex<Vec<IndexQuote>>,
    db: Arc<crate::db::Database>,
}

struct CachedQuote {
    data: Quote,
    #[allow(dead_code)]
    cached_at: std::time::Instant,
}

impl QuoteCache {
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self {
            quotes: Mutex::new(HashMap::new()),
            indices: Mutex::new(Vec::new()),
            db,
        }
    }

    /// Restore cache from SQLite (called on startup)
    pub fn restore_from_db(&self) {
        if let Ok(cached) = self.db.get_cached_quotes() {
            let mut quotes = self.quotes.lock().unwrap();
            let now = std::time::Instant::now();
            for q in cached {
                let key = format!("{}:{}", q.market, q.code);
                quotes.insert(
                    key,
                    CachedQuote {
                        data: q,
                        cached_at: now,
                    },
                );
            }
        }
    }

    /// Update cache with fresh quotes
    pub fn update_quotes(&self, quotes: &[Quote]) {
        let mut cache = self.quotes.lock().unwrap();
        let now = std::time::Instant::now();
        for q in quotes {
            let key = format!("{}:{}", q.market, q.code);
            cache.insert(
                key,
                CachedQuote {
                    data: q.clone(),
                    cached_at: now,
                },
            );
        }
        // Best-effort async write to SQLite
        let _ = self.db.cache_quotes(quotes);
    }

    /// Get all cached quotes
    pub fn get_all_quotes(&self) -> Vec<Quote> {
        let cache = self.quotes.lock().unwrap();
        cache.values().map(|c| c.data.clone()).collect()
    }

    /// Get a specific quote by market and code
    #[allow(dead_code)]
    pub fn get_quote(&self, market: &str, code: &str) -> Option<Quote> {
        let cache = self.quotes.lock().unwrap();
        let key = format!("{}:{}", market, code);
        cache.get(&key).map(|c| c.data.clone())
    }

    /// Update indices
    pub fn update_indices(&self, indices: Vec<IndexQuote>) {
        *self.indices.lock().unwrap() = indices;
    }

    /// Get cached indices
    pub fn get_indices(&self) -> Vec<IndexQuote> {
        self.indices.lock().unwrap().clone()
    }
}

/// Background polling scheduler
pub struct Scheduler;

impl Scheduler {
    /// Spawn the global polling loop (runs in a background tokio task)
    pub fn spawn(
        data_manager: Arc<crate::datasource::DataSourceManager>,
        cache: Arc<QuoteCache>,
        db: Arc<crate::db::Database>,
        app_handle: tauri::AppHandle,
        interval_secs: u64,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                // 1. Get watchlist codes
                let codes = match db.get_watch_codes() {
                    Ok(c) if !c.is_empty() => c,
                    _ => {
                        // No watchlist: just refresh indices
                        Self::fetch_and_emit_indices(
                            &data_manager, &cache, &app_handle,
                        )
                        .await;
                        continue;
                    }
                };

                // 2. Group by market
                let mut cn_codes: Vec<String> = Vec::new();
                for (code, market) in &codes {
                    if market == "CN" {
                        cn_codes.push(code.clone());
                    }
                }

                // 3. Batch fetch quotes
                if !cn_codes.is_empty() {
                    let source = data_manager.active_source();
                    match source.fetch_realtime(&cn_codes, "CN").await {
                        Ok(quotes) => {
                            cache.update_quotes(&quotes);
                            let _ = app_handle.emit("quotes-updated", &quotes);
                        }
                        Err(_e) => {
                            // Fallback: emit cached data
                            let cached = cache.get_all_quotes();
                            if !cached.is_empty() {
                                let _ = app_handle.emit("quotes-updated", &cached);
                            }
                        }
                    }
                }

                // 4. Refresh indices every cycle
                Self::fetch_and_emit_indices(&data_manager, &cache, &app_handle).await;
            }
        });
    }

    async fn fetch_and_emit_indices(
        manager: &crate::datasource::DataSourceManager,
        cache: &QuoteCache,
        app_handle: &tauri::AppHandle,
    ) {
        let source = manager.active_source();
        if let Ok(indices) = source.fetch_indices().await {
            cache.update_indices(indices.clone());
            let _ = app_handle.emit("indices-updated", &indices);
        }
    }
}
