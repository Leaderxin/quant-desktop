use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use encoding_rs::GBK;
use crate::domain::*;
use super::{DataSource, USER_AGENT, INDEX_CODES};

const TENCENT_URL: &str = "http://qt.gtimg.cn/q=";

pub struct TencentAdapter {
    client: Client,
}

impl TencentAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to build reqwest Client — TLS backend may be missing"),
        }
    }

    fn code_to_tencent(code: &str, market: &str) -> String {
        if market == "CN" {
            if code.starts_with("6") || code.starts_with("5") || code.starts_with("9") {
                format!("sh{}", code)
            } else {
                format!("sz{}", code)
            }
        } else {
            code.to_string()
        }
    }

    fn parse_quote_line(line: &str) -> Option<Quote> {
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let code_raw = var_part.strip_prefix("v_")?;
        let market = if code_raw.starts_with("sh") { "CN" } else if code_raw.starts_with("sz") { "CN" } else { "CN" };
        let code = if code_raw.len() >= 2 { code_raw[2..].to_string() } else { code_raw.to_string() };

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split('~').collect();

        if fields.len() < 38 { return None; }

        let name = fields[1].to_string();
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let prev_close = fields[4].parse::<f64>().unwrap_or(0.0);
        let change_pct = fields[32].parse::<f64>().unwrap_or(0.0);
        // Guard against bogus change when market closed (price=0, prev_close>0)
        let change = if price > 0.0 && prev_close > 0.0 {
            price - prev_close
        } else {
            0.0
        };
        let open = fields[5].parse::<f64>().unwrap_or(0.0);
        let high = fields[33].parse::<f64>().unwrap_or(0.0);
        let low = fields[34].parse::<f64>().unwrap_or(0.0);
        let volume = fields[6].parse::<u64>().unwrap_or(0);
        let turnover = fields[37].parse::<f64>().unwrap_or(0.0);
        let turnover_rate = fields.get(38).and_then(|s| s.parse::<f64>().ok());
        // Tencent volume is in "手" (100 shares), convert to shares
        let volume_shares = volume * 100;

        Some(Quote {
            code,
            market: market.to_string(),
            name,
            price,
            change: (change * 100.0).round() / 100.0,
            change_pct,
            open,
            high,
            low,
            volume: volume_shares,
            turnover: (turnover * 10000.0 * 100.0).round() / 100.0,
            turnover_rate,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    fn parse_index_line(line: &str) -> Option<IndexQuote> {
        // Tencent index format: v_sh000001="market~name~code~price~change~change_pct~volume~turnover~..."
        // Fields are separated by '~', 11+ fields for indices.
        //   [0]=market  [1]=name  [2]=code  [3]=price  [4]=change
        //   [5]=change%  [6]=volume(手)  [7]=turnover(万元)
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let name_raw = var_part.strip_prefix("v_")?;

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split('~').collect();

        if fields.len() < 6 { return None; }

        let name = fields[1].to_string();
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let change = fields[4].parse::<f64>().unwrap_or(0.0);
        let change_pct = fields[5].parse::<f64>().unwrap_or(0.0);
        let volume = fields[6].parse::<u64>().unwrap_or(0);
        // Tencent index format (11+ fields):
        //   [0]=market [1]=name [2]=code [3]=price [4]=change
        //   [5]=change% [6]=volume(手) [7]=turnover(万元) [8..]=...
        let turnover = if fields.len() > 7 {
            fields[7].parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };

        Some(IndexQuote {
            code: name_raw.to_string(),
            name,
            price,
            change,
            change_pct,
            volume: volume * 100, // 手 → 股
            turnover: turnover * 10000.0, // 万元 → 元
        })
    }
}

#[async_trait]
impl DataSource for TencentAdapter {
    fn name(&self) -> &str { "tencent" }

    fn display_name(&self) -> &str { "腾讯证券" }

    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, String> {
        let tenc_codes: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_tencent(c, market))
            .collect();
        let url = format!("{}{}", TENCENT_URL, tenc_codes.join(","));

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Tencent request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_quote_line)
            .collect();
        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        let index_codes = INDEX_CODES;
        let url = format!("{}{}", TENCENT_URL, index_codes);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Tencent indices request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let indices: Vec<IndexQuote> = body
            .lines()
            .filter_map(Self::parse_index_line)
            .collect();
        Ok(indices)
    }

    async fn search(
        &self,
        keyword: &str,
        market: &str,
    ) -> Result<Vec<StockBrief>, String> {
        let trimmed = keyword.trim();
        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            let tc_code = Self::code_to_tencent(trimmed, market);
            let url = format!("{}{}", TENCENT_URL, tc_code);
            let resp = self
                .client
                .get(&url)
                .header("Referer", "https://gu.qq.com")
                .send()
                .await
                .map_err(|e| format!("Tencent search request failed: {:#}", e))?;
            let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
            let (body, _, _) = GBK.decode(&body_bytes);

            for line in body.lines() {
                if let Some(quote) = Self::parse_quote_line(line) {
                    if !quote.name.is_empty() {
                        return Ok(vec![StockBrief {
                            code: quote.code,
                            market: quote.market,
                            name: quote.name,
                        }]);
                    }
                }
            }
        }
        Ok(vec![])
    }

    async fn fetch_minute_data(
        &self,
        code: &str,
        market: &str,
    ) -> Result<Vec<crate::domain::MinuteData>, String> {
        let tc_code = if code.starts_with("s_") {
            // Index codes already have exchange prefix: "s_sh000001" → "sh000001"
            code[2..].to_string()
        } else {
            Self::code_to_tencent(code, market)
        };
        // Use 5-min K-line endpoint — same as Sina, returns multi-day data
        let url = format!("http://ifzq.gtimg.cn/appstock/app/kline/mkline?param={},m5,,240", tc_code);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("Tencent kline request failed: {:#}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Tencent kline parse failed: {}", e))?;

        // Format: [["202606180935","open","close","high","low","volume",{},"rate"], ...]
        let lines = body
            .pointer("/data")
            .and_then(|d| d.as_object())
            .and_then(|obj| obj.values().next())
            .and_then(|stock| stock.get("m5"))
            .and_then(|arr| arr.as_array())
            .cloned()
            .unwrap_or_default();

        let data: Vec<crate::domain::MinuteData> = lines
            .iter()
            .filter_map(|pt| {
                let arr = pt.as_array()?;
                if arr.len() < 6 { return None; }
                let time_raw = arr[0].as_str()?;
                // "202606180935" → "09:35"
                let time = if time_raw.len() >= 12 {
                    format!("{}:{}", &time_raw[8..10], &time_raw[10..12])
                } else {
                    time_raw.to_string()
                };
                let open: f64 = arr[1].as_str()?.parse().ok()?;
                let close: f64 = arr[2].as_str()?.parse().ok()?;
                let high: f64 = arr[3].as_str()?.parse().unwrap_or(close);
                let low: f64 = arr[4].as_str()?.parse().unwrap_or(close);
                // Tencent volume is in 手, convert to 股
                let volume: u64 = arr[5].as_str()?.parse().unwrap_or(0);
                Some(crate::domain::MinuteData {
                    time,
                    price: close,
                    open,
                    high,
                    low,
                    volume: volume * 100,
                    avg_price: open,
                })
            })
            .collect();

        Ok(data)
    }

    async fn fetch_depth(
        &self,
        code: &str,
        market: &str,
    ) -> Result<crate::domain::Depth, String> {
        use crate::domain::Level;

        let tc_code = Self::code_to_tencent(code, market);
        let url = format!("{}{}", TENCENT_URL, tc_code);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Tencent depth request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for line in body.lines() {
            if let Some(eq_pos) = line.find('=') {
                let qs = line[eq_pos + 1..].find('"').unwrap_or(0) + eq_pos + 2;
                let qe = line[qs..].find('"').unwrap_or(0);
                let data = &line[qs..qs + qe];
                let fields: Vec<&str> = data.split('~').collect();

                if fields.len() >= 29 {
                    // Bids: fields 9-18 (price,vol alternating)
                    for i in 0..5 {
                        let pi = 9 + i * 2;
                        let vi = pi + 1;
                        if let (Ok(price), Ok(vol)) = (
                            fields[pi].parse::<f64>(),
                            fields[vi].parse::<u64>(),
                        ) {
                            if price > 0.0 && vol > 0 {
                                bids.push(Level { price, volume: vol * 100 }); // 手→股
                            }
                        }
                    }
                    // Asks: fields 19-28
                    for i in 0..5 {
                        let pi = 19 + i * 2;
                        let vi = pi + 1;
                        if let (Ok(price), Ok(vol)) = (
                            fields[pi].parse::<f64>(),
                            fields[vi].parse::<u64>(),
                        ) {
                            if price > 0.0 && vol > 0 {
                                asks.push(Level { price, volume: vol * 100 });
                            }
                        }
                    }
                }
                break; // Only first line matters
            }
        }

        Ok(crate::domain::Depth { code: code.to_string(), bids, asks })
    }

    async fn health_check(&self) -> Result<bool, String> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN")
            .await
            .map(|q| !q.is_empty())
    }
}
