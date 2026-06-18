use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Emitter;
use crate::domain::{Quote, IndexQuote};
use crate::datasource::market_clock::MarketSession;

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
            let mut quotes = self.quotes.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Update in-memory cache only (fast, no I/O)
    pub fn update_quotes_memory(&self, quotes: &[Quote]) {
        let mut cache = self.quotes.lock().unwrap_or_else(|e| e.into_inner());
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
    }

    /// Persist quotes to SQLite (call via spawn_blocking to avoid blocking tokio)
    pub fn persist_quotes(&self, quotes: &[Quote]) {
        if let Err(e) = self.db.cache_quotes(quotes) {
            log::warn!("Failed to persist quotes to DB: {}", e);
        }
    }

    /// Update cache with fresh quotes (combines memory update + best-effort DB write)
    /// Prefer update_quotes_memory + spawn_blocking persist_quotes in async contexts.
    pub fn update_quotes(&self, quotes: &[Quote]) {
        self.update_quotes_memory(quotes);
        // Best-effort sync write — use update_quotes_memory + spawn_blocking persist_quotes
        // in async contexts to avoid blocking tokio worker threads.
        self.persist_quotes(quotes);
    }

    /// Get all cached quotes
    pub fn get_all_quotes(&self) -> Vec<Quote> {
        let cache = self.quotes.lock().unwrap_or_else(|e| e.into_inner());
        cache.values().map(|c| c.data.clone()).collect()
    }

    /// Get a specific quote by market and code
    #[allow(dead_code)]
    pub fn get_quote(&self, market: &str, code: &str) -> Option<Quote> {
        let cache = self.quotes.lock().unwrap_or_else(|e| e.into_inner());
        let key = format!("{}:{}", market, code);
        cache.get(&key).map(|c| c.data.clone())
    }

    /// Update indices
    pub fn update_indices(&self, indices: Vec<IndexQuote>) {
        *self.indices.lock().unwrap_or_else(|e| e.into_inner()) = indices;
    }

    /// Get cached indices
    pub fn get_indices(&self) -> Vec<IndexQuote> {
        self.indices.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Background polling scheduler
pub struct Scheduler;

impl Scheduler {
    /// Spawn the global polling loop in a background tokio task.
    pub fn spawn(
        data_manager: Arc<crate::datasource::DataSourceManager>,
        cache: Arc<QuoteCache>,
        db: Arc<crate::db::Database>,
        app_handle: tauri::AppHandle,
        base_interval_secs: u64,
    ) {
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(base_interval_secs));
            let mut last_session = MarketSession::current();

            // Fetch guard — prevents concurrent fetch_once calls from the two loops
            let fetching = Arc::new(AtomicBool::new(false));

            // Background wakeup listener — triggered on datasource switch for immediate refresh
            let dm = data_manager.clone();
            let c = cache.clone();
            let d = db.clone();
            let ah = app_handle.clone();
            let fg = fetching.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    dm.wakeup.notified().await;
                    Self::fetch_once(&dm, &c, &d, &ah, &fg).await;
                }
            });

            loop {
                interval.tick().await;

                // Check market session and adjust interval dynamically
                let session = MarketSession::current();
                if session != last_session {
                    let new_interval = session.recommended_interval();
                    interval = tokio::time::interval(Duration::from_secs(new_interval));
                    last_session = session;
                    if let Err(e) = app_handle.emit("market-session-changed", serde_json::json!({
                        "session": session.name(),
                        "interval_secs": new_interval,
                    })) {
                        log::warn!("Failed to emit market-session-changed: {}", e);
                    }
                }

                Self::fetch_once(&data_manager, &cache, &db, &app_handle, &fetching).await;
            }
        });
    }

    /// Run one fetch cycle — used by both the interval loop and on-demand wakeups.
    /// The `fetching` guard prevents concurrent fetches from the two loops.
    async fn fetch_once(
        manager: &crate::datasource::DataSourceManager,
        cache: &Arc<QuoteCache>,
        db: &std::sync::Arc<crate::db::Database>,
        app_handle: &tauri::AppHandle,
        fetching: &AtomicBool,
    ) {
        // Skip if a fetch is already in progress (prevents duplicate API calls)
        if fetching.swap(true, Ordering::AcqRel) {
            return;
        }
        // Ensure the flag is cleared on exit (including early returns)
        let _guard = FetchGuard(fetching);

        let session = MarketSession::current();

        // 1. Get watchlist codes (use spawn_blocking to avoid blocking tokio worker)
        let db_for_codes = db.clone();
        let codes = match tokio::task::spawn_blocking(move || db_for_codes.get_watch_codes()).await
        {
            Ok(Ok(c)) if !c.is_empty() => c,
            Ok(Ok(_)) => {
                // Empty watchlist — skip quote fetch
                Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                return;
            }
            Ok(Err(e)) => {
                log::warn!("Failed to read watchlist from DB: {}", e);
                Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                return;
            }
            Err(join_err) => {
                log::warn!("spawn_blocking join error for get_watch_codes: {}", join_err);
                Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                return;
            }
        };

        // 2. Group by market
        let mut cn_codes: Vec<String> = Vec::new();
        for (code, market) in &codes {
            if market == "CN" {
                cn_codes.push(code.clone());
            }
        }

        // 3. Batch fetch quotes (skip API calls when market is closed, emit cached)
        if session == MarketSession::Closed {
            let cached = cache.get_all_quotes();
            if !cached.is_empty() {
                if let Err(e) = app_handle.emit("quotes-updated", &cached) {
                    log::warn!("Failed to emit quotes-updated (cached): {}", e);
                }
            }
            Self::fetch_and_emit_indices(manager, cache, app_handle).await;
            return;
        }

        if !cn_codes.is_empty() {
            if let Some(source) = manager.active_source() {
                match source.fetch_realtime(&cn_codes, "CN").await {
                    Ok(quotes) => {
                        cache.update_quotes_memory(&quotes);
                        if let Err(e) = app_handle.emit("quotes-updated", &quotes) {
                            log::warn!("Failed to emit quotes-updated: {}", e);
                        }
                        // Persist to DB via spawn_blocking to avoid blocking tokio
                        let cache_for_persist = cache.clone();
                        let quotes_for_db = quotes.to_vec();
                        tokio::task::spawn_blocking(move || {
                            cache_for_persist.persist_quotes(&quotes_for_db);
                        });
                    }
                    Err(e) => {
                        log::warn!("Quote fetch failed, serving cached data: {}", e);
                        let cached = cache.get_all_quotes();
                        if !cached.is_empty() {
                            if let Err(e) = app_handle.emit("quotes-updated", &cached) {
                                log::warn!("Failed to emit quotes-updated (fallback): {}", e);
                            }
                        }
                    }
                }
            }
        }

        // 4. Refresh indices
        Self::fetch_and_emit_indices(manager, cache, app_handle).await;
    }

    async fn fetch_and_emit_indices(
        manager: &crate::datasource::DataSourceManager,
        cache: &Arc<QuoteCache>,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(source) = manager.active_source() {
            if let Ok(indices) = source.fetch_indices().await {
                cache.update_indices(indices.clone());
                if let Err(e) = app_handle.emit("indices-updated", &indices) {
                    log::warn!("Failed to emit indices-updated: {}", e);
                }
            }
        }
    }
}

/// RAII guard that clears the fetch-in-progress flag on drop.
struct FetchGuard<'a>(&'a AtomicBool);

impl<'a> Drop for FetchGuard<'a> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
