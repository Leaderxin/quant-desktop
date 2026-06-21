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
    _cached_at: std::time::Instant,
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
                        _cached_at: now,
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
                    _cached_at: now,
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

    /// Update indices
    pub fn update_indices(&self, indices: Vec<IndexQuote>) {
        *self.indices.lock().unwrap_or_else(|e| e.into_inner()) = indices;
    }

    /// Get cached indices
    pub fn get_indices(&self) -> Vec<IndexQuote> {
        self.indices.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Snapshot current prices (code→price) for change detection
    pub fn get_price_snapshot(&self) -> HashMap<String, f64> {
        let cache = self.quotes.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .iter()
            .map(|(k, v)| (k.clone(), v.data.price))
            .collect()
    }
}

// ── Adaptive polling constants ──

/// Number of probes at session start to detect if market is actually open
const PROBE_COUNT: u32 = 3;
/// Interval used during probing phase (seconds)
const PROBE_INTERVAL: u64 = 2;
/// Number of consecutive unchanged polls before switching to idle
const STREAK_THRESHOLD: u32 = 5;
/// Idle polling interval when market is detected as closed (seconds)
const IDLE_INTERVAL: u64 = 30;

/// Adaptive polling state machine — detects holidays via price stasis
#[derive(Debug, Clone, Copy, PartialEq)]
enum PollingState {
    /// Probing at session start to determine if market is actually open
    Probing { remaining: u32 },
    /// Market is open, normal frequency. Tracks consecutive unchanged polls.
    Normal { unchanged_streak: u32 },
    /// Market detected as closed (holiday), throttled to idle frequency.
    Idle,
}

impl PollingState {
    fn new() -> Self {
        Self::Probing { remaining: PROBE_COUNT }
    }

    /// Reset to probing when entering a trading session
    fn on_session_enter(&mut self) {
        *self = Self::Probing { remaining: PROBE_COUNT };
    }

    /// Update state based on fetch result. Returns the interval for the next cycle.
    fn update(&mut self, prices_changed: bool, session: MarketSession) -> u64 {
        match self {
            Self::Probing { remaining } => {
                if prices_changed {
                    log::info!("Probe detected price change — market is open");
                    *self = Self::Normal { unchanged_streak: 0 };
                    return session.recommended_interval();
                }
                *remaining -= 1;
                if *remaining == 0 {
                    log::info!("All probes returned no price change — switching to idle (holiday/closure)");
                    *self = Self::Idle;
                    return IDLE_INTERVAL;
                }
                PROBE_INTERVAL
            }
            Self::Normal { unchanged_streak } => {
                if prices_changed {
                    *unchanged_streak = 0;
                } else {
                    *unchanged_streak += 1;
                    if *unchanged_streak >= STREAK_THRESHOLD {
                        log::info!(
                            "{} consecutive polls with no price change — switching to idle",
                            *unchanged_streak
                        );
                        *self = Self::Idle;
                        return IDLE_INTERVAL;
                    }
                }
                session.recommended_interval()
            }
            Self::Idle => {
                if prices_changed {
                    log::info!("Price change detected in idle mode — resuming normal polling");
                    *self = Self::Normal { unchanged_streak: 0 };
                    return session.recommended_interval();
                }
                IDLE_INTERVAL
            }
        }
    }
}

