use async_trait::async_trait;
use reqwest::Client;
use encoding_rs::GBK;
use crate::domain::*;
use crate::domain::AppError;
use super::{DataSource, INDEX_CODES, headers};

const TENCENT_URL: &str = "http://qt.gtimg.cn/q=";

/// 解析腾讯 K 线数组为 KLineData（日 K / 分钟 K 共用）。
/// 腾讯 K 线数组格式：`[date, open, close, high, low, volume(手), turnover_opt?, ...]`
/// - 日 K：`["2026-06-19", "开", "收", "高", "低", "量(手)", "额", ...]`（≥6 元素，turnover 在 index 6）
/// - 分钟 K：`["202606180935", "开", "收", "高", "低", "量(手)", "{}"|金额|缺失, 金额?|缺失]`
///   分钟K的 turnover 位置不固定(腾讯接口版本差异)，扫描 index 6/7 取首个可解析数值。
///   index 6 既可能是 JSON 空对象 {} (由 as_str 返回 None 自动跳过), 也可能是金额字符串或缺失;
///   index 7 同样可能是金额字符串或缺失。
///   注: (6..=7) 扫描基于**实际观察到的**腾讯 m1/m5/m15/m30/m60 响应格式(已知 index 6/7 仅承载 turnover 或空对象),
///   非接口文档; 若上游格式演进(index 6/7 改为均价等其它数值字段)需重新核实, 避免误取非金额数值写入 turnover。
///   **重要**：腾讯 mkline API 分钟K线时间戳采用同花顺惯例：
///   - 1 分钟 K：返回周期**开始**时间（如 `202608070930` → 09:30）
///   - 5/15/30/60 分钟 K：返回周期**结束**时间（如 30 分钟 `202608071000` → 10:00）
///   两种格式均为行业惯例，直接透传即可，无需时间偏移修正。
/// `span_minutes` 控制：Some(span) 为分钟 K（date 格式转换 + 时间偏移修正 + turnover 扫描 6-7），
/// None 为日/周/月 K（date 直接使用为 `t.to_string()` + turnover 仅 index 6）。
fn parse_kline_bar(arr: &[serde_json::Value], span_minutes: Option<u32>) -> Option<crate::domain::KLineData> {
    // 必须字段：date, open, close, high, low, volume（索引 0-5）
    if arr.len() < 6 {
        return None;
    }
    let t = arr[0].as_str()?;
    let date = if let Some(_span) = span_minutes {
        if t.len() >= 12 {
            // 腾讯 mkline 返回的是周期结束时间，需减去 span 得到周期开始时间
            let naive = chrono::NaiveDateTime::parse_from_str(t, "%Y%m%d%H%M").ok()?;
            // 腾讯 mkline 分钟 K 时间戳直接透传（见上方注释）
            let corrected = naive;
            corrected.format("%Y-%m-%d %H:%M").to_string()
        } else {
            // 时间戳格式异常时降级为原始字符串（防御性处理）
            t.to_string()
        }
    } else {
        t.to_string()
    };
    let open: f64 = arr[1].as_str()?.parse().ok()?;
    let close: f64 = arr[2].as_str()?.parse().ok()?;
    let high: f64 = arr[3].as_str()?.parse().ok()?;
    let low: f64 = arr[4].as_str()?.parse().ok()?;
    let volume_hands: f64 = arr[5].as_str()?.parse().unwrap_or(0.0);
    let volume: u64 = (volume_hands * super::VOLUME_HANDS_TO_SHARES as f64) as u64;
    // 分钟K 的 turnover 位置随腾讯接口版本变化: index 6 可能是 JSON 空对象 {}、金额字符串或缺失,
    // index 7 也可能是金额字符串或缺失。因此扫描 index 6 与 7, 取首个 as_str 非空且可解析为 f64 的字符串。
    // (扫描范围基于实际观察到的 m1/m5/m15/m30/m60 响应, 非接口文档, 详见函数上方注释。)
    let turnover: f64 = if span_minutes.is_some() {
        (6..=7)
            .filter_map(|i| arr.get(i))
            .filter_map(|v| v.as_str())
            .filter_map(|s| s.parse::<f64>().ok())
            .next()
            .unwrap_or(0.0)
    } else {
        arr.get(6)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    };
    Some(crate::domain::KLineData {
        date,
        open,
        high,
        low,
        close,
        volume,
        turnover,
    })
}

