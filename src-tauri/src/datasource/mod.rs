use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;
use tokio::sync::Notify;
use reqwest::Client;
use crate::domain::*;
use crate::domain::AppError;

// ── Shared HTTP Client ──
// A single reqwest::Client shared across all data source adapters.
// Client::clone() is a shallow copy — the connection pool, TLS config,
// and timeout are all shared by every clone.

static SHARED_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get or initialize the shared reqwest::Client.
/// All adapters should call this and clone the handle.
pub fn shared_client() -> &'static Client {
    SHARED_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ",
                "AppleWebKit/537.36 (KHTML, like Gecko) ",
                "Chrome/131.0.0.0 Safari/537.36"
            ))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to build shared reqwest Client — TLS backend may be missing")
    })
}

// ── Shared constants across data source adapters ──

/// Major A-share index codes (Shanghai + Shenzhen)
pub const INDEX_CODES: &str =
    "s_sh000001,s_sz399001,s_sz399006,s_sh000688,s_sh000698,s_sh000905,s_sh000680";

/// Ticker window default dimensions
pub const TICKER_WIDTH: u32 = 230;
pub const TICKER_HEIGHT: u32 = 38;

// ── Data Normalization ──

/// 成交量归一化: 手 → 股 (×100)
pub const VOLUME_HANDS_TO_SHARES: u64 = 100;

/// 成交额归一化: 万元 → 元 (×10000)
pub const TURNOVER_WAN_TO_YUAN: f64 = 10000.0;

/// 将成交量从手转换为股
#[inline]
pub fn normalize_volume(volume_hands: u64) -> u64 {
    volume_hands * VOLUME_HANDS_TO_SHARES
}

/// 将成交额从万元转换为元
#[inline]
pub fn normalize_turnover(turnover_wan: f64) -> f64 {
    turnover_wan * TURNOVER_WAN_TO_YUAN
}

/// Abstract data source trait — all market data adapters implement this
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Unique identifier for this data source
    fn name(&self) -> &str;

    /// Human-readable display name
    fn display_name(&self) -> &str;

    /// Fetch real-time quotes (batch)
    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, AppError>;

    /// Fetch major indices
    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, AppError>;

    /// Search stocks (fuzzy match code or name)
    async fn search(
        &self,
        keyword: &str,
        market: &str,
    ) -> Result<Vec<StockBrief>, AppError>;

    /// Fetch 5-level depth (bid/ask order book)
    async fn fetch_depth(
        &self,
        _code: &str,
        _market: &str,
    ) -> Result<crate::domain::Depth, AppError> {
        Ok(crate::domain::Depth {
            code: _code.to_string(),
            bids: vec![],
            asks: vec![],
        })
    }

    /// Fetch intraday minute data for charting
    async fn fetch_minute_data(
        &self,
        _code: &str,
        _market: &str,
    ) -> Result<Vec<crate::domain::MinuteData>, AppError> {
        Ok(vec![])
    }

    /// Fetch K-line data for charting (daily/weekly/monthly)
    /// - `end_date`: None → latest data; Some("YYYY-MM-DD") → data up to and including this date
    /// - `count`: None → default 200; Some(n) → return at most n bars
    async fn fetch_kline(
        &self,
        _code: &str,
        _market: &str,
        _period: &str,
        _end_date: Option<&str>,
        _count: Option<u32>,
    ) -> Result<Vec<crate::domain::KLineData>, AppError> {
        Ok(vec![])
    }

    /// Health check
    async fn health_check(&self) -> Result<bool, AppError>;
}

/// Data source manager — registration, switching, unified dispatch
pub struct DataSourceManager {
    sources: HashMap<String, Box<dyn DataSource>>,
    active: RwLock<String>,
    pub wakeup: Notify,
}

impl DataSourceManager {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            active: RwLock::new(String::new()),
            wakeup: Notify::new(),
        }
    }

    /// Register a data source. First registered source becomes active automatically.
    pub fn register(&mut self, source: Box<dyn DataSource>) {
        let name = source.name().to_string();
        if self.sources.is_empty() {
            *self.active.write().unwrap_or_else(|e| e.into_inner()) = name.clone();
        }
        self.sources.insert(name, source);
    }

    /// Switch the active data source (notifies the scheduler to refresh immediately).
    pub fn set_active(&self, name: &str) -> Result<(), String> {
        if self.sources.contains_key(name) {
            *self.active.write().unwrap_or_else(|e| e.into_inner()) = name.to_string();
            log::info!("Data source switched to: {}", name);
            self.wakeup.notify_one();
            Ok(())
        } else {
            log::warn!("Attempted to switch to unregistered data source: {}", name);
            Err(format!("Data source '{}' is not registered", name))
        }
    }

    /// Set the active data source WITHOUT notifying the scheduler.
    /// Used during initial setup — the scheduler's main loop will pick up
    /// the correct source on its first tick without a duplicate wakeup fetch.
    pub fn set_active_initial(&self, name: &str) -> Result<(), String> {
        if self.sources.contains_key(name) {
            *self.active.write().unwrap_or_else(|e| e.into_inner()) = name.to_string();
            log::info!("Data source set (initial): {}", name);
            Ok(())
        } else {
            log::warn!("Attempted to set unregistered data source: {}", name);
            Err(format!("Data source '{}' is not registered", name))
        }
    }

    /// Get the name of the currently active data source
    pub fn active_name(&self) -> String {
        self.active.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Get a reference to the currently active data source.
    /// Returns None if no source is registered (shouldn't happen after setup).
    pub fn active_source(&self) -> Option<&dyn DataSource> {
        let name = self.active.read().unwrap_or_else(|e| e.into_inner());
        self.sources.get(&*name).map(|s| s.as_ref())
    }

    /// Get a reference to a specific data source by name
    pub fn get_source(&self, name: &str) -> Option<&dyn DataSource> {
        self.sources.get(name).map(|s| s.as_ref())
    }

    /// Iterate over all registered data sources (name, source)
    pub fn all_sources(&self) -> Vec<(String, &dyn DataSource)> {
        self.sources
            .iter()
            .map(|(k, v)| (k.clone(), v.as_ref()))
            .collect()
    }

    /// List all registered data sources (id, display_name)
    pub fn list_sources(&self) -> Vec<(&str, &str)> {
        self.sources
            .iter()
            .map(|(k, v)| (k.as_str(), v.display_name()))
            .collect()
    }
}

pub mod sina;
pub mod tencent;
pub mod market_clock;
pub mod search;
pub mod headers;
