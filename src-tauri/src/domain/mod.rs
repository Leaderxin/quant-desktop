// src-tauri/src/domain/mod.rs
use serde::{Deserialize, Serialize};

// Note: market is stored as a String (e.g. "CN", "HK", "US") in Quote/IndexQuote
// rather than using a typed enum, to keep the IPC boundary simple.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub code: String,
    pub market: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub turnover: f64,
    pub turnover_rate: Option<f64>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexQuote {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub volume: u64,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Depth {
    pub code: String,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub price: f64,
    pub volume: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteData {
    pub time: String,
    pub price: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub avg_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KLineData {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBrief {
    pub code: String,
    pub market: String,
    pub name: String,
}
