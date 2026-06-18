// src-tauri/src/domain/mod.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Market {
    #[serde(rename = "CN")]
    CN,
    #[serde(rename = "HK")]
    HK,
    #[serde(rename = "US")]
    US,
}

impl Market {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CN" => Some(Self::CN),
            "HK" => Some(Self::HK),
            "US" => Some(Self::US),
            _ => None,
        }
    }

    pub fn as_prefix(&self) -> &str {
        match self {
            Market::CN => "0",
            Market::HK => "116",
            Market::US => "105",
        }
    }
}

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
pub struct StockBrief {
    pub code: String,
    pub market: String,
    pub name: String,
}

/// 行情快照的批量查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotesResponse {
    pub quotes: Vec<Quote>,
    pub errors: Vec<String>,
}
