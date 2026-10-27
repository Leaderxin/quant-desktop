//! 自选导入的**纯逻辑**：把 CSV 一行的「代码 / 名称」翻译成查询条件，
//! 再和搜索结果对上号。
//!
//! 与网络、IPC、数据库都无关，所以可以直接写测试 —— 导入功能的失败原因
//! 几乎全在这两块（代码写法五花八门、名称重名），而它们恰恰是最难在界面上
//! 复现的一类问题。

use crate::domain::StockBrief;

/// 交易所前缀。顺序无关，匹配时按 `sh` / `sz` / `bj` 全试一遍。
const EXCHANGES: [&str; 3] = ["sh", "sz", "bj"];

/// CSV 一行解析出来的查询条件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportQuery {
    /// 6 位代码（已去掉 `sh` / `sz` / `bj` 前缀与 `.SH` 之类的后缀）
    pub code: Option<String>,
    /// 代码里带的交易所前缀，用于同一代码命中多只时消歧
    pub exchange: Option<String>,
    /// 名称（原文，已 trim）
    pub name: Option<String>,
    /// 代码是**补零**出来的时，存补零前的原文。
    ///
    /// Excel 的数字单元格会吃掉前导零（`000001` 存回来变成 `1`），补零是
    /// 必要的猜测 —— 但它终究是猜测：行情软件导出表里夹着的 5 位港股代码
    /// `700,腾讯控股` 也会被补成 `000700`、唯一命中模塑科技。所以补零出来的
    /// 代码不能单独作为依据，必须由名称证实（见 `match_padded_guess`）。
    pub padded: Option<String>,
}

/// 解析一行：代码列与名称列。
///
/// 两种写法都接受 —— 用户从行情软件里复制出来的可能是 `600519`、`sh600519`、
/// `600519.SH`，也可能是从设置页抄来的 `s_sh000001`（`s_` 是接口前缀）。
/// 代码列填的不是代码时（两列写反、或直接填了名称），按名称处理而不是报错：
/// 用户想表达的是同一件事，没必要让他去理解列的含义。
pub fn parse_query(code_cell: &str, name_cell: &str) -> ImportQuery {
    let code_cell = code_cell.trim();
    let name_cell = name_cell.trim();
    let (code, exchange, padded) = split_code(code_cell);

    let name = if !name_cell.is_empty() {
        Some(name_cell.to_string())
    } else if code.is_none() && !code_cell.is_empty() {
        Some(code_cell.to_string())
    } else {
        None
    };

    ImportQuery { code, exchange, name, padded }
}

/// 把各种写法的代码拆成 `(6 位码, 交易所, 补零前的原文)`；
/// 不是代码就返回 `(None, None, None)`。
///
/// 前缀/后缀都认，中间允许 `.` `:` `-` `_` 空格 —— 专业行情软件导出 CSV 时
/// 各家写法不一：`sh600519`、`SH.600519`、`600519.SH`、`600519-sh` 都出现过。
fn split_code(raw: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut s = raw.trim().to_ascii_lowercase();
    if s.is_empty() {
        return (None, None, None);
    }
    // 数据源前缀：`s_sh000001`
    if let Some(rest) = s.strip_prefix("s_") {
        s = rest.to_string();
    }

    // 前缀写法：sh600519 / sh.600519 / SH:600519 / sh-600519 / sh 600519
    for ex in EXCHANGES {
        if let Some(rest) = s.strip_prefix(ex) {
            let digits = rest.trim_start_matches(CODE_SEPARATORS);
            if is_six_digits(digits) {
                return (Some(digits.to_string()), Some(ex.to_string()), None);
            }
        }
    }

    // 后缀写法：600519.sh / 600519-SH / 600519 sh
    for ex in EXCHANGES {
        if let Some(stem) = s.strip_suffix(ex) {
            let digits = stem.trim_end_matches(CODE_SEPARATORS);
            if is_six_digits(digits) {
                return (Some(digits.to_string()), Some(ex.to_string()), None);
            }
        }
    }

    // 纯数字：足 6 位是代码；不足 6 位是 Excel 吃掉前导零的残留，补上并
    // 带上原文 —— 补出来的是猜测，可信度由 match_stock 判断。
    // 7 位以上（美股等）不是 A 股代码，不猜 —— 注意 `{:0>6}` 对更长的串
    // 只是不截断，长度这道闸必须自己把。
    if !s.is_empty() && s.len() < 6 && s.chars().all(|c| c.is_ascii_digit()) {
        let original = s.clone();
        return (Some(format!("{:0>6}", s)), None, Some(original));
    }
    if is_six_digits(&s) {
        return (Some(s), None, None);
    }

    (None, None, None)
}

