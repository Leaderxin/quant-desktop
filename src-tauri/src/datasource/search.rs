use encoding_rs::GBK;

use crate::domain::StockBrief;
use super::headers;

const SINA_SUGGEST_URL: &str = "https://suggest3.sinajs.cn/suggest/name=cn";
const TENCENT_SUGGEST_URL: &str = "http://smartbox.gtimg.cn/s3/";

/// Maximum number of search results returned to the frontend
const MAX_RESULTS: usize = 20;

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
        super::shared_client().get(&url),
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
        super::shared_client().get(&url),
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
            "sh" | "sz" | "bj" => "CN",
            // Anything else (hk/us, and Tencent's `jj` for funds) has no CN ticker.
            _ => continue,
        };

        // Preserve the full symbol (exchange + code) so ambiguous codes
        // (index vs stock sharing a 6-digit code) stay distinct.
        let full_code = format!("{}{}", fields[0], code);
        let category = super::cn_category(&full_code).to_string();

        if seen.insert(full_code.clone()) {
            results.push(StockBrief {
                code: full_code,
                market: market.to_string(),
                name,
                category,
            });
        }
    }

    results
}

/// Whether a full symbol carries an exchange prefix both adapters can build a
/// quote request from (`sh`/`sz`/`bj` — see `code_to_tencent` / `code_to_sina`).
///
/// Sina suggests 场外基金 (open-end funds) under an `of` prefix, reusing the very
/// same 6-digit code as the exchange-traded share class: searching 588000 returns
/// both `of588000` (type 22, the OTC class of 科创50ETF华夏) and `sh588000`
/// (type 203, the ETF itself), two rows with identical names. An `of` symbol has
/// no exchange ticker — the quote request falls back to `szof588000` and comes
/// back empty, so the watchlist row sits blank forever. Same for 159915/510050,
/// and for a pure OTC fund like 华夏成长混合A (`of000001`) no listed sibling
/// exists at all, which is why the entry is dropped rather than rewritten.
fn is_exchange_listed(full_code: &str) -> bool {
    full_code.starts_with("sh") || full_code.starts_with("sz") || full_code.starts_with("bj")
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
///   [0]=full_code (sh600519), [1]=type (11=A-share), [2]=code, [3]=symbol, [4]=name
///
/// Filter: type whitelist (A/B-share, ETF, LOF) **and** an exchange-prefixed symbol
/// (see `is_exchange_listed`).
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

        // Preserve the full symbol (sh/sz/bj + code). The exchange-prefixed symbol
        // is at [3]: for 北交所 (bj) entries Sina puts the name at [0] and the
        // symbol at [3], so [3] is the reliable source. A bare code can be
        // ambiguous — e.g. 000852 is both sh000852 (中证1000 index) and sz000852
        // (石化机械 stock) — so we key on the full symbol to keep them distinct.
        let full_code = fields[3].to_string();
        if !is_exchange_listed(&full_code) {
            continue;
        }
        let market = "CN".to_string();
        let category = super::cn_category(&full_code).to_string();

        // Deduplicate by full symbol, not bare code.
        if seen.insert(full_code.clone()) {
            results.push(StockBrief {
                code: full_code,
                market,
                name,
                category,
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
        assert_eq!(results[0].code, "sh600519");
        assert_eq!(results[0].market, "CN");
    }

    #[test]
    fn test_parse_sina_multiple() {
        let body = "var cn=\"sh600519,11,600519,sh600519,MT,,\u{8d35}\u{5dde}\u{8305}\u{53f0},99,1,ESG,,;sz000001,11,000001,sz000001,PB,,\u{5e73}\u{5b89}\u{94f6}\u{884c},99,1,ESG,,;00883,31,00883,00883,CM,,\u{4e2d}\u{56fd}\u{6d77}\u{6d0b}\u{77f3}\u{6cb9},99,1,ESG,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "sh600519");
        assert_eq!(results[1].code, "sz000001");
    }

    #[test]
    fn test_parse_sina_etf() {
        // Type 203 = ETF (on-exchange); type 22 is its OTC share class, `of`-prefixed
        let body = "var cn=\"sh510050,203,510050,sh510050,ETF_50,,\u{4e0a}\u{8bc1}50ETF,99,1,,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "sh510050");
    }

    #[test]
    fn test_parse_sina_suggest_drops_otc_fund_share_class() {
        // The real response for 588000: the OTC share class (`of`, type 22) and the
        // ETF itself (type 203) carry the same code and name. Only the latter is
        // quotable — the `of` symbol has no exchange ticker.
        let body = "var cn=\"of588000,22,588000,of588000,科创50ETF华夏,,科创50ETF华夏,99,1,,,;sh588000,203,588000,sh588000,科创50ETF华夏,,科创50ETF华夏,99,1,,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "sh588000");
        assert_eq!(results[0].category, "ETF");
    }

    #[test]
    fn test_parse_sina_suggest_drops_pure_otc_fund() {
        // 华夏成长混合A (of000001, type 201) has no listed sibling — 000001 is
        // 上证指数 / 平安银行, unrelated instruments. Nothing to keep.
        let body = "var cn=\"of000001,201,000001,of000001,华夏成长混合A,,华夏成长混合A,99,1,,,;of000001,21,000001,of000001,华夏成长混合A,,华夏成长混合A,99,1,,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_parse_sina_suggest_code_collision() {
        // 000852 is ambiguous: sh000852 = 中证1000 (index), sz000852 = 石化机械 (stock).
        // Dedup must key on the full symbol so both instruments survive.
        let body = "var cn=\"sh000852,11,000852,sh000852,中证1000,,中证1000,99,1,,,;sz000852,11,000852,sz000852,石化机械,,石化机械,99,1,,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "sh000852");
        assert_eq!(results[0].name, "中证1000");
        assert_eq!(results[1].code, "sz000852");
        assert_eq!(results[1].name, "石化机械");
    }

    #[test]
    fn test_parse_sina_suggest_bse_name_format() {
        // 北交所 (bj) entries put the name at [0] and the full symbol at [3].
        let body = "var cn=\"贝特瑞,11,920185,bj920185,贝特瑞,,贝特瑞,99,1,,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "bj920185");
        assert_eq!(results[0].name, "贝特瑞");
        assert_eq!(results[0].category, "GP-A");
    }

    #[test]
    fn test_parse_sina_name_search() {
        let body = "var cn=\"sh600030,11,600030,sh600030,ZX,,\u{4e2d}\u{4fe1}\u{8bc1}\u{5238},99,1,ESG,,;01114,31,01114,01114,HC,,\u{534e}\u{6668}\u{4e2d}\u{56fd},99,1,ESG,,\"";
        let results = parse_sina_suggest(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "sh600030");
    }

    // ── Tencent smartbox tests ──

    #[test]
    fn test_parse_tencent_exact_code() {
        let body = "v_hint=\"sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A\"";
        let results = parse_tencent_smartbox(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "sh600519");
        assert_eq!(results[0].name, "贵州茅台");
        assert_eq!(results[0].market, "CN");
    }

    #[test]
    fn test_parse_tencent_partial_code() {
        let body = "v_hint=\"sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A^sz~000600~\\u5efa\\u6295\\u80fd\\u6e90~jtny~GP-A^hk~00600~\\u7231\\u82af\\u5143\\u667a~axyz~GP\"";
        let results = parse_tencent_smartbox(body, 20);
        // Only 2 A-shares, HK stock (GP) filtered out
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "sh600519");
        assert_eq!(results[1].code, "sz000600");
    }

    #[test]
    fn test_parse_tencent_no_results() {
        let body = "v_hint=\"N\"";
        let results = parse_tencent_smartbox(body, 20);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_parse_tencent_drops_non_cn_exchange() {
        // A `hk`/`us` symbol has no CN ticker: `code_to_tencent` would leave it as-is
        // (market != "CN") or `code_to_sina` would prepend `sz`. Guard on the exchange
        // itself rather than trusting the type whitelist to cover every case.
        let body = "v_hint=\"hk~00700~\\u817e\\u8baf\\u63a7\\u80a1~txkg~GP-A^sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A\"";
        let results = parse_tencent_smartbox(body, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "sh600519");
    }

    #[test]
    fn test_parse_tencent_filters_non_a() {
        let body = "v_hint=\"jj~000600~\\u6c47\\u6dfb\\u5bcc\\u548c~htfh~KJ-HB^sh~600519~\\u8d35\\u5dde\\u8305\\u53f0~gzmt~GP-A\"";
        let results = parse_tencent_smartbox(body, 20);
        // Only GP-A, fund (KJ-HB) filtered out
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, "sh600519");
    }

    #[test]
    fn test_parse_tencent_etf() {
        let body = "v_hint=\"sh~510050~\\u4e0a\\u8bc150ETF\\u534e\\u590f~sz50etfhx~ETF^sz~159915~\\u521b\\u4e1a\\u677fETF\\u6613\\u65b9\\u8fbe~cybetfyfd~ETF^hk~02800~\\u76c8\\u5bcc\\u57fa\\u91d1~yfjj~ETF\"";
        let results = parse_tencent_smartbox(body, 20);
        // The HK ETF (type ETF but code not 6-digit) should be filtered out
        // Plus the ETF type now passes
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].code, "sh510050");
        assert_eq!(results[1].code, "sz159915");
    }
}
