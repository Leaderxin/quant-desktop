use async_trait::async_trait;
use reqwest::Client;
use encoding::{Encoding, DecoderTrap};
use encoding::all::GBK;
use crate::domain::*;
use super::DataSource;

const TENCENT_URL: &str = "http://qt.gtimg.cn/q=";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct TencentAdapter {
    client: Client,
}

impl TencentAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_default(),
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
        let change = price - prev_close;
        let open = fields[5].parse::<f64>().unwrap_or(0.0);
        let high = fields[33].parse::<f64>().unwrap_or(0.0);
        let low = fields[34].parse::<f64>().unwrap_or(0.0);
        let volume = fields[6].parse::<u64>().unwrap_or(0);
        let turnover = fields[37].parse::<f64>().unwrap_or(0.0);
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
            turnover_rate: None,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    fn parse_index_line(line: &str) -> Option<IndexQuote> {
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let name_raw = var_part.strip_prefix("v_")?;

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split('~').collect();

        if fields.len() < 32 { return None; }

        let name = fields[1].to_string();
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let change_pct = fields[32].parse::<f64>().unwrap_or(0.0);
        let change = fields[31].parse::<f64>().unwrap_or(0.0);
        let volume = fields[6].parse::<u64>().unwrap_or(0);
        let turnover = fields[37].parse::<f64>().unwrap_or(0.0);

        Some(IndexQuote {
            code: name_raw.to_string(),
            name,
            price,
            change,
            change_pct,
            volume: volume * 100,
            turnover,
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
        let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
            .map_err(|e| format!("Tencent GBK decode failed: {}", e))?;

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_quote_line)
            .collect();

        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        let index_codes = "s_sh000001,s_sz399001,s_sz399006,s_sh000688";
        let url = format!("{}{}", TENCENT_URL, index_codes);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Tencent indices request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
        let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
            .map_err(|e| format!("Tencent GBK decode failed: {}", e))?;

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
            let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
                .map_err(|e| format!("Tencent GBK decode failed: {}", e))?;

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

    async fn health_check(&self) -> Result<bool, String> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN")
            .await
            .map(|q| !q.is_empty())
    }
}
