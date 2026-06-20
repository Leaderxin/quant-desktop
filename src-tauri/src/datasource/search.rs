use std::sync::OnceLock;
use std::time::Duration;

use encoding_rs::GBK;
use reqwest::Client;

use crate::domain::StockBrief;
use super::headers;

const SINA_SUGGEST_URL: &str = "https://suggest3.sinajs.cn/suggest/name=cn";
const TENCENT_SUGGEST_URL: &str = "http://smartbox.gtimg.cn/s3/";

/// Maximum number of search results returned to the frontend
const MAX_RESULTS: usize = 20;

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("Failed to build search reqwest Client")
    })
}

/// Search stocks by code or name using Sina's public suggest API.
///
/// Supports:
/// - Exact 6-digit codes (e.g. "600519")
/// - Partial codes (e.g. "600")
/// - Chinese names (e.g. "茅台") or pinyin abbreviations
///
/// Returns up to 20 A-share matches.
pub async fn suggest_search(keyword: &str) -> Result<Vec<StockBrief>, String> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    let url = format!("{}&key={}", SINA_SUGGEST_URL, urlencoding(trimmed));

    let resp = headers::with_browser_headers(
        client().get(&url),
        "https://finance.sina.com.cn",
    )
        .send()
        .await
        .map_err(|e| format!("Search request failed: {:#}", e))?;

    let body_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Search read failed: {:#}", e))?;

    let (body, _, _) = GBK.decode(&body_bytes);

    let results = parse_sina_suggest(&body, MAX_RESULTS);
    Ok(results)
}

/// Search stocks via Tencent smartbox API.
///
/// Supports:
/// - Exact codes, partial codes, Chinese names, pinyin abbreviations
///
/// Response format (UTF-8 with \uXXXX escapes):
///   v_hint="sh~600519~贵州茅台~gzmt~GP-A^sz~000001~...~...~GP-A^..."
///
/// Each entry: exchange~code~name~pinyin~type
/// Filter: type "GP-A" for A-shares.
pub async fn tencent_suggest_search(keyword: &str) -> Result<Vec<StockBrief>, String> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    let url = format!(
        "{}?q={}&t=all",
        TENCENT_SUGGEST_URL,
        urlencoding(trimmed)
    );

    let resp = headers::with_browser_headers(
        client().get(&url),
        "https://gu.qq.com",
    )
        .send()
        .await
        .map_err(|e| format!("Tencent search request failed: {:#}", e))?;

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Tencent search read failed: {:#}", e))?;

    let results = parse_tencent_smartbox(&body, MAX_RESULTS);
    Ok(results)
}

/// Parse Tencent smartbox response into StockBrief list.
///
/// Response format:
///   v_hint="sh~600519~贵州茅台~gzmt~GP-A^sz~000001~平安银行~payh~GP-A^..."
///
/// Fields separated by `~`, entries separated by `^`.
/// Field order: [0]=exchange, [1]=code, [2]=name(\u escaped), [3]=pinyin, [4]=type
fn parse_tencent_smartbox(body: &str, limit: usize) -> Vec<StockBrief> {
    let content = body
        .find("v_hint=\"")
        .and_then(|start| {
            let after = start + 8;
            let remaining = &body[after..];
            remaining.find('"').map(|end| &body[after..after + end])
        })
        .unwrap_or("");

    if content.is_empty() || content == "N" {
        return vec![];
    }

    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for entry in content.split('^') {
        if entry.is_empty() || results.len() >= limit {
            continue;
        }

        let fields: Vec<&str> = entry.split('~').collect();
        if fields.len() < 5 {
            continue;
        }

        // GP-A=A股, ETF=ETF, LOF=LOF
        let stype = fields[4];
        if !matches!(stype, "GP-A" | "ETF" | "LOF") {
            continue;
        }

        let code = fields[1].to_string();
        // Must be 6-digit numeric code
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let name = unescape_unicode(fields[2]);
        if name.is_empty() {
            continue;
        }

        let market = match fields[0] {
            "sh" => "CN",
            "sz" => "CN",
            _ => "CN",
        };

        if seen.insert(code.clone()) {
            results.push(StockBrief {
                code,
                market: market.to_string(),
                name,
            });
        }
    }

    results
}

/// Decode \uXXXX escape sequences into UTF-8 characters.
fn unescape_unicode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'u') {
            chars.next(); // consume 'u'
            let mut hex = String::with_capacity(4);
            for _ in 0..4 {
                if let Some(h) = chars.next() {
                    hex.push(h);
                } else {
                    break;
                }
            }
            if hex.len() == 4 {
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    if let Some(uc) = char::from_u32(cp) {
                        out.push(uc);
                        continue;
                    }
                }
            }
            // Failed to parse — push raw chars back
            out.push_str("\\u");
            out.push_str(&hex);
        } else {
            out.push(c);
        }
    }
    out
}

