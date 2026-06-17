use async_trait::async_trait;
use reqwest::Client;
use crate::domain::*;
use super::DataSource;

const SINA_URL: &str = "http://hq.sinajs.cn/list=";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct SinaAdapter {
    client: Client,
}

impl SinaAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_default(),
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
        let code = code_raw[2..].to_string();

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

        let change = price - prev_close;
        let change_pct = if prev_close != 0.0 {
            (change / prev_close) * 100.0
        } else {
            0.0
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

        Some(IndexQuote {
            code,
            name,
            price,
            change,
            change_pct,
            volume,
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
    ) -> Result<Vec<Quote>, String> {
        let sina_codes: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_sina(c, market))
            .collect();
        let url = format!("{}{}", SINA_URL, sina_codes.join(","));

        eprintln!("[Sina] fetching quotes: {}", url);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina request failed: {:#}", e))?;

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Sina read body failed: {:#}", e))?;

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_sina_line)
            .collect();

        eprintln!("[Sina] parsed {} quotes", quotes.len());
        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        // Sina index codes
        let index_codes = "s_sh000001,s_sz399001,s_sz399006,s_sh000688";
        let url = format!("{}{}", SINA_URL, index_codes);

        eprintln!("[Sina] fetching indices: {}", url);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina indices request failed: {:#}", e))?;

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Sina read body failed: {:#}", e))?;

        let indices: Vec<IndexQuote> = body
            .lines()
            .filter_map(Self::parse_sina_index)
            .collect();

        eprintln!("[Sina] parsed {} indices", indices.len());
        Ok(indices)
    }

    async fn search(
        &self,
        _keyword: &str,
        _market: &str,
    ) -> Result<Vec<StockBrief>, String> {
        // Sina doesn't have a good search API; return empty and
        // the caller can fall back to Eastmoney for search
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<bool, String> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN")
            .await
            .map(|q| !q.is_empty())
    }
}
