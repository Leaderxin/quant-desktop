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
    pub volume: u64,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBrief {
    pub code: String,
    pub market: String,
    pub name: String,
}

/// 单个板块(行业/概念)的涨跌排名条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorItem {
    /// 板块代码(新浪行业为 `hangye_ZA01` 之类,东财概念为 `BKxxxx`)
    pub code: String,
    /// 板块名称
    pub name: String,
    /// 板块涨跌幅(%)
    pub change_pct: f64,
    /// 领涨股名称(可选)
    pub leader_name: Option<String>,
    /// 领涨股涨跌幅(%)(可选)
    pub leader_pct: Option<f64>,
}

/// 市场概览聚合数据(总成交额 + 涨跌家数 + 板块排名)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverview {
    /// 沪深两市总成交额(元),来自 cache 指数 turnover 相加
    pub turnover: f64,
    /// 上涨家数(沪深京三市)
    pub up: u32,
    /// 下跌家数(沪深京三市)
    pub down: u32,
    /// 平盘家数(沪深京三市)
    pub flat: u32,
    /// 行业板块排名(按涨/跌方向排序后的前 N)
    pub industry: Vec<SectorItem>,
    /// 概念板块排名(按涨/跌方向排序后的前 N)
    pub concept: Vec<SectorItem>,
}

// ── Structured Error Type ──

/// Unified application error type replacing ad-hoc String errors.
/// Implements `Display` via `thiserror` so `.to_string()` produces
/// user-facing messages suitable for the Tauri IPC boundary.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("网络请求失败 ({origin}): {message}")]
    Network {
        origin: String,
        message: String,
    },

    #[error("数据源不可用: {0}")]
    DataSourceUnavailable(String),

    #[error("不支持的操作: {0}")]
    Unsupported(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("数据解析失败 ({origin}): {message}")]
    Parse {
        origin: String,
        message: String,
    },
}

impl AppError {
    /// Convenience: create a network error with origin label
    pub fn network(origin: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Network {
            origin: origin.into(),
            message: message.into(),
        }
    }

    /// Convenience: create a parse error with origin label
    pub fn parse(origin: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Parse {
            origin: origin.into(),
            message: message.into(),
        }
    }
}

/// Update check result returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_date: String,
    pub notes: String,
    pub release_url: String,
    pub download_size: Option<u64>,
}