/// 交易所前缀与代码之间允许的分隔符
const CODE_SEPARATORS: &[char] = &['.', ':', '-', '_', ' ', '\t'];

fn is_six_digits(s: &str) -> bool {
    s.len() == 6 && s.chars().all(|c| c.is_ascii_digit())
}

/// `sh600519` → `600519`
fn bare_code(full: &str) -> &str {
    for ex in EXCHANGES {
        if let Some(rest) = full.strip_prefix(ex) {
            return rest;
        }
    }
    full
}

/// 在搜索结果里挑出这一行真正指的那只标的。
///
/// 规则是「代码优先、名称兜底、精确匹配」：
/// - **补零出来的代码是猜测，必须由名称证实**（`1,平安银行` 可以导，
///   `700,腾讯控股` 不能悄悄导成模塑科技）—— 见 `match_padded_guess`；
/// - 代码按 6 位比对（`sh600519` 与 `600519` 是同一只），带交易所时优先同一交易所；
/// - 代码命中多只（沪深同号，如 `000001`）时，用名称在**这几只里**再筛一次 ——
///   名称本来就填在表里，是现成的消歧信息，不该让用户回头补前缀；
/// - 代码对不上再用名称，名称必须**完全相同** —— 导入是批量操作，模糊匹配
///   悄悄导错一只，比报错难发现得多；
/// - 名称命中多只时直接报错并列出代码，让人回模板里补代码，而不是替他选一只。
pub fn match_stock(candidates: &[StockBrief], query: &ImportQuery) -> Result<StockBrief, String> {
    if let Some(raw) = &query.padded {
        return match_padded_guess(candidates, query, raw);
    }

    if let Some(code) = &query.code {
        let mut hits: Vec<&StockBrief> = candidates
            .iter()
            .filter(|c| bare_code(&c.code) == code)
            .collect();
        if let Some(ex) = &query.exchange {
            let same_exchange: Vec<&StockBrief> = hits
                .iter()
                .copied()
                .filter(|c| c.code.starts_with(ex.as_str()))
                .collect();
            if !same_exchange.is_empty() {
                hits = same_exchange;
            }
        }
        match hits.len() {
            1 => return Ok(hits[0].clone()),
            n if n > 1 => {
                // 代码不唯一（沪深同号，如 000001 上证指数 / 平安银行）时，
                // 名称正好是现成的消歧信息：填了名称就在这几只里再精确匹配一次，
                // 而不是让用户回头去加交易所前缀。
                if let Some(name) = &query.name {
                    let by_name: Vec<&StockBrief> = hits
                        .iter()
                        .copied()
                        .filter(|c| c.name.trim() == name)
                        .collect();
                    if by_name.len() == 1 {
                        return Ok(by_name[0].clone());
                    }
                }
                return Err(format!(
                    "代码 {} 在沪深各有一只（{}），请把名称一并填上，或写成 sh{} / sz{}",
                    code,
                    list_codes(&hits),
                    code,
                    code
                ));
            }
            _ => {}
        }
    }

    if let Some(name) = &query.name {
        let exact: Vec<&StockBrief> = candidates
            .iter()
            .filter(|c| c.name.trim() == name)
            .collect();
        match exact.len() {
            1 => return Ok(exact[0].clone()),
            n if n > 1 => {
                return Err(format!(
                    "名称「{}」匹配到多只（{}），请在模板里填写代码",
                    name,
                    list_codes(&exact)
                ))
            }
            _ => {}
        }
        if let Some(code) = &query.code {
            return Err(format!("未找到代码 {} 对应的标的", code));
        }
        return Err(format!("未找到名称「{}」", name));
    }

    if let Some(code) = &query.code {
        return Err(format!("未找到代码 {} 对应的标的", code));
    }
    Err("缺少代码与名称".to_string())
}