/// Outcome of a fetch cycle — indicates whether quote prices changed vs the cache
struct FetchOutcome {
    prices_changed: bool,
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
            let initial_interval = Duration::from_secs(base_interval_secs);
            let mut interval = tokio::time::interval(initial_interval);
            let mut last_session = MarketSession::current();
            let mut state = PollingState::new();

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
                    Self::fetch_once(&dm, &c, &d, &ah, &fg, true).await;
                }
            });

            loop {
                interval.tick().await;

                // ── Session transition handling ──
                let session = MarketSession::current();
                if session != last_session {
                    let is_trading_enter = matches!(
                        (last_session, session),
                        (MarketSession::PreOpen, MarketSession::MorningTrade)
                            | (MarketSession::LunchBreak, MarketSession::AfternoonTrade)
                    );

                    if is_trading_enter {
                        state.on_session_enter();
                        log::info!(
                            "Entering trading session ({:?}), starting probe",
                            session
                        );
                    }

                    last_session = session;
                    if let Err(e) = app_handle.emit("market-session-changed", serde_json::json!({
                        "session": session.name(),
                        "interval_secs": session.recommended_interval(),
                    })) {
                        log::warn!("Failed to emit market-session-changed: {}", e);
                    }
                }

                // ── Fetch data ──
                let outcome = Self::fetch_once(&data_manager, &cache, &db, &app_handle, &fetching, false).await;

                // ── Adaptive interval ──
                // Only update the state machine when we have actual quote data.
                // None means empty watchlist / error / closed market — no data to
                // judge price changes, so fall back to market_clock recommendation.
                let new_interval = match outcome {
                    Some(o) => state.update(o.prices_changed, session),
                    None => session.recommended_interval(),
                };
                interval = tokio::time::interval(Duration::from_secs(new_interval));
            }
        });
    }

    /// Run one fetch cycle. Returns `FetchOutcome` with price-change info,
    /// or `None` if no quote data was fetched (empty watchlist, error, closed market).
    async fn fetch_once(
        manager: &crate::datasource::DataSourceManager,
        cache: &Arc<QuoteCache>,
        db: &std::sync::Arc<crate::db::Database>,
        app_handle: &tauri::AppHandle,
        fetching: &AtomicBool,
        force: bool,
    ) -> Option<FetchOutcome> {
        // Skip if a fetch is already in progress (prevents duplicate API calls)
        if fetching.swap(true, Ordering::AcqRel) {
            return None;
        }
        let _guard = FetchGuard(fetching);

        let session = MarketSession::current();

        // 1. Get watchlist codes
        let db_for_codes = db.clone();
        let codes = match tokio::task::spawn_blocking(move || db_for_codes.get_watch_codes()).await
        {
            Ok(Ok(c)) if !c.is_empty() => c,
            Ok(Ok(_)) => {
                Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                return None;
            }
            Ok(Err(e)) => {
                log::warn!("Failed to read watchlist from DB: {}", e);
                Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                return None;
            }
            Err(join_err) => {
                log::warn!("spawn_blocking join error for get_watch_codes: {}", join_err);
                Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                return None;
            }
        };

        // 2. Group by market
        let mut cn_codes: Vec<String> = Vec::new();
        for (code, market) in &codes {
            if market == "CN" {
                cn_codes.push(code.clone());
            }
        }

        // 3. When market is Closed (after 15:00 or weekend), serve from cache
        //    if it fully covers the watchlist — otherwise fetch to populate it.
        if session == MarketSession::Closed && !force {
            let cached = cache.get_all_quotes();
            if !cached.is_empty() {
                // Check whether cached quotes cover every watchlist code
                let all_cached = cn_codes.iter().all(|code| {
                    cached.iter().any(|q| q.code == *code && q.market == "CN")
                });
                if all_cached {
                    if let Err(e) = app_handle.emit("quotes-updated", &cached) {
                        log::warn!("Failed to emit quotes-updated (cached): {}", e);
                    }
                    Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                    return None;
                }
                log::info!("Cache incomplete during closed market — fetching to backfill missing stocks");
            } else {
                log::info!("Cache empty during closed market — fetching to populate");
            }
            // Cache is empty or incomplete → fall through to fetch from API
        }

        if !cn_codes.is_empty() {
            if let Some(source) = manager.active_source() {
                // Snapshot prices before fetch for change detection
                let prices_before = cache.get_price_snapshot();

                match source.fetch_realtime(&cn_codes, "CN").await {
                    Ok(quotes) => {
                        cache.update_quotes_memory(&quotes);
                        if let Err(e) = app_handle.emit("quotes-updated", &quotes) {
                            log::warn!("Failed to emit quotes-updated: {}", e);
                        }
                        let cache_for_persist = cache.clone();
                        let quotes_for_db = quotes.to_vec();
                        tokio::task::spawn_blocking(move || {
                            cache_for_persist.persist_quotes(&quotes_for_db);
                        });

                        // Compare: did any price actually change?
                        let changed = Self::any_price_changed(&prices_before, &quotes);

                        Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                        return Some(FetchOutcome { prices_changed: changed });
                    }
                    Err(e) => {
                        log::warn!("Quote fetch failed (will retry): {}", e);
                        let cached = cache.get_all_quotes();
                        if !cached.is_empty() {
                            if let Err(e) = app_handle.emit("quotes-updated", &cached) {
                                log::warn!("Failed to emit quotes-updated (fallback): {}", e);
                            }
                        }
                        // Fetch failed — we can't determine price change, so return None
                        // to keep the state machine from making a false decision.
                        Self::fetch_and_emit_indices(manager, cache, app_handle).await;
                        return None;
                    }
                }
            }
        }

        Self::fetch_and_emit_indices(manager, cache, app_handle).await;
        None
    }

    /// Check whether any quote price differs from the snapshot
    fn any_price_changed(snapshot: &std::collections::HashMap<String, f64>, quotes: &[crate::domain::Quote]) -> bool {
        for q in quotes {
            let key = format!("{}:{}", q.market, q.code);
            match snapshot.get(&key) {
                Some(&prev_price) if (prev_price - q.price).abs() > f64::EPSILON => return true,
                Some(_) => {} // same price
                None => return true, // new stock added to watchlist
            }
        }
        // Also check: were stocks removed from watchlist?
        if snapshot.len() != quotes.len() {
            return true;
        }
        false
    }

    async fn fetch_and_emit_indices(
        manager: &crate::datasource::DataSourceManager,
        cache: &Arc<QuoteCache>,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(source) = manager.active_source() {
            match source.fetch_indices().await {
                Ok(indices) => {
                    cache.update_indices(indices.clone());
                    if let Err(e) = app_handle.emit("indices-updated", &indices) {
                        log::warn!("Failed to emit indices-updated: {}", e);
                    }
                }
                Err(e) => log::warn!("Index fetch failed: {}", e),
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
