use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use crate::domain::*;

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
    ) -> Result<Vec<Quote>, String>;

    /// Fetch major indices
    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String>;

    /// Search stocks (fuzzy match code or name)
    async fn search(
        &self,
        keyword: &str,
        market: &str,
    ) -> Result<Vec<StockBrief>, String>;

    /// Health check
    async fn health_check(&self) -> Result<bool, String>;
}

/// Data source manager — registration, switching, unified dispatch
pub struct DataSourceManager {
    sources: HashMap<String, Box<dyn DataSource>>,
    active: RwLock<String>,
}

impl DataSourceManager {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            active: RwLock::new(String::new()),
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

    /// Switch the active data source
    pub fn set_active(&self, name: &str) -> Result<(), String> {
        if self.sources.contains_key(name) {
            *self.active.write().unwrap_or_else(|e| e.into_inner()) = name.to_string();
            Ok(())
        } else {
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

    /// List all registered data sources (id, display_name)
    pub fn list_sources(&self) -> Vec<(&str, &str)> {
        self.sources
            .iter()
            .map(|(k, v)| (k.as_str(), v.display_name()))
            .collect()
    }
}

pub mod eastmoney;
pub mod sina;