fn list_codes(stocks: &[&StockBrief]) -> String {
    stocks
        .iter()
        .map(|s| s.code.as_str())
        .collect::<Vec<_>>()
        .join("、")
}

/// 补零代码的匹配：**名称是唯一可信的依据，补出的代码只用来消歧。**
///
/// 为什么不反过来：`700,腾讯控股`（行情软件导出表里夹着的港股）会被补成
/// `000700` 唯一命中模塑科技 —— 拿猜测的代码当依据、再拿名称「确认」，
/// 实际上是让猜测否决用户的原文，属于这套匹配器存在的全部理由（「不报错、
/// 只导错」）。名称是用户填的原文，代码是我们造的，谁可信一目了然。
fn match_padded_guess(
    candidates: &[StockBrief],
    query: &ImportQuery,
    raw: &str,
) -> Result<StockBrief, String> {
    let Some(name) = &query.name else {
        return Err(format!(
            "代码「{}」不足 6 位，请补全前导零，或填上名称按名称导入",
            raw
        ));
    };

    let exact: Vec<&StockBrief> = candidates
        .iter()
        .filter(|c| c.name.trim() == name)
        .collect();
    match exact.len() {
        1 => return Ok(exact[0].clone()),
        n if n > 1 => {
            // 名称撞车时（沪深各一只同名），补出来的代码正好是现成的消歧线索
            let agrees: Vec<&StockBrief> = exact
                .iter()
                .copied()
                .filter(|c| Some(bare_code(&c.code)) == query.code.as_deref())
                .collect();
            if agrees.len() == 1 {
                return Ok(agrees[0].clone());
            }
            return Err(format!(
                "名称「{}」匹配到多只（{}），请填写完整代码",
                name,
                list_codes(&exact)
            ));
        }
        _ => {}
    }
    Err(format!("未找到名称「{}」", name))
}

