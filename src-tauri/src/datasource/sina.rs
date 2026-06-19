use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use encoding_rs::GBK;
use crate::domain::*;
use super::{DataSource, USER_AGENT, INDEX_CODES};

const SINA_URL: &str = "http://hq.sinajs.cn/list=";

pub struct SinaAdapter {
    client: Client,
}

impl SinaAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to build reqwest Client — TLS backend may be missing"),
        }
    }

    /// Convert code to Sina format: "sh600519" or "sz000001"
    fn code_to_sina(code: &str, market: &str) -> String {
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

    /// Parse Sina quote line: var hq_str_sh600519="name,open,prev_close,price,..."
    fn parse_sina_line(line: &str) -> Option<Quote> {
        // Extract code from "var hq_str_shXXXXXX"
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let code_raw = var_part.strip_prefix("var hq_str_")?;
        let market = if code_raw.len() >= 2 {
            match &code_raw[..2] {
                "sh" => "CN",
                "sz" => "CN",
                _ => "CN",
            }
        } else {
            "CN"
        };
        let code = if code_raw.len() >= 3 { code_raw[2..].to_string() } else { code_raw.to_string() };

        // Extract data between quotes
        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split(',').collect();

        if fields.len() < 32 {
            return None;
        }

        let name = fields[0].to_string();
        let open = fields[1].parse::<f64>().unwrap_or(0.0);
        let prev_close = fields[2].parse::<f64>().unwrap_or(0.0);
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let high = fields[4].parse::<f64>().unwrap_or(0.0);
        let low = fields[5].parse::<f64>().unwrap_or(0.0);
        let volume = fields[8].parse::<u64>().unwrap_or(0);
        let turnover = fields[9].parse::<f64>().unwrap_or(0.0);
        // Sina's hq.sinajs.cn/list= endpoint only returns basic quote +
        // 5-level depth (~33 fields). Turnover rate is NOT included.
        let turnover_rate: Option<f64> = None;

        // When market is closed, Sina returns price=0.0 while prev_close
        // retains the last close. Computing change from that yields bogus
        // -100% values. Guard: if price is 0, the stock isn't trading.
        let (change, change_pct) = if price > 0.0 && prev_close > 0.0 {
            let c = price - prev_close;
            let pct = (c / prev_close) * 100.0;
            (c, pct)
        } else {
            (0.0, 0.0)
        };

        Some(Quote {
            code,
            market: market.to_string(),
            name,
            price,
            change: (change * 100.0).round() / 100.0,
            change_pct: (change_pct * 100.0).round() / 100.0,
            open,
            high,
            low,
            volume,
            turnover,
            turnover_rate,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    /// Parse Sina index line
    fn parse_sina_index(line: &str) -> Option<IndexQuote> {
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let code_raw = var_part.strip_prefix("var hq_str_")?;
        let code = code_raw.to_string();

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split(',').collect();

        if fields.len() < 6 {
            return None;
        }

        let name = fields[0].to_string();
        let price = fields[1].parse::<f64>().unwrap_or(0.0);
        let change = fields[2].parse::<f64>().unwrap_or(0.0);
        let change_pct = fields[3].parse::<f64>().unwrap_or(0.0);
        let volume = fields[4].parse::<u64>().unwrap_or(0);
        let turnover = fields[5].parse::<f64>().unwrap_or(0.0);
        // Sina index API returns volume in 手 and turnover in 万元.
        // Normalize: volume 手→股 (×100), turnover 万元→元 (×10000),
        // matching Tencent adapter and stock Quote conventions.
        let volume_shares = volume * 100;
        let turnover_yuan = turnover * 10000.0;

        Some(IndexQuote {
            code,
            name,
            price,
            change,
            change_pct,
            volume: volume_shares,
            turnover: turnover_yuan,
        })
    }
}

#[async_trait]
impl DataSource for SinaAdapter {
    fn name(&self) -> &str {
        "sina"
    }

    fn display_name(&self) -> &str {
        "新浪财经"
    }

    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, String> {
        let sina_codes: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_sina(c, market))
            .collect();
        let url = format!("{}{}", SINA_URL, sina_codes.join(","));

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina request failed: {:#}", e))?;

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Sina read body failed: {:#}", e))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_sina_line)
            .collect();

        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        // Sina index codes
        let index_codes = INDEX_CODES;
        let url = format!("{}{}", SINA_URL, index_codes);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina indices request failed: {:#}", e))?;

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Sina read body failed: {:#}", e))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let indices: Vec<IndexQuote> = body
            .lines()
            .filter_map(Self::parse_sina_index)
            .collect();

        Ok(indices)
    }

    async fn search(
        &self,
        keyword: &str,
        market: &str,
    ) -> Result<Vec<StockBrief>, String> {
        // If the keyword looks like a 6-digit stock code, try direct lookup
        let trimmed = keyword.trim();
        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            let sina_code = Self::code_to_sina(trimmed, market);
            let url = format!("{}{}", SINA_URL, sina_code);
            let resp = self
                .client
                .get(&url)
                .header("Referer", "https://finance.sina.com.cn")
                .send()
                .await
                .map_err(|e| format!("Sina search request failed: {:#}", e))?;
            let body_bytes = resp.bytes().await.map_err(|e| format!("Sina read failed: {:#}", e))?;
            let (body, _, _) = GBK.decode(&body_bytes);

            // Parse the response to extract name
            for line in body.lines() {
                if let Some(quote) = Self::parse_sina_line(line) {
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
        let symbol = if code.starts_with("s_") {
            // Index codes already have exchange prefix: "s_sh000001" → "sh000001"
            code[2..].to_string()
        } else {
            Self::code_to_sina(code, market)
        };
        let url = format!(
            "http://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData?symbol={}&scale=5&datalen=240",
            symbol
        );

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina minute request failed: {:#}", e))?;

        let body_text = resp
            .text()
            .await
            .map_err(|e| format!("Sina minute read failed: {:#}", e))?;

        // Sina's response ends with a JS callback comment; strip it
        let json_str = body_text.trim_end_matches(|c| c != ']').trim();
        let raw: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| format!("Sina minute parse failed: {} — body: {}", e, &body_text[..body_text.len().min(100)]))?;


        let data: Vec<crate::domain::MinuteData> = raw
            .iter()
            .filter_map(|pt| {
                let time_raw = pt.get("day")?.as_str()?;
                let time = if time_raw.len() >= 16 {
                    time_raw[11..16].to_string()
                } else {
                    time_raw.to_string()
                };
                let open = pt.get("open")?.as_str()?.parse().ok()?;
                let close = pt.get("close")?.as_str()?.parse().ok()?;
                let high = pt.get("high")?.as_str()?.parse().unwrap_or(close);
                let low = pt.get("low")?.as_str()?.parse().unwrap_or(close);
                Some(crate::domain::MinuteData {
                    time,
                    price: close,
                    open,
                    high,
                    low,
                    volume: pt.get("volume")?.as_str()?.parse().unwrap_or(0),
                    avg_price: open,
                })
            })
            .collect();

        Ok(data)
    }

    async fn fetch_kline(
        &self,
        code: &str,
        market: &str,
        period: &str,
    ) -> Result<Vec<crate::domain::KLineData>, String> {
        let symbol = if code.starts_with("s_") {
            code[2..].to_string()
        } else {
            Self::code_to_sina(code, market)
        };

        // Sina only supports daily K-line; weekly/monthly are not available
        if period != "daily" && period != "minute" {
            return Err("新浪数据源不支持周K/月K，请切换到腾讯数据源查看".into());
        }
        let scale = "240";

        let url = format!(
            "http://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData?symbol={}&scale={}&ma=no&datalen=200",
            symbol, scale
        );

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina kline request failed: {:#}", e))?;

        let body_text = resp
            .text()
            .await
            .map_err(|e| format!("Sina kline read failed: {:#}", e))?;

        if body_text.is_empty() || body_text == "null" {
            log::warn!("Sina kline empty body for code={}", symbol);
            return Ok(vec![]);
        }

        let json_str = body_text.trim_end_matches(|c| c != ']').trim();
        let raw: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| format!("Sina kline parse failed: {} — body: {}", e, &body_text[..body_text.len().min(100)]))?;

        if raw.is_empty() {
            log::warn!("Sina kline empty for code={} period={}", symbol, scale);
        }

        let data: Vec<crate::domain::KLineData> = raw
            .iter()
            .filter_map(|pt| {
                let date = pt.get("day")?.as_str()?.to_string();
                let open: f64 = pt.get("open")?.as_str()?.parse().ok()?;
                let high: f64 = pt.get("high")?.as_str()?.parse().ok()?;
                let low: f64 = pt.get("low")?.as_str()?.parse().ok()?;
                let close: f64 = pt.get("close")?.as_str()?.parse().ok()?;
                let volume: u64 = pt.get("volume")?.as_str()?.parse().unwrap_or(0u64);
                // Sina doesn't provide turnover in K-line data, default to 0
                let turnover: f64 = 0.0;
                Some(crate::domain::KLineData {
                    date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    turnover,
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
        // Sina's buy_/sell_ depth API is dead — fall back to Tencent's
        // quote endpoint which embeds 5-level depth in fields 9-28.
        use crate::domain::Level;

        let tc_code = if market == "CN" {
            if code.starts_with("6") || code.starts_with("5") || code.starts_with("9") {
                format!("sh{}", code)
            } else {
                format!("sz{}", code)
            }
        } else {
            code.to_string()
        };
        let url = format!("http://qt.gtimg.cn/q={}", tc_code);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Sina depth (via Tencent) request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Read failed: {:#}", e))?;
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
                    for i in 0..5 {
                        let pi = 9 + i * 2;
                        if let (Ok(price), Ok(vol)) = (
                            fields[pi].parse::<f64>(),
                            fields[pi + 1].parse::<u64>(),
                        ) {
                            if price > 0.0 && vol > 0 {
                                bids.push(Level { price, volume: vol * 100 });
                            }
                        }
                    }
                    for i in 0..5 {
                        let pi = 19 + i * 2;
                        if let (Ok(price), Ok(vol)) = (
                            fields[pi].parse::<f64>(),
                            fields[pi + 1].parse::<u64>(),
                        ) {
                            if price > 0.0 && vol > 0 {
                                asks.push(Level { price, volume: vol * 100 });
                            }
                        }
                    }
                }
                break;
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