pub struct TencentAdapter {
    client: Client,
}

impl TencentAdapter {
    pub fn new() -> Self {
        Self {
            client: super::shared_client().clone(),
        }
    }

    fn code_to_tencent(code: &str, market: &str) -> String {
        if market == "CN" {
            // Already has exchange prefix (e.g. "sh000001" from index codes)
            if code.starts_with("sh") || code.starts_with("sz") {
                return code.to_string();
            }
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
        // Preserve the full symbol (sh/sz + code) so ambiguous codes that map to
        // both an index and a stock (e.g. 000852) remain distinguishable.
        let code = code_raw.to_string();

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
        // Tencent volume is in 手, turnover in 万元 — normalize to 股/元
        let volume_shares = super::normalize_volume(volume);

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
            turnover: (super::normalize_turnover(turnover) * 100.0).round() / 100.0,
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
            volume: super::normalize_volume(volume),
            turnover: super::normalize_turnover(turnover),
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
    ) -> Result<Vec<Quote>, AppError> {
        let tenc_codes: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_tencent(c, market))
            .collect();
        let url = format!("{}{}", TENCENT_URL, tenc_codes.join(","));

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://gu.qq.com",
        )
            .send()
            .await
            .map_err(|e| AppError::network("tencent", format!("请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("tencent", format!("HTTP {}", resp.status())));
        }

        let body_bytes = resp.bytes().await.map_err(|e| AppError::network("tencent", format!("读取响应失败: {:#}", e)))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_quote_line)
            .collect();
        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, AppError> {
        let index_codes = INDEX_CODES;
        let url = format!("{}{}", TENCENT_URL, index_codes);

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://gu.qq.com",
        )
            .send()
            .await
            .map_err(|e| AppError::network("tencent", format!("指数请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("tencent", format!("指数 HTTP {}", resp.status())));
        }

        let body_bytes = resp.bytes().await.map_err(|e| AppError::network("tencent", format!("读取响应失败: {:#}", e)))?;
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
    ) -> Result<Vec<StockBrief>, AppError> {
        let trimmed = keyword.trim();
        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            let tc_code = Self::code_to_tencent(trimmed, market);
            let url = format!("{}{}", TENCENT_URL, tc_code);
            let resp = headers::with_browser_headers(
                self.client.get(&url),
                "https://gu.qq.com",
            )
                .send()
                .await
                .map_err(|e| AppError::network("tencent", format!("搜索请求失败: {:#}", e)))?;
            let body_bytes = resp.bytes().await.map_err(|e| AppError::network("tencent", format!("读取响应失败: {:#}", e)))?;
            let (body, _, _) = GBK.decode(&body_bytes);

            for line in body.lines() {
                if let Some(quote) = Self::parse_quote_line(line) {
                    if !quote.name.is_empty() {
                        let category = super::cn_category(&quote.code).to_string();
                        return Ok(vec![StockBrief {
                            code: quote.code,
                            market: quote.market,
                            name: quote.name,
                            category,
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
        let tc_code = if code.starts_with("s_") {
            // Index codes already have exchange prefix: "s_sh000001" → "sh000001"
            code[2..].to_string()
        } else {
            Self::code_to_tencent(code, market)
        };
        // Use 1-min K-line endpoint — gives finer-grained intraday data (240 bars
        // covers exactly one trading day: 9:30-11:30 + 13:00-15:00 = 240 min).
        let url = format!("http://ifzq.gtimg.cn/appstock/app/kline/mkline?param={},m1,,242", tc_code);

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://gu.qq.com",
        )
            .send()
            .await
            .map_err(|e| AppError::network("tencent", format!("K线请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("tencent", format!("K线 HTTP {}", resp.status())));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::network("tencent", format!("K线解析失败: {}", e)))?;

        // Format: [["202606180935","open","close","high","low","volume",{},"rate"], ...]
        let lines = body
            .pointer("/data")
            .and_then(|d| d.as_object())
            .and_then(|obj| obj.values().next())
            .and_then(|stock| stock.get("m1"))
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
                let volume_hands: f64 = arr[5].as_str()?.parse().unwrap_or(0.0);
                let volume: u64 = (volume_hands * super::VOLUME_HANDS_TO_SHARES as f64) as u64;
                Some(crate::domain::MinuteData {
                    time,
                    price: close,
                    open,
                    high,
                    low,
                    volume,
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
        let tc_code = if code.starts_with("s_") {
            code[2..].to_string()
        } else {
            Self::code_to_tencent(code, market)
        };

        // ── 分钟 K 线：走 mkline 端点 ──
        // fqkline 不支持分钟周期（返回 "bad params"），故分钟 K 单独走 mkline。
        // 复权取舍：mkline 端点不支持复权参数，故分钟K为【不复权原始价】；
        // 而日/周/月走 fqkline 携带 qfq（【前复权】）。同一股票的分钟K与日K价格基准
        // 在除权日会出现跳变（分钟K跳高/原价，日K平滑），这是依赖腾讯接口能力的设计取舍。
        // 若前端需要一致性，可将日/周/月也改为不复权（fqkline 末参改空），代价为历史价位回退原始价。
        // URL 格式: param={code},m{span},{start},{end_YYYYMMDDHHMM},{count}
        // end_date 去横线以匹配 mkline 响应时间戳格式（YYYYMMDDHHMM）
        if let Some(span) = super::minute_span(period) {
            let end_date_str = end_date.map(|d| d.replace('-', "")).unwrap_or_default();
            let cnt = count.unwrap_or(320);
            let url = format!(
                "http://ifzq.gtimg.cn/appstock/app/kline/mkline?param={},m{},{},{},{}",
                tc_code, span, "", end_date_str, cnt
            );
            log::debug!("Tencent mkline URL: {}", url);

            let resp = headers::with_browser_headers(self.client.get(&url), "https://gu.qq.com")
                .send()
                .await
                .map_err(|e| AppError::network("tencent", format!("分钟K线请求失败: {}", e)))?;
            if !resp.status().is_success() {
                return Err(AppError::network("tencent", format!("分钟K线 HTTP {}", resp.status())));
            }
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| AppError::network("tencent", format!("分钟K线解析失败: {}", e)))?;
            let lines: &[serde_json::Value] = body
                .pointer("/data")
                .and_then(|d| d.as_object())
                .and_then(|obj| obj.values().next())
                .and_then(|stock| stock.get(format!("m{}", span).as_str()))
                .and_then(|arr| arr.as_array())
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            if lines.is_empty() {
                log::warn!("Tencent minute kline empty for code={} span={}", tc_code, span);
            }
            return Ok(lines.iter()
                .filter_map(|pt| parse_kline_bar(pt.as_array()?, Some(span)))
                .collect());
        }

        // ── 日/周/月 K：走 fqkline 端点 ──
        // fqkline 支持完整的 end_date 翻页，历史数据可无限向左拖动
        let period_param = match period {
            "weekly" => "week",
            "monthly" => "month",
            _ => "day",
        };

        let cnt = count.unwrap_or(200);
        let end_date_str = end_date.unwrap_or("");

        let url = format!(
            "http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={},{},,{},{},qfq",
            tc_code, period_param, end_date_str, cnt
        );

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://gu.qq.com",
        )
            .send()
            .await
            .map_err(|e| AppError::network("tencent", format!("K线请求失败: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::network("tencent", format!("K线解析失败: {}", e)))?;

        let stock_data = body
            .pointer("/data")
            .and_then(|d| d.as_object())
            .and_then(|obj| obj.values().next());

        let klines = stock_data
            .and_then(|stock| {
                stock.get(period_param)
                    .or_else(|| stock.get(&format!("qfq{}", period_param)))
            })
            .and_then(|arr| arr.as_array())
            .cloned()
            .unwrap_or_default();

        if klines.is_empty() {
            log::warn!("Tencent kline empty for code={} period={}", tc_code, period_param);
        }

        let data: Vec<crate::domain::KLineData> = klines
            .iter()
            .filter_map(|pt| parse_kline_bar(pt.as_array()?, None))
            .collect();

        Ok(data)
    }

    async fn fetch_depth(
        &self,
        code: &str,
        market: &str,
    ) -> Result<crate::domain::Depth, AppError> {
        use crate::domain::Level;

        let tc_code = Self::code_to_tencent(code, market);
        let url = format!("{}{}", TENCENT_URL, tc_code);

        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://gu.qq.com",
        )
            .send()
            .await
            .map_err(|e| AppError::network("tencent", format!("深度数据请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("tencent", format!("深度数据 HTTP {}", resp.status())));
        }

        let body_bytes = resp.bytes().await.map_err(|e| AppError::network("tencent", format!("读取响应失败: {:#}", e)))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for line in body.lines() {
            if let Some(eq_pos) = line.find('=') {
                // Use safe fallback instead of unwrap_or(0) to avoid
                // panicking on malformed responses without quoted data.
                let q_start = match line[eq_pos + 1..].find('"') {
                    Some(p) => p + eq_pos + 2,
                    None => continue, // skip lines without quoted data
                };
                let qe = line[q_start..].find('"').unwrap_or(0);
                let data = &line[q_start..q_start + qe];
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
                                bids.push(Level { price, volume: super::normalize_volume(vol) });
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
                                asks.push(Level { price, volume: super::normalize_volume(vol) });
                            }
                        }
                    }
                }
                break; // Only first line matters
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quote_line_preserves_exchange_prefix() {
        // Build a minimal 39-field Tencent quote line for sz000852 (石化机械).
        let mut fields: Vec<&str> = vec![
            "51",       // 0  market
            "石化机械", // 1  name
            "000852",   // 2  code
            "6.06",     // 3  price
            "5.51",     // 4  prev_close
            "5.82",     // 5  open
            "581842",   // 6  volume(手)
        ];
        // 7..=31 filler (unused fields)
        for _ in 0..25 {
            fields.push("0");
        }
        fields.push("9.98");  // 32 change_pct
        fields.push("6.06");  // 33 high
        fields.push("5.68");  // 34 low
        fields.push("0");     // 35
        fields.push("0");     // 36
        fields.push("34829"); // 37 turnover(万元)
        fields.push("1.84");  // 38 turnover_rate
        let line = format!("v_sz000852=\"{}\"", fields.join("~"));

        let q = TencentAdapter::parse_quote_line(&line).unwrap();
        assert_eq!(q.code, "sz000852");
        assert_eq!(q.name, "石化机械");
        assert_eq!(q.price, 6.06);
    }

    #[test]
    fn parses_kline_bar_minute() {
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500",{},"1530000"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), Some(1)).unwrap();
        assert_eq!(out.date, "2026-06-18 09:35");
        assert_eq!(out.open, 10.00);
        assert_eq!(out.close, 10.20);
        assert_eq!(out.high, 10.30);
        assert_eq!(out.low, 9.90);
        assert_eq!(out.volume, 150000); // 1500 手 ×100
        assert_eq!(out.turnover, 1530000.0);
    }

    #[test]
    fn parses_kline_bar_minute_turnover_at_index6() {
        // 另一种分钟K格式: index 6 直接是金额字符串, 无空对象 {}
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500","1530000"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), Some(1)).unwrap();
        assert_eq!(out.date, "2026-06-18 09:35");
        assert_eq!(out.volume, 150000); // 1500 手 ×100
        assert_eq!(out.turnover, 1530000.0);
    }

    #[test]
    fn parses_kline_bar_minute_turnover_at_index7() {
        // 原有格式: index 6 为 {}, index 7 为金额
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500",{},"1530000"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), Some(1)).unwrap();
        assert_eq!(out.turnover, 1530000.0);
    }

    #[test]
    fn parses_kline_bar_minute_no_turnover() {
        // turnover 缺失: 回退 0
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), Some(1)).unwrap();
        assert_eq!(out.turnover, 0.0);
    }
}
