use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use crate::domain::*;
use super::DataSource;

const EASTMONEY_URL: &str = "https://push2.eastmoney.com/api/qt/ulist.npz";
const SEARCH_URL: &str = "https://searchapi.eastmoney.com/api/suggest/get";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct EastmoneyAdapter {
    client: Client,
}

impl EastmoneyAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_default(),
        }
    }

    /// Convert stock code to Eastmoney secid format
    /// A-shares: sh600519 → 1.600519, sz000001 → 0.000001
    fn code_to_secid(code: &str, market: &str) -> String {
        if market == "CN" {
            let prefix = if code.starts_with("6") || code.starts_with("5") || code.starts_with("9") {
                "1" // Shanghai
            } else {
                "0" // Shenzhen / Beijing
            };
            format!("{}.{}", prefix, code)
        } else {
            format!("{}.{}", code, code) // HK/US fallback
        }
    }
}

#[async_trait]
impl DataSource for EastmoneyAdapter {
    fn name(&self) -> &str {
        "eastmoney"
    }

    fn display_name(&self) -> &str {
        "东方财富"
    }

    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, String> {
        let secids: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_secid(c, market))
            .collect();
        let secids_str = secids.join(",");

        let params = [
            ("fltt", "2"),
            ("fields", "f2,f3,f4,f12,f14,f15,f16,f17,f18"),
            ("secids", &secids_str),
        ];

        #[derive(Deserialize)]
        struct RawResponse {
            data: Option<RawData>,
        }
        #[derive(Deserialize)]
        struct RawData {
            diff: Option<Vec<RawQuote>>,
        }
        #[derive(Deserialize)]
        struct RawQuote {
            #[serde(rename = "f2")]
            price: Option<f64>,
            #[serde(rename = "f3")]
            change_pct: Option<f64>,
            #[serde(rename = "f4")]
            change: Option<f64>,
            #[serde(rename = "f12")]
            code: Option<String>,
            #[serde(rename = "f14")]
            name: Option<String>,
            #[serde(rename = "f15")]
            high: Option<f64>,
            #[serde(rename = "f16")]
            low: Option<f64>,
            #[serde(rename = "f17")]
            open: Option<f64>,
            #[serde(rename = "f18")]
            volume: Option<u64>,
        }

        let resp = self
            .client
            .get(EASTMONEY_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let body: RawResponse = resp
            .json()
            .await
            .map_err(|e| format!("Parse response failed: {}", e))?;

        let quotes = body
            .data
            .and_then(|d| d.diff)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| {
                Some(Quote {
                    code: r.code?,
                    market: market.to_string(),
                    name: r.name.unwrap_or_default(),
                    price: r.price.unwrap_or(0.0),
                    change: r.change.unwrap_or(0.0),
                    change_pct: r.change_pct.unwrap_or(0.0),
                    open: r.open.unwrap_or(0.0),
                    high: r.high.unwrap_or(0.0),
                    low: r.low.unwrap_or(0.0),
                    volume: r.volume.unwrap_or(0),
                    turnover: 0.0,
                    timestamp: chrono::Utc::now().timestamp(),
                })
            })
            .collect();

        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        // SSE 000001, SZSE 399001, ChiNext 399006, STAR 50 000688
        let index_secids = "1.000001,0.399001,0.399006,1.000688";
        let params = [
            ("fltt", "2"),
            ("fields", "f2,f3,f4,f12,f14"),
            ("secids", index_secids),
        ];

        #[derive(Deserialize)]
        struct RawResponse {
            data: Option<RawData>,
        }
        #[derive(Deserialize)]
        struct RawData {
            diff: Option<Vec<RawIndex>>,
        }
        #[derive(Deserialize)]
        struct RawIndex {
            #[serde(rename = "f2")]
            price: Option<f64>,
            #[serde(rename = "f3")]
            change_pct: Option<f64>,
            #[serde(rename = "f4")]
            change: Option<f64>,
            #[serde(rename = "f12")]
            code: Option<String>,
            #[serde(rename = "f14")]
            name: Option<String>,
        }

        let resp = self
            .client
            .get(EASTMONEY_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let body: RawResponse = resp.json().await.map_err(|e| format!("Parse failed: {}", e))?;

        let indices = body
            .data
            .and_then(|d| d.diff)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| {
                Some(IndexQuote {
                    code: r.code?,
                    name: r.name.unwrap_or_default(),
                    price: r.price.unwrap_or(0.0),
                    change: r.change.unwrap_or(0.0),
                    change_pct: r.change_pct.unwrap_or(0.0),
                    volume: 0,
                    turnover: 0.0,
                })
            })
            .collect();

        Ok(indices)
    }

    async fn search(
        &self,
        keyword: &str,
        _market: &str,
    ) -> Result<Vec<StockBrief>, String> {
        #[derive(Deserialize)]
        struct RawResponse {
            #[serde(rename = "QuotationCodeTable")]
            data: Option<Vec<RawStock>>,
        }
        #[derive(Deserialize)]
        struct RawStock {
            #[serde(rename = "Code")]
            code: Option<String>,
            #[serde(rename = "Name")]
            name: Option<String>,
            #[serde(rename = "Market")]
            market_raw: Option<String>,
        }

        let resp = self
            .client
            .get(SEARCH_URL)
            .query(&[
                ("input", keyword),
                ("type", "14"),
                ("token", "D43BF722C8E33BDC906FB84A85F326E1"),
                ("count", "20"),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let body: RawResponse = resp.json().await.map_err(|e| format!("Parse failed: {}", e))?;

        let results = body
            .data
            .unwrap_or_default()
            .into_iter()
            .filter(|s| {
                // Only keep A-shares (Shanghai=1, Shenzhen=0)
                s.market_raw.as_deref() == Some("0")
                    || s.market_raw.as_deref() == Some("1")
            })
            .map(|s| StockBrief {
                code: s.code.unwrap_or_default(),
                market: "CN".to_string(),
                name: s.name.unwrap_or_default(),
            })
            .collect();

        Ok(results)
    }

    async fn health_check(&self) -> Result<bool, String> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN")
            .await
            .map(|q| !q.is_empty())
    }
}
