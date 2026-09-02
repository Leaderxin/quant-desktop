use reqwest::Client;
use encoding_rs::GBK;
use crate::domain::{SectorItem, AppError};
use super::headers;

/// 市场概览聚合客户端 —— 专门负责「全市场聚合数据」,与报价数据源(腾讯/新浪)解耦。
///
/// 注意:它**不实现 `DataSource` trait** —— 它不提供个股/指数报价,只提供
/// 成交额 + 涨跌家数 + 板块排名。
///
/// 数据源实测依据(2026-08-26,本机验证):
/// - 成交额:新浪 `hq.sinajs.cn/list=sh000001,sz399106`,第 9 字段=成交额(元)。
///   注意用深证综指 `sz399106`(覆盖深市全市场),不用成指 `sz399001`。
/// - 涨跌家数:东财 `push2ex.eastmoney.com/getTopicZDFenBu`,返回涨跌分布自算。
/// - 板块排名:东财 `push2delay.eastmoney.com/api/qt/clist/get`,
///   行业 `m:90+t:2`、概念 `m:90+t:3`。
///
/// ⚠️ 域名坑:`push2.eastmoney.com` 在本环境被拦截(clist 返回 HTTP 000),
/// 必须用 `push2delay` / `push2ex` 这两个可用域名。
pub struct MarketOverviewClient {
    client: Client,
}

/// 新浪指数接口(GBK) —— 成交额用
const SINA_INDEX_URL: &str = "https://hq.sinajs.cn/list=sh000001,sz399106";

/// 东财涨跌分布接口
const EASTMONEY_PUSH2EX: &str = "https://push2ex.eastmoney.com";
/// 东财板块列表接口(可用的延迟域名)
const EASTMONEY_PUSH2DELAY: &str = "https://push2delay.eastmoney.com";

/// getTopicZDFenBu 的 ut token
const BREADTH_UT: &str = "7eea3edcaed734bea9cbfc24409ed989";

impl MarketOverviewClient {
    pub fn new() -> Self {
        Self {
            client: super::shared_client().clone(),
        }
    }

