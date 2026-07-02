use async_trait::async_trait;
use reqwest::Client;
use encoding_rs::GBK;
use crate::domain::*;
use crate::domain::AppError;
use super::{DataSource, INDEX_CODES, headers};

const SINA_URL: &str = "http://hq.sinajs.cn/list=";

pub struct SinaAdapter {
    client: Client,
}

impl SinaAdapter {
    pub fn new() -> Self {
        Self {
            client: super::shared_client().clone(),
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

        // We only need fields[0]–[9] (name, open, prev_close, price, high,
        // low, _, _, volume, turnover). A stricter bound would reject valid
        // responses from API variants that return fewer than 32 fields.
        if fields.len() < 10 {
            return None;
        }

        let name = fields[0].to_string();
        let open = fields[1].parse::<f64>().unwrap_or(0.0);
        let prev_close = fields[2].parse::<f64>().unwrap_or(0.0);
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let high = fields[4].parse::<f64>().unwrap_or(0.0);
        let low = fields[5].parse::<f64>().unwrap_or(0.0);
        // Sina's stock-format API returns volume in 股 (shares) and turnover in
        // 元 (yuan) — same as the index endpoint.  Do NOT apply normalize_volume /
        // normalize_turnover here; they would over-multiply (×100 and ×10000
        // respectively), producing wildly inflated values.
        // The Tencent adapter needs normalization because Tencent returns volume
        // in 手 and turnover in 万元; Sina's stock and index formats are already
        // in the normalised units that Quote expects.
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

    /// Parse index data from Sina's stock-format API (codes without s_ prefix).
    ///
    /// The stock-format endpoint returns 30+ fields matching the individual stock
    /// layout: `[0]=name [1]=open [2]=prev_close [3]=price [4]=high [5]=low
    /// [8]=volume(股) [9]=turnover(元)`.  This is more reliable than the compact
    /// index-only format (`s_` prefix, 6 fields) which returns incorrect volume
    /// **and** turnover for 创业板指 (s_sz399006).
    ///
    /// Shanghai (`sh*`) index volume is 1/100 of the correct value in every Sina
    /// format (index compact and stock alike).  Multiply by 100 so it matches
    /// Shenzhen / Tencent before the value enters the shared data pipeline.
    fn parse_sina_index(line: &str) -> Option<IndexQuote> {
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let code_raw = var_part.strip_prefix("var hq_str_")?;
        // Stock format gives "sh000001" — prepend "s_" for the canonical code
        // form used everywhere else (s_sh000001) so the frontend matches.
        let code = format!("s_{}", code_raw);
        let is_shanghai = code_raw.starts_with("sh");

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split(',').collect();

        // Stock format has 30+ fields; we need at least 10.
        if fields.len() < 10 {
            return None;
        }

        let name = fields[0].to_string();
        let prev_close = fields[2].parse::<f64>().unwrap_or(0.0);
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        // Stock-format volume is already in 股 (shares) and turnover in 元 (yuan),
        // i.e. the same normalised units that IndexQuote expects.  Do NOT apply
        // normalize_volume / normalize_turnover — they would over-multiply.
        let volume = fields[8].parse::<u64>().unwrap_or(0);
        let turnover = fields[9].parse::<f64>().unwrap_or(0.0);

        // Compute change from price vs prev_close (stock format reports raw
        // prices, not pre-computed change like the compact index format).
        let (change, change_pct) = if price > 0.0 && prev_close > 0.0 {
            let c = price - prev_close;
            let pct = (c / prev_close) * 100.0;
            (c, pct)
        } else {
            (0.0, 0.0)
        };

        // Shanghai index volume is consistently 1/100 of the correct value across
        // all Sina formats.  Correct it here so it matches Shenzhen / Tencent.
        let volume_corrected = if is_shanghai {
            volume.saturating_mul(100)
        } else {
            volume
        };

        Some(IndexQuote {
            code,
            name,
            price,
            change: (change * 100.0).round() / 100.0,
            change_pct: (change_pct * 100.0).round() / 100.0,
            volume: volume_corrected,
            turnover,
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
    ) -> Result<Vec<Quote>, AppError> {
        let sina_codes: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_sina(c, market))
            .collect();
        let url = format!("{}{}", SINA_URL, sina_codes.join(","));

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://finance.sina.com.cn",
        )
            .send()
            .await
            .map_err(|e| AppError::network("sina", format!("请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("sina", format!("HTTP {}", resp.status())));
        }

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::network("sina", format!("读取响应体失败: {:#}", e)))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_sina_line)
            .collect();

        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, AppError> {
        // Use stock-format codes (strip "s_" prefix) — the stock endpoint
        // returns reliable volume/turnover for every index including 创业板指,
        // unlike the compact index-only format (s_ prefix) which has data
        // quality issues.
        let stock_codes: Vec<&str> = INDEX_CODES
            .split(',')
            .map(|c| c.strip_prefix("s_").unwrap_or(c))
            .collect();
        let url = format!("{}{}", SINA_URL, stock_codes.join(","));

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://finance.sina.com.cn",
        )
            .send()
            .await
            .map_err(|e| AppError::network("sina", format!("指数请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("sina", format!("指数 HTTP {}", resp.status())));
        }

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::network("sina", format!("读取响应体失败: {:#}", e)))?;
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
    ) -> Result<Vec<StockBrief>, AppError> {
        // If the keyword looks like a 6-digit stock code, try direct lookup
        let trimmed = keyword.trim();
        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            let sina_code = Self::code_to_sina(trimmed, market);
            let url = format!("{}{}", SINA_URL, sina_code);
            let resp = headers::with_browser_headers(
                self.client.get(&url),
                "https://finance.sina.com.cn",
            )
                .send()
                .await
                .map_err(|e| AppError::network("sina", format!("搜索请求失败: {:#}", e)))?;
            let body_bytes = resp.bytes().await.map_err(|e| AppError::network("sina", format!("搜索读取失败: {:#}", e)))?;
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
    ) -> Result<Vec<crate::domain::MinuteData>, AppError> {
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

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://finance.sina.com.cn",
        )
            .send()
            .await
            .map_err(|e| AppError::network("sina", format!("分钟数据请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("sina", format!("分钟数据 HTTP {}", resp.status())));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| AppError::network("sina", format!("分钟数据读取失败: {:#}", e)))?;

        // Sina's response ends with a JS callback comment; strip it.
        // Guard against format changes: if the response doesn't contain valid JSON
        // array brackets, fail with a clear error instead of an opaque parse error.
        let json_str = body_text.trim_end_matches(|c| c != ']').trim();
        if json_str.is_empty() || !json_str.starts_with('[') {
            return Err(AppError::network(
                "sina",
                "分钟数据响应格式异常：未找到 JSON 数组",
            ));
        }
        let raw: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| AppError::network("sina", format!("分钟数据解析失败: {}", e)))?;


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
                    // NOTE: Sina's 5-min K-line API does not provide VWAP (avg_price).
                    // We fall back to the bar open price as a best-effort approximation
                    // so the chart has a non-zero value for rendering.
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
        end_date: Option<&str>,
        count: Option<u32>,
    ) -> Result<Vec<crate::domain::KLineData>, AppError> {
        let symbol = if code.starts_with("s_") {
            code[2..].to_string()
        } else {
            Self::code_to_sina(code, market)
        };

        // Sina only supports daily K-line; reject minute/weekly/monthly.
        // Minute data should use fetch_minute_data instead.
        if period != "daily" {
            return Err(AppError::Unsupported("新浪数据源不支持周K/月K/分钟K线，请切换到腾讯数据源查看".into()));
        }

        if end_date.is_some() || count.is_some() {
            log::warn!("Sina adapter does not support end_date/count pagination; ignoring");
        }

        let scale = "240";

        let url = format!(
            "http://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData?symbol={}&scale={}&ma=no&datalen=600",
            symbol, scale
        );

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://finance.sina.com.cn",
        )
            .send()
            .await
            .map_err(|e| AppError::network("sina", format!("K线请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("sina", format!("K线 HTTP {}", resp.status())));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| AppError::network("sina", format!("K线读取失败: {:#}", e)))?;

        if body_text.is_empty() || body_text == "null" {
            log::warn!("Sina kline empty body for code={}", symbol);
            return Ok(vec![]);
        }

        let json_str = body_text.trim_end_matches(|c| c != ']').trim();
        // Guard against format changes producing empty or non-array responses
        if json_str.is_empty() || !json_str.starts_with('[') {
            return Err(AppError::network(
                "sina",
                "K线响应格式异常：未找到 JSON 数组",
            ));
        }
        let raw: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| AppError::network("sina", format!("K线解析失败: {}", e)))?;

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
    ) -> Result<crate::domain::Depth, AppError> {
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

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://gu.qq.com",
        )
            .send()
            .await
            .map_err(|e| AppError::network("sina", format!("深度数据(Tencent)请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("sina", format!("深度数据 HTTP {}", resp.status())));
        }

        let body_bytes = resp.bytes().await.map_err(|e| AppError::network("sina", format!("深度数据读取失败: {:#}", e)))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for line in body.lines() {
            if let Some(eq_pos) = line.find('=') {
                // Use ?-style fallback instead of unwrap_or(0) to avoid
                // panicking on malformed responses without quoted data.
                let q_start = match line[eq_pos + 1..].find('"') {
                    Some(p) => p + eq_pos + 2,
                    None => continue, // skip lines without quoted data
                };
                let qe = line[q_start..].find('"').unwrap_or(0);
                let data = &line[q_start..q_start + qe];
                let fields: Vec<&str> = data.split('~').collect();

                if fields.len() >= 29 {
                    for i in 0..5 {
                        let pi = 9 + i * 2;
                        if let (Ok(price), Ok(vol)) = (
                            fields[pi].parse::<f64>(),
                            fields[pi + 1].parse::<u64>(),
                        ) {
                            if price > 0.0 && vol > 0 {
                                bids.push(Level { price, volume: super::normalize_volume(vol) });
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
                                asks.push(Level { price, volume: super::normalize_volume(vol) });
                            }
                        }
                    }
                }
                break;
            }
        }

        Ok(crate::domain::Depth { code: code.to_string(), bids, asks })
    }

    async fn health_check(&self) -> Result<bool, AppError> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN")
            .await
            .map(|q| !q.is_empty())
    }
}