/// URL-encode a string slice (handles Chinese characters, etc.)
fn urlencoding(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for byte in s.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Parse Sina suggest response into StockBrief list.
///
/// Response format (GBK-decoded):
///   var cn="<entry>;<entry>;...;"
///
/// Each entry (comma-separated):
///   [0]=full_code (sh600519), [1]=type (11=A-share), [2]=code, [4]=name
///
/// Filter: only type-11/12 entries (A-shares, both Shanghai and Shenzhen).
fn parse_sina_suggest(body: &str, limit: usize) -> Vec<StockBrief> {
    // Extract content between quotes after "cn="
    let content = body
        .find("cn=\"")
        .and_then(|start| {
            let after_quote = start + 4; // "cn=" + opening quote
            let remaining = &body[after_quote..];
            remaining.find('"').map(|end| &body[after_quote..after_quote + end])
        })
        .unwrap_or("");

    if content.is_empty() {
        return vec![];
    }

    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for entry in content.split(';') {
        if entry.is_empty() || results.len() >= limit {
            continue;
        }

        let fields: Vec<&str> = entry.split(',').collect();
        if fields.len() < 5 {
            continue;
        }

        // Type 11=A-share, 12=B-share, 22/203=ETF, 23=LOF
        let stype = fields.get(1).copied().unwrap_or("");
        if !matches!(stype, "11" | "12" | "22" | "23" | "203") {
            continue;
        }

        let code = fields[2].to_string();
        // Verify it's a numeric 6-digit A-share code
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let name = fields[4].to_string();
        if name.is_empty() {
            continue;
        }

        // Determine market from full code prefix
        let full_code = fields[0];
        let market = if full_code.starts_with("sh") {
            "CN"
        } else if full_code.starts_with("sz") {
            "CN"
        } else {
            "CN"
        };

        // Deduplicate by code
        if seen.insert(code.clone()) {
            results.push(StockBrief {
                code,
                market: market.to_string(),
                name,
            });
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("600519"), "600519");
        assert_eq!(urlencoding("茅台"), "%E8%8C%85%E5%8F%B0");
    }

    #[test]
    fn test_unescape_unicode() {
        assert_eq!(unescape_unicode("\\u8d35\\u5dde\\u8305\\u53f0"), "贵州茅台");
        assert_eq!(unescape_unicode("\\u5e73\\u5b89\\u94f6\\u884c"), "平安银行");
        assert_eq!(unescape_unicode("gzmt"), "gzmt");
        assert_eq!(unescape_unicode(""), "");
        // Invalid escape
        assert_eq!(unescape_unicode("\\u12"), "\\u12");
    }

    // ── Sina suggest tests ──

    #[test]
    fn test_parse_sina_exact_code() {
        let body = "var cn=\"sh600519,11,600519,sh600519,\u{8d35}\u{5dde}\u{8305}\u{53f0},,\u{8d35}\u{5dde}\u{8305}\u{53f0},99,1,ESG,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "600519");
        assert_eq!(results[0].market, "CN");
    }

    #[test]
    fn test_parse_sina_multiple() {
        let body = "var cn=\"sh600519,11,600519,sh600519,MT,,\u{8d35}\u{5dde}\u{8305}\u{53f0},99,1,ESG,,;sz000001,11,000001,sz000001,PB,,\u{5e73}\u{5b89}\u{94f6}\u{884c},99,1,ESG,,;00883,31,00883,00883,CM,,\u{4e2d}\u{56fd}\u{6d77}\u{6d0b}\u{77f3}\u{6cb9},99,1,ESG,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "600519");
        assert_eq!(results[1].code, "000001");
    }

    #[test]
    fn test_parse_sina_etf() {
        // Type 22 = ETF (off-exchange), Type 203 = ETF (on-exchange)
        let body = "var cn=\"sh510050,203,510050,sh510050,ETF_50,,\u{4e0a}\u{8bc1}50ETF,99,1,,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "510050");
    }

    #[test]
    fn test_parse_sina_name_search() {
        let body = "var cn=\"sh600030,11,600030,sh600030,ZX,,\u{4e2d}\u{4fe1}\u{8bc1}\u{5238},99,1,ESG,,;01114,31,01114,01114,HC,,\u{534e}\u{6668}\u{4e2d}\u{56fd},99,1,ESG,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "600030");
    }

    // ── Tencent smartbox tests ──

    #[test]
    fn test_parse_tencent_exact_code() {
        let body = "v_hint=\"sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A\"";
        let results = parse_tencent_smartbox(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "600519");
        assert_eq!(results[0].name, "贵州茅台");
        assert_eq!(results[0].market, "CN");
    }

    #[test]
    fn test_parse_tencent_partial_code() {
        let body = "v_hint=\"sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A^sz~000600~\\u5efa\\u6295\\u80fd\\u6e90~jtny~GP-A^hk~00600~\\u7231\\u82af\\u5143\\u667a~axyz~GP\"";
        let results = parse_tencent_smartbox(body, 20);
        // Only 2 A-shares, HK stock (GP) filtered out
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "600519");
        assert_eq!(results[1].code, "000600");
    }

    #[test]
    fn test_parse_tencent_no_results() {
        let body = "v_hint=\"N\"";
        let results = parse_tencent_smartbox(body, 20);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_parse_tencent_filters_non_a() {
        let body = "v_hint=\"jj~000600~\\u6c47\\u6dfb\\u5bcc\\u548c~htfh~KJ-HB^sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A\"";
        let results = parse_tencent_smartbox(body, 20);
        // Only GP-A, fund (KJ-HB) filtered out
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "600519");
    }

    #[test]
    fn test_parse_tencent_etf() {
        let body = "v_hint=\"sh~510050~\\u4e0a\\u8bc150ETF\\u534e\\u590f~sz50etfhx~ETF^sz~159915~\\u521b\\u4e1a\\u677fETF\\u6613\\u65b9\\u8fbe~cybetfyfd~ETF^hk~02800~\\u76c8\\u5bcc\\u57fa\\u91d1~yfjj~ETF\"";
        let results = parse_tencent_smartbox(body, 20);
        // The HK ETF (type ETF but code not 6-digit) should be filtered out
        // Plus the ETF type now passes
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "510050");
        assert_eq!(results[1].code, "159915");
    }
}