    /// 沪深两市总成交额(元)。
    /// 取 上证指数(sh000001) + 深证综指(sz399106) 的成交额相加。
    pub async fn fetch_total_turnover(&self) -> Result<f64, AppError> {
        let resp = headers::with_browser_headers(
            self.client.get(SINA_INDEX_URL),
            "https://finance.sina.com.cn",
        )
            .send()
            .await
            .map_err(|e| AppError::network("sina", format!("成交额请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("sina", format!("成交额 HTTP {}", resp.status())));
        }

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::network("sina", format!("成交额读取失败: {:#}", e)))?;
        let (body, _, _) = GBK.decode(&body_bytes);

        Ok(parse_sina_turnover(&body))
    }

    /// 涨跌家数(全市场)。返回 (上涨, 下跌, 平盘)。
    pub async fn fetch_market_breadth(&self) -> Result<(u32, u32, u32), AppError> {
        let url = format!("{}/getTopicZDFenBu?ut={}&dpt=wz.ztzt", EASTMONEY_PUSH2EX, BREADTH_UT);
        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://quote.eastmoney.com/",
        )
            .send()
            .await
            .map_err(|e| AppError::network("eastmoney", format!("涨跌家数请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("eastmoney", format!("涨跌家数 HTTP {}", resp.status())));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::network("eastmoney", format!("涨跌家数解析失败: {}", e)))?;

        Ok(parse_breadth(&body).unwrap_or((0, 0, 0)))
    }

    /// 板块排名(行业或概念)。`fs` 为东财筛选串(`m:90+t:2` 行业 / `m:90+t:3` 概念)。
    /// 按 `direction` 排序取前 5 返回。
    pub async fn fetch_sector_ranking(
        &self,
        fs: &str,
        direction: &str,
    ) -> Result<Vec<SectorItem>, AppError> {
        self.fetch_sector_ranking_paged(fs, direction, 5).await
    }

    /// 概念板块排名 —— 与 `fetch_sector_ranking` 相同,但先多拉一些再剔除
    /// 风格/指数/资金/业绩等「非主题概念」板块,最后截取前 5。
    ///
    /// 为什么不能 `pz=5` 直接过滤:东财 `m:90+t:3` 的「概念」实为主题概念 +
    /// 风格因子(历史新高、微盘股…) + 指数成分(中证500、茅指数…) + 资金持仓的
    /// 大杂烩,若榜首就是这类板块,直接过滤会导致列数不足 5。
    pub async fn fetch_concept_ranking(
        &self,
        direction: &str,
    ) -> Result<Vec<SectorItem>, AppError> {
        let mut items = self
            .fetch_sector_ranking_paged("m:90+t:3", direction, 30)
            .await?;
        items.retain(|s| !is_non_concept_board(&s.code));
        items.truncate(5);
        Ok(items)
    }

    async fn fetch_sector_ranking_paged(
        &self,
        fs: &str,
        direction: &str,
        pz: usize,
    ) -> Result<Vec<SectorItem>, AppError> {
        // po=1 降序(涨幅榜:涨跌幅从高到低), po=0 升序(跌幅榜:从低到高)。
        let po = if direction == "down" { "0" } else { "1" };

        let url = format!(
            "{}/api/qt/clist/get?pn=1&pz={}&po={}&np=1&fltt=2&invt=2&fid=f3&fs={}&fields=f12,f14,f3,f128,f136",
            EASTMONEY_PUSH2DELAY, pz, po, fs
        );
        let resp = headers::with_browser_headers(
            self.client.get(&url),
            "https://quote.eastmoney.com/",
        )
            .send()
            .await
            .map_err(|e| AppError::network("eastmoney", format!("板块排名请求失败: {:#}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::network("eastmoney", format!("板块排名 HTTP {}", resp.status())));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::network("eastmoney", format!("板块排名解析失败: {}", e)))?;

        Ok(parse_clist(&body))
    }
}

/// 概念板块黑名单 —— 东财 `m:90+t:3` 的「概念」实为主题概念 + 风格因子 + 指数成分 +
/// 资金持仓 + 业绩预披露的大杂烩。这里列出非「主题概念」的板块代码(取自 `f12`,如
/// `BK1675` 历史新高),在概念排名中剔除,只保留真正的主题概念(5G/AI/芯片/新能源…)。
///
/// 接口层面无法拆分:`t:1` 地域 / `t:2` 行业 / `t:3` 概念是仅有的合法筛选串,`t:4`+
/// 均返回空。故只能用代码黑名单兜底。
mod concept_blacklist {
    /// 宽基/成分指数(HS300_、中证500、茅指数…)
    pub const INDEX: &[&str] = &[
        "BK0498", "BK0499", "BK0500", "BK0568", "BK0610", "BK0611", "BK0612",
        "BK0636", "BK0638", "BK0701", "BK0705", "BK0742", "BK0743", "BK0821",
        "BK0867", "BK0879", "BK0999", "BK1000",
    ];

    /// 风格/因子(历史新高、微盘股、破净、大小盘成长价值、昨日涨停系列…)
    pub const STYLE: &[&str] = &[
        "BK0501", "BK0505", "BK0511",
        "BK0815", "BK0816", "BK0817",
        "BK1050", "BK1051", "BK1053", "BK1059",
        "BK1112", "BK1158",
        "BK1630", "BK1631", "BK1632", "BK1633", "BK1635", "BK1636", "BK1637",
        "BK1638", "BK1639", "BK1640", "BK1641", "BK1642", "BK1643", "BK1644",
        "BK1645", "BK1661", "BK1662", "BK1663", "BK1664", "BK1665", "BK1666",
        "BK1667", "BK1668", "BK1669", "BK1670", "BK1671", "BK1672", "BK1673",
        "BK1674", "BK1675", "BK1676", "BK1693", "BK1698", "BK1699", "BK1700",
        "BK1710", "BK1711", "BK1712", "BK1713", "BK1714", "BK1715", "BK1716",
        "BK1717", "BK1721", "BK1722", "BK1723", "BK1743",
    ];

    /// 资金/持仓/互联互通(基金重仓、沪股通、融资融券…)
    pub const FUND: &[&str] = &[
        "BK0520", "BK0528", "BK0535", "BK0536", "BK0552", "BK0596", "BK0707",
        "BK0718", "BK0804", "BK0823",
    ];

    /// 业绩预披露(预增/预减/扭亏/首亏…)
    pub const EARNINGS: &[&str] = &[
        "BK1198", "BK1199", "BK1628", "BK1680", "BK1681", "BK1682", "BK1749",
        "BK1750", "BK1751", "BK1752",
    ];
}

/// 判断板块代码是否属于「非主题概念」(风格/指数/资金/业绩),应从概念排名中剔除。
fn is_non_concept_board(code: &str) -> bool {
    concept_blacklist::INDEX.contains(&code)
        || concept_blacklist::STYLE.contains(&code)
        || concept_blacklist::FUND.contains(&code)
        || concept_blacklist::EARNINGS.contains(&code)
}

/// 解析新浪指数响应,累加两行(上证 + 深证)的成交额(字段索引 9)。
/// 响应形如 `var hq_str_sh000001="名称,开,昨收,价,高,低,0,0,量,额,...";`
fn parse_sina_turnover(body: &str) -> f64 {
    body.lines()
        .filter_map(|line| {
            let eq = line.find('=')?;
            let start = line[eq + 1..].find('"')? + eq + 2;
            let end = line[start..].find('"')? + start;
            let data = &line[start..end];
            let fields: Vec<&str> = data.split(',').collect();
            if fields.len() < 10 {
                return None;
            }
            fields[9].parse::<f64>().ok()
        })
        .sum()
}

/// 解析东财涨跌分布响应,自算涨/跌/平家数。
/// 响应形如 `{"data":{"fenbu":[{"-1":1173},{"0":143},{"1":1364},...]}}`,
/// 每个元素的 key 是涨跌幅区间(负=跌, 0=平, 正=涨),value 是该区间家数。
fn parse_breadth(body: &serde_json::Value) -> Option<(u32, u32, u32)> {
    let fenbu = body.pointer("/data/fenbu")?.as_array()?;
    if fenbu.is_empty() {
        return None;
    }
    let mut up = 0u32;
    let mut down = 0u32;
    let mut flat = 0u32;
    for item in fenbu {
        let Some(obj) = item.as_object() else { continue };
        for (k, v) in obj {
            let count = parse_count(Some(v));
            match k.parse::<i64>() {
                Ok(n) if n < 0 => down = down.saturating_add(count),
                Ok(n) if n > 0 => up = up.saturating_add(count),
                Ok(_) => flat = flat.saturating_add(count),
                Err(_) => {}
            }
        }
    }
    Some((up, down, flat))
}

/// 解析东财 `clist` 板块列表响应。
/// 字段:f12=代码 f14=名称 f3=涨跌幅% f128=领涨股 f136=领涨股涨跌幅%。
fn parse_clist(body: &serde_json::Value) -> Vec<SectorItem> {
    let Some(diff) = body.pointer("/data/diff").and_then(|d| d.as_array()) else {
        return Vec::new();
    };
    diff.iter()
        .filter_map(|item| {
            Some(SectorItem {
                code: item.get("f12")?.as_str()?.to_string(),
                name: item.get("f14")?.as_str()?.to_string(),
                change_pct: item.get("f3")?.as_f64().unwrap_or(0.0),
                leader_name: item.get("f128").and_then(|v| v.as_str()).map(|s| s.to_string()),
                leader_pct: item.get("f136").and_then(|v| v.as_f64()),
            })
        })
        .collect()
}

/// 解析东财的计数/家数字段 —— 兼容整数(`1313`)与浮点(`1313.0`)两种返回。
fn parse_count(v: Option<&serde_json::Value>) -> u32 {
    match v {
        Some(v) => v
            .as_u64()
            .map(|n| n as u32)
            .or_else(|| v.as_f64().map(|f| f as u32))
            .unwrap_or(0),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sina_turnover_two_lines() {
        let body = r#"var hq_str_sh000001="上证指数,1,2,3,4,5,0,0,489553021,857223487993,0";
var hq_str_sz399106="深证综指,1,2,3,4,5,0,0,57760736028,951499754726.529,0";"#;
        let t = parse_sina_turnover(body);
        assert!((t - 1808723242719.529).abs() < 0.001, "got {}", t);
    }

    #[test]
    fn parses_breadth_distribution() {
        let body = serde_json::json!({
            "data": { "fenbu": [
                {"-1": 1173}, {"-2": 690}, {"0": 143}, {"1": 1364}, {"2": 736}
            ]}
        });
        // 涨=1364+736=2100, 跌=1173+690=1863, 平=143
        assert_eq!(parse_breadth(&body), Some((2100, 1863, 143)));
    }

    #[test]
    fn breadth_empty_returns_none() {
        let body = serde_json::json!({ "data": { "fenbu": [] } });
        assert_eq!(parse_breadth(&body), None);
    }

    #[test]
    fn parses_clist_board() {
        let body = serde_json::json!({
            "data": { "diff": [
                {"f12": "BK1556", "f14": "教育运营及其他", "f3": 6.59, "f128": "ST豆神", "f136": 12.92}
            ]}
        });
        let items = parse_clist(&body);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "教育运营及其他");
        assert_eq!(items[0].change_pct, 6.59);
        assert_eq!(items[0].leader_name.as_deref(), Some("ST豆神"));
        assert_eq!(items[0].leader_pct, Some(12.92));
    }

    #[test]
    fn concept_blacklist_flags_non_concept_boards() {
        // 风格因子、指数成分、资金持仓、业绩预披露 → 剔除
        assert!(is_non_concept_board("BK1675")); // 历史新高(风格)
        assert!(is_non_concept_board("BK1639")); // 周期股(风格)
        assert!(is_non_concept_board("BK0500")); // HS300_(指数)
        assert!(is_non_concept_board("BK0999")); // 茅指数(指数)
        assert!(is_non_concept_board("BK0536")); // 基金重仓(资金)
        assert!(is_non_concept_board("BK0707")); // 沪股通(资金)
        assert!(is_non_concept_board("BK1749")); // 2026中报预增(业绩)
        // 主题概念 → 保留
        assert!(!is_non_concept_board("BK0917")); // 半导体概念
        assert!(!is_non_concept_board("BK0800")); // 人工智能
        assert!(!is_non_concept_board("BK0900")); // 新能源车
    }

    #[test]
    fn count_parses_int_and_float() {
        assert_eq!(parse_count(Some(&serde_json::json!(1313))), 1313);
        assert_eq!(parse_count(Some(&serde_json::json!(1313.0))), 1313);
        assert_eq!(parse_count(Some(&serde_json::json!("x"))), 0);
        assert_eq!(parse_count(None), 0);
    }
}