/// 逐行搜索用的关键字。
///
/// 正常行用代码（更精确）；**补零行用名称** —— 补出来的代码是猜测，拿它当
/// 关键字搜出来的候选里很可能根本没有用户要的那只：`2,贵州茅台` 按 000002
/// 搜索只会得到万科A，名称才是这行真正说的东西。
pub fn search_keyword(query: &ImportQuery) -> String {
    if query.padded.is_some() {
        if let Some(name) = query.name.as_deref().filter(|n| !n.is_empty()) {
            return name.to_string();
        }
    }
    query
        .code
        .clone()
        .or_else(|| query.name.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stock(code: &str, name: &str) -> StockBrief {
        StockBrief {
            code: code.to_string(),
            market: "CN".to_string(),
            name: name.to_string(),
            category: crate::datasource::cn_category(code).to_string(),
        }
    }

    #[test]
    fn parses_every_code_spelling_people_paste() {
        for raw in [
            "600519",
            "sh600519",
            "SH600519",
            "600519.SH",
            "600519.sh",
            " s_sh600519 ",
            // 专业软件导出的各种分隔写法
            "sh.600519",
            "SH:600519",
            "sh-600519",
            "sh_600519",
            "sh 600519",
            "600519-sh",
            "600519 sh",
        ] {
            let q = parse_query(raw, "");
            assert_eq!(q.code.as_deref(), Some("600519"), "{}", raw);
        }
        assert_eq!(parse_query("sh600519", "").exchange.as_deref(), Some("sh"));
        assert_eq!(parse_query("600519.SZ", "").exchange.as_deref(), Some("sz"));
        assert_eq!(parse_query("s_sh000001", "").code.as_deref(), Some("000001"));
    }

    /// Excel 的数字单元格会吃掉前导零（000001 → 1），补回来才能匹配上。
    /// 补出来的码是**猜测**，原文要一起带出来给匹配端当可信度依据。
    #[test]
    fn pads_leading_zeros_lost_by_excel() {
        let q = parse_query("1", "");
        assert_eq!(q.code.as_deref(), Some("000001"));
        assert_eq!(q.padded.as_deref(), Some("1"));
        let q = parse_query(" 600 ", "");
        assert_eq!(q.code.as_deref(), Some("000600"));
        assert_eq!(q.padded.as_deref(), Some("600"));
        assert_eq!(parse_query("600519", "").code.as_deref(), Some("600519"));
        assert_eq!(parse_query("600519", "").padded, None, "足 6 位不是猜测");
        // 7 位不是 A 股代码，也不该被补成 8 位
        assert_eq!(parse_query("1234567", "").code, None);
    }

    /// 补零的代码必须由名称证实 —— 否则行情软件导出表里夹着的港股
    /// `700,腾讯控股` 会被补成 000700、静默导成模塑科技。
    #[test]
    fn padded_code_needs_the_name_to_confirm() {
        // 名称对得上 → 导；这里两只都叫得上号，靠名称选中平安银行
        let candidates = vec![stock("sh000001", "上证指数"), stock("sz000001", "平安银行")];
        let hit = match_stock(&candidates, &parse_query("1", "平安银行")).unwrap();
        assert_eq!(hit.code, "sz000001");

        // 名称对不上候选 → 报错，而不是拿补零的代码硬选一只
        let candidates = vec![stock("sz000700", "模塑科技")];
        let err = match_stock(&candidates, &parse_query("700", "腾讯控股")).unwrap_err();
        assert!(err.contains("未找到名称「腾讯控股」"), "{}", err);

        // 只填短代码、没有名称 → 不猜，把话说清楚
        let err = match_stock(&candidates, &parse_query("700", "")).unwrap_err();
        assert!(err.contains("不足 6 位"), "{}", err);
    }

    /// 名称撞车时（沪深各一只同名），补零出来的代码正好能当消歧线索。
    #[test]
    fn padded_code_disambiguates_same_named_hits() {
        let candidates = vec![stock("sh000300", "沪深300"), stock("sz399300", "沪深300")];
        let hit = match_stock(&candidates, &parse_query("300", "沪深300")).unwrap();
        assert_eq!(hit.code, "sh000300");
    }

    /// 补零行的搜索关键字用**名称**：`2,贵州茅台` 按 000002 搜索只会得到
    /// 万科A —— 拿猜测的代码当关键字，候选里根本没有用户要的那只。
    #[test]
    fn search_keyword_prefers_name_for_padded_codes() {
        assert_eq!(search_keyword(&parse_query("2", "贵州茅台")), "贵州茅台");
        assert_eq!(search_keyword(&parse_query("600519", "")), "600519");
        assert_eq!(search_keyword(&parse_query("", "贵州茅台")), "贵州茅台");
        assert_eq!(search_keyword(&parse_query("700", "")), "000700", "没名称时退回补零码，匹配端会给出行数不足的错误");
    }

    /// 代码列写错时按名称试一次：两列写反、或直接填了名称都能导。
    #[test]
    fn non_code_cell_falls_back_to_name() {
        let q = parse_query("贵州茅台", "");
        assert_eq!(q.code, None);
        assert_eq!(q.name.as_deref(), Some("贵州茅台"));

        let q = parse_query("600519", "贵州茅台");
        assert_eq!(q.code.as_deref(), Some("600519"));
        assert_eq!(q.name.as_deref(), Some("贵州茅台"));

        let q = parse_query("", "");
        assert_eq!(q, ImportQuery::default());
    }

    #[test]
    fn matches_by_code_regardless_of_prefix_style() {
        let candidates = vec![stock("sh600519", "贵州茅台"), stock("sz000001", "平安银行")];
        let hit = match_stock(&candidates, &parse_query("600519", "")).unwrap();
        assert_eq!(hit.code, "sh600519");
        let hit = match_stock(&candidates, &parse_query("000001", "")).unwrap();
        assert_eq!(hit.code, "sz000001");
    }

    /// 同一 6 位码在沪深各有一只时（指数/基金常见），带交易所前缀才消歧。
    #[test]
    fn exchange_hint_disambiguates_same_code() {
        let candidates = vec![stock("sh000001", "上证指数"), stock("sz000001", "平安银行")];
        let hit = match_stock(&candidates, &parse_query("sz000001", "")).unwrap();
        assert_eq!(hit.name, "平安银行");

        let err = match_stock(&candidates, &parse_query("000001", "")).unwrap_err();
        assert!(err.contains("在沪深各有一只"), "{}", err);
        assert!(err.contains("sh000001") && err.contains("sz000001"), "{}", err);
    }

    /// 代码不唯一时，名称就是现成的消歧信息 —— 模板示例 `000001,平安银行`
    /// 正是这种情况，不该判失败。
    #[test]
    fn ambiguous_code_is_disambiguated_by_name() {
        let candidates = vec![stock("sh000001", "上证指数"), stock("sz000001", "平安银行")];
        let hit = match_stock(&candidates, &parse_query("000001", "平安银行")).unwrap();
        assert_eq!(hit.code, "sz000001");
        let hit = match_stock(&candidates, &parse_query("000001", "上证指数")).unwrap();
        assert_eq!(hit.code, "sh000001");

        // 名称与这几只都对不上时仍按歧义报错，不能瞎选一只
        let err = match_stock(&candidates, &parse_query("000001", "沪深300ETF")).unwrap_err();
        assert!(err.contains("在沪深各有一只"), "{}", err);
    }

    #[test]
    fn falls_back_to_exact_name() {
        let candidates = vec![stock("sh600519", "贵州茅台"), stock("sh600809", "山西汾酒")];
        let hit = match_stock(&candidates, &parse_query("", "贵州茅台")).unwrap();
        assert_eq!(hit.code, "sh600519");
    }

    /// 名称只认完全相等：搜索接口会返回一堆包含关键字的标的，模糊匹配会导错。
    #[test]
    fn name_must_match_exactly() {
        let candidates = vec![stock("sh600519", "贵州茅台"), stock("sh600809", "山西汾酒")];
        let err = match_stock(&candidates, &parse_query("", "茅台")).unwrap_err();
        assert!(err.contains("未找到名称"), "{}", err);
    }

    #[test]
    fn ambiguous_name_lists_candidates() {
        let candidates = vec![stock("sh510300", "沪深300ETF"), stock("sz159919", "沪深300ETF")];
        let err = match_stock(&candidates, &parse_query("", "沪深300ETF")).unwrap_err();
        assert!(err.contains("匹配到多只"), "{}", err);
        assert!(err.contains("sh510300") && err.contains("sz159919"), "{}", err);
    }

    #[test]
    fn missing_code_and_name_is_explained() {
        let err = match_stock(&[], &parse_query("", "")).unwrap_err();
        assert_eq!(err, "缺少代码与名称");
    }

    #[test]
    fn unknown_code_reports_the_code_not_the_name() {
        let candidates = vec![stock("sh600519", "贵州茅台")];
        let err = match_stock(&candidates, &parse_query("600000", "")).unwrap_err();
        assert!(err.contains("600000"), "{}", err);
    }
}
