use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{FixedOffset, Timelike, Utc};
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
/// - 较昨日同期的成交额增量:腾讯分时 `ifzq.gtimg.cn/appstock/app/{minute,day}/query`
///   —— 每行是「时间 点位 累计成交量 累计成交额」,五日那份直接给出前一交易日的曲线。
///
/// ⚠️ 域名坑:`push2.eastmoney.com` 在本环境被拦截(clist 返回 HTTP 000),
/// 必须用 `push2delay` / `push2ex` 这两个可用域名。
pub struct MarketOverviewClient {
    client: Client,
    /// 前一交易日的分时曲线。它是**静态**的(昨天不会变),每天只值得拉一次;
    /// 当日那条每次轮询都要新值,走 `MINUTE_QUERY` 的小响应。
    prev_day: Mutex<Option<Arc<PrevDayCurves>>>,
}

/// 新浪指数接口(GBK) —— 成交额用
const SINA_INDEX_URL: &str = "https://hq.sinajs.cn/list=sh000001,sz399106";

/// 东财涨跌分布接口
const EASTMONEY_PUSH2EX: &str = "https://push2ex.eastmoney.com";
/// 东财板块列表接口(可用的延迟域名)
const EASTMONEY_PUSH2DELAY: &str = "https://push2delay.eastmoney.com";

/// 腾讯当日分时(~12KB):`data.<code>.data = {date, data: [...]}`
const TENCENT_MINUTE_QUERY: &str = "http://ifzq.gtimg.cn/appstock/app/minute/query?code=";
/// 腾讯五日分时(~50KB):`data.<code>.data = [{date, data: [...]}, ...]`
const TENCENT_DAY_QUERY: &str = "http://ifzq.gtimg.cn/appstock/app/day/query?code=";

/// 两市成交额口径的两个指数 —— 与 `SINA_INDEX_URL` 一致,增量和上面那个数才可比。
const TURNOVER_CODES: [&str; 2] = ["sh000001", "sz399106"];

/// getTopicZDFenBu 的 ut token
const BREADTH_UT: &str = "7eea3edcaed734bea9cbfc24409ed989";

/// 板块榜单条数的合法区间。设置页的「自定义」输入框与后端共用这一对边界 ——
/// 前端限位只是为了不让人填出离谱的值，真正的兜底在 `clamp_top_n`:
/// IPC 参数是前端传来的，不能假设它已经被校验过。
pub const SECTOR_TOP_N_MIN: usize = 1;
pub const SECTOR_TOP_N_MAX: usize = 50;

/// 把请求的榜单条数夹到合法区间。上限 50 同时是概念板块预拉量(200 条)的约束来源。
pub fn clamp_top_n(n: usize) -> usize {
    n.clamp(SECTOR_TOP_N_MIN, SECTOR_TOP_N_MAX)
}

/// 概念板块排名的预拉条数。
///
/// 不能直接按 `top_n` 拉:东财「概念」(`m:90+t:3`) 里混着风格因子、指数成分、
/// 资金持仓这些非主题板块(见 `is_non_concept_board`),若榜首恰好是这类,按 `top_n`
/// 拉完再过滤就会列数不足。按 10 倍留余量,再封顶 200 控制响应体大小 ——
/// `top_n` 上限 50,而 200 条在最坏情况(涨停潮,被剔比例远超平日)下过滤完仍够 50。
pub fn concept_prefetch_size(top_n: usize) -> usize {
    (clamp_top_n(top_n) * 10).clamp(50, 200)
}

/// 行业板块的预拉条数 —— 与 `concept_prefetch_size` 同一个理由:东财「行业」(`m:90+t:2`)
/// 把同一个板块在申万两级里各列一行(白酒Ⅱ / 白酒Ⅲ),按 `top_n` 拉完再合并会缺行。
/// 实测 496 个行业板块里有 41 组同名(占 8%),2 倍余量足够;封顶 100 是控制响应体 ——
/// `top_n` 上限 50,合并后仍余下一半。
pub fn industry_prefetch_size(top_n: usize) -> usize {
    (clamp_top_n(top_n) * 2 + 10).clamp(20, 100)
}

impl MarketOverviewClient {
    pub fn new() -> Self {
        Self {
            client: super::shared_client().clone(),
            prev_day: Mutex::new(None),
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

    /// 两市成交额较**上一交易日同一时点**的增量(元):正数=放量,负数=缩量。
    ///
    /// 为什么两边都取腾讯分时、而不是拿上面那个新浪成交额直接减:
    /// 「增量」是个差值,两边必须同源同时刻才成立 —— 混用两家口径,差值里就混进了
    /// 换源误差,而那个误差和真实的增减量同一个量级，看起来完全像真的。
    ///
    /// 盘前、非交易日、接口异常一律返回 `None`(面板据此隐藏这块),不返回 0 ——
    /// 0 会被读成「与昨日持平」。
    pub async fn fetch_turnover_delta(&self) -> Option<f64> {
        let now = beijing_now();
        let today = now.format("%Y%m%d").to_string();
        let hhmm = now.hour() * 100 + now.minute();

        let (today_curves, prev) = tokio::join!(
            async {
                let (sh, sz) = tokio::join!(
                    self.fetch_today_curve(TURNOVER_CODES[0], &today),
                    self.fetch_today_curve(TURNOVER_CODES[1], &today),
                );
                (sh, sz)
            },
            self.prev_day_curves(&today),
        );

        let (Some(sh_today), Some(sz_today)) = today_curves else {
            return None;
        };
        let prev = prev?;

        let today_total =
            cumulative_at(&sh_today, hhmm)? + cumulative_at(&sz_today, hhmm)?;
        let prev_total = cumulative_at(&prev.sh, hhmm)? + cumulative_at(&prev.sz, hhmm)?;
        Some(today_total - prev_total)
    }

    /// 当日分时曲线。接口在盘前/休市时给的是**上一交易日**的整天数据 ——
    /// 用 `date` 对一下,不是今天就当没有,免得把昨天的曲线当成今天。
    async fn fetch_today_curve(&self, code: &str, today: &str) -> Option<AmountCurve> {
        let body = self
            .get_json(&format!("{}{}", TENCENT_MINUTE_QUERY, code), "分时")
            .await?;
        let day = parse_minute_query(&body, code)?;
        (day.date == today).then_some(day.curve)
    }

    /// 前一交易日的两条分时曲线(沪、深),按 UTC+8 日期缓存 —— 昨天不会变,
    /// 没必要每 60s 重拉 100KB。
    async fn prev_day_curves(&self, today: &str) -> Option<Arc<PrevDayCurves>> {
        if let Some(cached) = self.prev_day.lock().ok().and_then(|g| g.clone()) {
            if cached.for_date == today {
                return Some(cached);
            }
        }

        let (sh, sz) = tokio::join!(
            self.fetch_prev_curve(TURNOVER_CODES[0], today),
            self.fetch_prev_curve(TURNOVER_CODES[1], today),
        );
        let curves = Arc::new(PrevDayCurves {
            for_date: today.to_string(),
            sh: sh?,
            sz: sz?,
        });
        if let Ok(mut guard) = self.prev_day.lock() {
            *guard = Some(curves.clone());
        }
        Some(curves)
    }

    async fn fetch_prev_curve(&self, code: &str, today: &str) -> Option<AmountCurve> {
        let body = self
            .get_json(&format!("{}{}", TENCENT_DAY_QUERY, code), "五日分时")
            .await?;
        pick_prev_day(&parse_day_query(&body, code), today).map(|d| d.curve.clone())
    }

    async fn get_json(&self, url: &str, what: &str) -> Option<serde_json::Value> {
        let resp = headers::with_browser_headers(self.client.get(url), "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| log::debug!("[market] {}请求失败: {:#}", what, e))
            .ok()?;

        if !resp.status().is_success() {
            log::debug!("[market] {} HTTP {}", what, resp.status());
            return None;
        }

        resp.json()
            .await
            .map_err(|e| log::debug!("[market] {}解析失败: {}", what, e))
            .ok()
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

        // JSON 合法但结构不符(缺 data.fenbu / 非数组)要当错误向上抛:
        // 兜成 (0,0,0) 会让命令层的降级告警永远不触发,UI 静默显示 -- 且日志无痕。
        parse_breadth(&body)
            .ok_or_else(|| AppError::parse("eastmoney", "涨跌家数响应缺少 data.fenbu"))
    }

    /// 板块排名(行业或概念)。`fs` 为东财筛选串(`m:90+t:2` 行业 / `m:90+t:3` 概念)。
    /// 按 `direction` 排序取前 `top_n` 条返回(条数由设置页配置)。
    ///
    /// 先按 `industry_prefetch_size` 多拉一批,再合并同名板块(见 `merge_duplicate_boards`)
    /// 才截取 —— 直接按 `top_n` 拉,榜首那对「白酒Ⅱ / 白酒Ⅲ」就会吃掉两个名额。
    pub async fn fetch_sector_ranking(
        &self,
        fs: &str,
        direction: &str,
        top_n: usize,
    ) -> Result<Vec<SectorItem>, AppError> {
        let n = clamp_top_n(top_n);
        let items = self
            .fetch_sector_ranking_paged(fs, direction, industry_prefetch_size(n))
            .await?;
        let mut merged = merge_duplicate_boards(items, direction);
        merged.truncate(n);
        Ok(merged)
    }

    /// 概念板块排名 —— 与 `fetch_sector_ranking` 相同,但先多拉一些再剔除
    /// 风格/指数/资金/业绩等「非主题概念」板块,最后截取前 `top_n`。
    ///
    /// 为什么不能 `pz=top_n` 直接过滤:东财 `m:90+t:3` 的「概念」实为主题概念 +
    /// 风格因子(历史新高、微盘股…) + 指数成分(中证500、茅指数…) + 资金持仓的
    /// 大杂烩,若榜首就是这类板块,直接过滤会导致列数不足。
    ///
    /// 预拉倍率的取值理由见 `concept_prefetch_size`。
    pub async fn fetch_concept_ranking(
        &self,
        direction: &str,
        top_n: usize,
    ) -> Result<Vec<SectorItem>, AppError> {
        let n = clamp_top_n(top_n);
        let mut items = self
            .fetch_sector_ranking_paged("m:90+t:3", direction, concept_prefetch_size(n))
            .await?;
        items.retain(|s| !is_non_concept_board(&s.code, &s.name));
        // 概念榜没有申万那套 Ⅱ/Ⅲ 同名板块,但接口偶尔会把同一个板块吐两遍
        // (实测行业榜里 `BK1032 风电设备` 连出两条),合并一下省得榜上出现两行一样的。
        let mut merged = merge_duplicate_boards(items, direction);
        merged.truncate(n);
        Ok(merged)
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
            "{}/api/qt/clist/get?pn=1&pz={}&po={}&np=1&fltt=2&invt=2&fid=f3&fs={}&fields=f12,f14,f3,f128,f136,f207,f222",
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
/// 均返回空。故只能用黑名单兜底。
///
/// 名单只覆盖**已经出现过的那几批**;业绩预告与持仓变化是每期新开一批板块、代码跟着增加
/// (2026一季报预增 → 中报预增 → 三季报预增…),靠代码表必然每期漏掉新的几个 ——
/// 那一半交给 `is_recurring_non_concept` 按名字认。
mod concept_blacklist {
    /// 宽基/成分指数(HS300_、中证500、茅指数…)
    /// + 股份类别/交易机制(AB股、AH股、B股、GDR、科创板做市商/做市股…)
    /// —— 后一类同样是「非主题的机械分类」,和风格因子一样不该出现在概念榜里。
    pub const INDEX: &[&str] = &[
        "BK0498", "BK0499", "BK0500", "BK0568", "BK0610", "BK0611", "BK0612",
        "BK0636", "BK0638", "BK0701", "BK0705", "BK0742", "BK0743", "BK0821",
        "BK0867", "BK0868", "BK0879", "BK0999", "BK1000", "BK1107", "BK1108",
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

    /// 业绩预披露(预增/预减/扭亏/首亏…) —— 历史各期。之后新开的期次(如 2026 三季报)
    /// 代码还没进表,由 `is_recurring_non_concept` 的年份规则兜住。
    pub const EARNINGS: &[&str] = &[
        "BK1198", "BK1199", "BK1628", "BK1680", "BK1681", "BK1682", "BK1749",
        "BK1750", "BK1751", "BK1752",
    ];
}

/// 判断板块是否属于「非主题概念」(风格/指数/资金/业绩),应从概念排名中剔除。
fn is_non_concept_board(code: &str, name: &str) -> bool {
    concept_blacklist::INDEX.contains(&code)
        || concept_blacklist::STYLE.contains(&code)
        || concept_blacklist::FUND.contains(&code)
        || concept_blacklist::EARNINGS.contains(&code)
        || is_recurring_non_concept(name)
}

/// 「每期新开一批」的非主题板块 —— 代码表跟不住,按名字认。
///
/// 东财给每一期业绩预告、每一类持仓变化、每一种市场风格都单开板块,代码随增量发放:
/// 2026 年内的 `一季报预增 → 中报预增 → 三季报预增` 就是三批不同的 `BK` 号,
/// 而三季报那四个(实测 2026-10:`2026三季报预增/预减/首亏/扭亏`)当时都不在黑名单里,
/// 业绩预告季恰好是它们最可能冲上榜首的时候。这几类名字的构词是稳定的:
///
/// - 业绩:四位年份开头 + 含「报」(`2026三季报预增`、`2025年报预减` …)
/// - 持仓:以 重仓 / 新进 / 增仓 / 减仓 / 持股 结尾(`保险重仓`、`社保新进`、`大基金持股` …)
/// - 风格:以「风格」结尾(`消费风格`、`科技风格` …)
/// - 昨日系列:以「昨日」开头(`昨日涨停`、`昨日高换手`、`昨日炸板` …),东财隔一阵就补一种
///
/// 主题概念里没有以这些构词开头/结尾的(`参股券商`、`超级品牌` 之类都不沾),所以按名字剔不会误伤。
fn is_recurring_non_concept(name: &str) -> bool {
    const POSITION_SUFFIXES: &[&str] = &["重仓", "新进", "增仓", "减仓", "持股"];
    const STYLE_SUFFIXES: &[&str] = &["风格"];

    is_earnings_series(name)
        || name.starts_with("昨日")
        || POSITION_SUFFIXES.iter().any(|s| name.ends_with(s))
        || STYLE_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// 「2026三季报预增」这类业绩预告板块:四位年份 + 含「报」。
fn is_earnings_series(name: &str) -> bool {
    // `get(..4)` 在第四个字节不是字符边界时返回 None(中文名几乎都如此),正好直接排除。
    let Some(year) = name.get(..4) else {
        return false;
    };
    year.bytes().all(|b| b.is_ascii_digit()) && name[4..].contains('报')
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

/// 分时累计成交额曲线:`(HHMM, 当日累计成交额(元))`,按时间升序。
type AmountCurve = Vec<(u32, f64)>;

/// 一个交易日的分时曲线
#[derive(Debug, Clone)]
struct DayCurve {
    /// `YYYYMMDD`
    date: String,
    curve: AmountCurve,
}

/// 前一交易日的沪深两条曲线 —— 口径与 `SINA_INDEX_URL` 相同(沪指 + 深证综指)。
struct PrevDayCurves {
    /// 抓取时 UTC+8 的日期。跨日即失效:「昨天」是相对于今天说的。
    for_date: String,
    sh: AmountCurve,
    sz: AmountCurve,
}

/// UTC+8 的当前时间 —— 盘中判断与时间戳都按北京时间,不看本机时区。
fn beijing_now() -> chrono::DateTime<FixedOffset> {
    Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).expect("UTC+8 is a valid offset"))
}

/// 解析腾讯分时的单行:`0930 3671.00 4371035 7416126878.20`
/// → `(930, 7416126878.20)`。第四列是**当日累计**成交额,不是本分钟的量。
fn parse_amount_line(line: &str) -> Option<(u32, f64)> {
    let mut fields = line.split_whitespace();
    let hhmm = fields.next()?.parse::<u32>().ok()?;
    let _price = fields.next()?;
    let _volume = fields.next()?;
    let amount = fields.next()?.parse::<f64>().ok()?;
    Some((hhmm, amount))
}

fn parse_curve(lines: &[serde_json::Value]) -> AmountCurve {
    lines
        .iter()
        .filter_map(|v| parse_amount_line(v.as_str()?))
        .collect()
}

/// `.../minute/query`:`data.<code>.data = { date, data: [...] }`
fn parse_minute_query(body: &serde_json::Value, code: &str) -> Option<DayCurve> {
    let data = body.pointer(&format!("/data/{}/data", code))?;
    Some(DayCurve {
        date: data.get("date")?.as_str()?.to_string(),
        curve: parse_curve(data.get("data")?.as_array()?),
    })
}

/// `.../day/query`:`data.<code>.data = [{ date, data: [...] }, ...]`(最近五个交易日)
fn parse_day_query(body: &serde_json::Value, code: &str) -> Vec<DayCurve> {
    let Some(days) = body
        .pointer(&format!("/data/{}/data", code))
        .and_then(|d| d.as_array())
    else {
        return Vec::new();
    };
    days.iter()
        .filter_map(|d| {
            Some(DayCurve {
                date: d.get("date")?.as_str()?.to_string(),
                curve: parse_curve(d.get("data")?.as_array()?),
            })
        })
        .collect()
}

/// 取 `hhmm` 时点的累计成交额 —— 最后一条时间 ≤ hhmm 的记录。
/// 午休时 11:30 之后没有新行,自然停在 11:30,正是要和昨天 11:30 对比的那个数。
fn cumulative_at(curve: &[(u32, f64)], hhmm: u32) -> Option<f64> {
    curve
        .iter()
        .rev()
        .find(|(t, _)| *t <= hhmm)
        .map(|(_, amount)| *amount)
}

/// 从五日分时里挑**前一交易日**。接口按时间**倒序**给(最新的一天在前):
/// 今天已经出现时,昨天就在它后一条;今天还没出现(盘前 / 休市)时最新那条就是
/// 前一交易日 —— 那种情况下今天本来也拿不到曲线,调用方会放弃对比。
fn pick_prev_day<'a>(days: &'a [DayCurve], today: &str) -> Option<&'a DayCurve> {
    match days.iter().position(|d| d.date == today) {
        Some(i) => days.get(i + 1),
        None => days.first(),
    }
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
/// 字段:f12=代码 f14=名称 f3=涨跌幅% f128=领涨股 f136=领涨股涨跌幅%
///       f207=领跌股 f222=领跌股涨跌幅%
///
/// 为什么两个都要:跌幅榜里 f128 返回的仍是该板块**涨得最好**的成分股
/// (实测 2026-09-15 种子板块 -6.93%,f128=ST荃银 -1.59%),直接把它标成
/// 「领跌」会张冠李戴;真正的领跌股在 f207(同板块 f207=国投丰乐 -9.99%)。
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
                laggard_name: item.get("f207").and_then(|v| v.as_str()).map(|s| s.to_string()),
                laggard_pct: item.get("f222").and_then(|v| v.as_f64()),
            })
        })
        .collect()
}

/// 板块名称末尾的申万层级罗马数字 —— 返回 (层级, 去掉后缀的本名)。
/// 无后缀记 0(申万一级,或该板块只出现过一层)。
fn split_board_level(name: &str) -> (u8, &str) {
    for (suffix, level) in [("Ⅳ", 4u8), ("Ⅲ", 3), ("Ⅱ", 2), ("Ⅰ", 1)] {
        if let Some(base) = name.strip_suffix(suffix) {
            return (level, base);
        }
    }
    (0, name)
}

/// 合并同名板块 —— 东财的「行业」榜(`m:90+t:2`)把申万一级/二级/三级行业混在同一张榜里,
/// 同一个板块在两级里各占一行,靠末尾的 Ⅱ/Ⅲ 区分:实测「白酒Ⅱ」与「白酒Ⅲ」的成分股
/// 完全一致(up/down/flat 与涨跌幅逐项相同),排名靠前时就是同一行内容重复两遍。
/// 这里按去掉后缀的本名合并,名称也还原成「白酒」—— 榜单是给人看排行,不是给人看分类层级。
///
/// 保留哪一只:层级更高(数字更小)的那只。二级是三级的上位集合,两者成分股偶尔并不相同
/// (`其他电源设备Ⅱ` 36 只 / `其他电源设备Ⅲ` 30 只),取上位的那只才不会漏掉成分股。
///
/// 合并后重排:替换代表项会让涨跌幅微调,不重排榜单就看起来失序。
fn merge_duplicate_boards(items: Vec<SectorItem>, direction: &str) -> Vec<SectorItem> {
    let mut kept: Vec<(SectorItem, u8)> = Vec::with_capacity(items.len());
    let mut index: HashMap<String, usize> = HashMap::new();

    for mut item in items {
        let (level, base) = split_board_level(&item.name);
        let base = base.to_string();
        item.name = base.clone();
        match index.get(&base) {
            Some(&pos) => {
                if level < kept[pos].1 {
                    kept[pos] = (item, level);
                }
            }
            None => {
                index.insert(base, kept.len());
                kept.push((item, level));
            }
        }
    }

    let mut merged: Vec<SectorItem> = kept.into_iter().map(|(item, _)| item).collect();
    sort_by_change(&mut merged, direction);
    merged
}

/// 按涨跌幅重排。`direction` 为 `"down"` 时升序(跌得最深在前),其余降序 ——
/// 与 `fetch_sector_ranking_paged` 里 `po` 的取值方向一致。
fn sort_by_change(items: &mut [SectorItem], direction: &str) {
    let descending = direction != "down";
    items.sort_by(|a, b| {
        let ord = a
            .change_pct
            .partial_cmp(&b.change_pct)
            .unwrap_or(std::cmp::Ordering::Equal);
        if descending { ord.reverse() } else { ord }
    });
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
    fn clamp_top_n_bounds_the_configurable_sector_count() {
        assert_eq!(clamp_top_n(0), SECTOR_TOP_N_MIN, "0 条无意义，夹到下限");
        assert_eq!(clamp_top_n(5), 5);
        assert_eq!(clamp_top_n(50), 50);
        assert_eq!(clamp_top_n(999), SECTOR_TOP_N_MAX, "超限夹到上限");
    }

    /// 预拉量必须**严格大于**所需条数，否则概念榜过滤掉非主题板块后会缺列。
    /// 同时封顶 200：设置页允许填到 50，无上限的话 pz 会随配置线性放大，
    /// 东财那个接口的响应体也跟着涨。
    #[test]
    fn concept_prefetch_size_leaves_headroom_and_is_capped() {
        for n in SECTOR_TOP_N_MIN..=SECTOR_TOP_N_MAX {
            assert!(
                concept_prefetch_size(n) > n,
                "预拉 {} 条不足以在过滤后凑出 {} 条",
                concept_prefetch_size(n),
                n
            );
        }
        assert_eq!(concept_prefetch_size(1), 50, "小值走 50 下限");
        assert_eq!(concept_prefetch_size(5), 50);
        assert_eq!(concept_prefetch_size(10), 100);
        assert_eq!(concept_prefetch_size(50), 200, "大值封顶 200");
        assert_eq!(concept_prefetch_size(999), 200, "越界先夹到 50 再算");
    }

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
                {"f12": "BK1556", "f14": "教育运营及其他", "f3": 6.59, "f128": "ST豆神",
                 "f136": 12.92, "f207": "博瑞传播", "f222": 0.41}
            ]}
        });
        let items = parse_clist(&body);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "教育运营及其他");
        assert_eq!(items[0].change_pct, 6.59);
        assert_eq!(items[0].leader_name.as_deref(), Some("ST豆神"));
        assert_eq!(items[0].leader_pct, Some(12.92));
        assert_eq!(items[0].laggard_name.as_deref(), Some("博瑞传播"));
        assert_eq!(items[0].laggard_pct, Some(0.41));
    }

    /// 跌幅榜里 f128 仍是板块内**涨得最好**的股票,不能当领跌股用
    /// (实测 2026-09-15 种子板块 -6.93%:f128=ST荃银 -1.59%,f207=国投丰乐 -9.99%)。
    #[test]
    fn parses_clist_laggard_is_not_the_leader() {
        let body = serde_json::json!({
            "data": { "diff": [
                {"f12": "BK0433", "f14": "种子", "f3": -6.93, "f128": "ST荃银",
                 "f136": -1.59, "f207": "国投丰乐", "f222": -9.99}
            ]}
        });
        let items = parse_clist(&body);
        assert_eq!(items[0].leader_name.as_deref(), Some("ST荃银"));
        assert_eq!(items[0].laggard_name.as_deref(), Some("国投丰乐"));
        assert_ne!(items[0].leader_name, items[0].laggard_name);
    }

    #[test]
    fn concept_blacklist_flags_non_concept_boards() {
        // 风格因子、指数成分、资金持仓、业绩预披露 → 剔除
        assert!(is_non_concept_board("BK1675", "历史新高")); // 风格
        assert!(is_non_concept_board("BK1639", "周期股")); // 风格
        assert!(is_non_concept_board("BK0500", "HS300_")); // 指数
        assert!(is_non_concept_board("BK0999", "茅指数")); // 指数
        assert!(is_non_concept_board("BK0536", "基金重仓")); // 资金
        assert!(is_non_concept_board("BK0707", "沪股通")); // 资金
        assert!(is_non_concept_board("BK1749", "2026中报预增")); // 业绩
        // 股份类别 / 交易机制 → 与 AB股、B股 同类,同样剔除
        assert!(is_non_concept_board("BK0498", "AB股"));
        assert!(is_non_concept_board("BK0868", "GDR"));
        assert!(is_non_concept_board("BK1107", "科创板做市商"));
        assert!(is_non_concept_board("BK1108", "科创板做市股"));
        // 主题概念 → 保留
        assert!(!is_non_concept_board("BK0917", "半导体概念"));
        assert!(!is_non_concept_board("BK0800", "人工智能"));
        assert!(!is_non_concept_board("BK0900", "新能源车"));
        // 区域/参股主题是东财口径下的概念板块,不应误剔
        assert!(!is_non_concept_board("BK0813", "雄安新区"));
        assert!(!is_non_concept_board("BK0514", "参股券商"));
    }

    /// 代码表跟不住的那几类:东财每期新开一批板块、代码跟着变。
    /// 下面的代码都不在黑名单里(实测 2026-10),只能靠名字认。
    #[test]
    fn recurring_series_are_flagged_by_name_without_being_listed() {
        // 业绩预告:2026 三季报那四个当时一个都没在名单里
        assert!(is_non_concept_board("BK1755", "2026三季报预增"));
        assert!(is_non_concept_board("BK1756", "2026三季报预减"));
        assert!(is_non_concept_board("BK1757", "2026三季报扭亏"));
        assert!(is_non_concept_board("BK1754", "2026三季报首亏"));
        assert!(is_non_concept_board("BK9999", "2027年报预增"), "明年那批也认得出");
        // 持仓变化
        assert!(is_non_concept_board("BK1685", "保险重仓"));
        assert!(is_non_concept_board("BK1686", "保险新进"));
        assert!(is_non_concept_board("BK1689", "基金减仓"));
        assert!(is_non_concept_board("BK1690", "基金增仓"));
        assert!(is_non_concept_board("BK1694", "社保新进"));
        assert!(is_non_concept_board("BK1696", "私募新进"));
        assert!(is_non_concept_board("BK1697", "信托重仓"));
        assert!(is_non_concept_board("BK1688", "大基金持股"));
        // 风格
        assert!(is_non_concept_board("BK9998", "小盘风格"));
        // 昨日系列:名单里有 7 个变体,东财隔一阵还会补新的
        assert!(is_non_concept_board("BK9997", "昨日炸板"));
    }

    /// 按名字剔的口子不能开太大 —— 主题概念里没有以这些构词结尾的。
    #[test]
    fn name_rules_do_not_swallow_theme_boards() {
        for name in [
            "参股券商", "参股银行", "超级品牌", "稀缺资源", "中特估",
            "消费电子概念", "电池技术", "油气资源", "谷子经济", "首发经济",
            "北交所概念", "融资融券", "转债标的", "养老金", "券商金股",
        ] {
            assert!(!is_recurring_non_concept(name), "{} 被名字规则误剔", name);
        }
        // 四是四位年份开头的才认:3D 开头、少一位年份都不算业绩预告
        assert!(!is_recurring_non_concept("3D打印"));
        assert!(!is_recurring_non_concept("2026规划"));
    }

    fn board(code: &str, name: &str, pct: f64) -> SectorItem {
        SectorItem {
            code: code.to_string(),
            name: name.to_string(),
            change_pct: pct,
            leader_name: None,
            leader_pct: None,
            laggard_name: None,
            laggard_pct: None,
        }
    }

    /// 白酒Ⅱ / 白酒Ⅲ 是同一个板块在申万两级里的两次出现(成分股完全一致,
    /// 实测 2026-10),合并后只留一行,名称也不带后缀。
    #[test]
    fn merges_same_named_boards_across_levels() {
        let merged = merge_duplicate_boards(
            vec![
                board("BK1575", "白酒Ⅲ", 1.22),
                board("BK1277", "白酒Ⅱ", 1.22),
                board("BK1496", "洗护用品", 0.98),
            ],
            "up",
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].name, "白酒");
        assert_eq!(merged[0].code, "BK1277", "保留层级更高的那只(Ⅱ)");
        assert_eq!(merged[1].name, "洗护用品");
    }

    /// 只有一层分类的板块没有孪生项,后缀照样去掉 —— 榜单上没有层级信息的位置。
    #[test]
    fn strips_level_suffix_without_a_twin() {
        let merged = merge_duplicate_boards(vec![board("BK1612", "农商行Ⅲ", -0.46)], "down");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "农商行");
    }

    /// 两个层级的成分股偶尔并不相同(其他电源设备Ⅱ 36 只 / Ⅲ 30 只),
    /// 取上位的那只,并按它的涨跌幅重排 —— 否则替换代表项后榜单会失序。
    #[test]
    fn keeps_the_upper_level_board_and_resorts() {
        let merged = merge_duplicate_boards(
            vec![
                board("BK0003", "风电设备", -2.09),
                board("BK0002", "其他电源设备Ⅲ", -3.44),
                board("BK0001", "其他电源设备Ⅱ", -3.23),
            ],
            "down",
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].name, "其他电源设备", "跌幅榜里深的一只排前面");
        assert_eq!(merged[0].code, "BK0001");
        assert_eq!(merged[0].change_pct, -3.23, "用的是上位板块自己的涨跌幅");
        assert_eq!(merged[1].name, "风电设备");
    }

    /// 预拉量必须大于所需条数,否则合并同名板块后会缺列;同时封顶 100 控制响应体。
    #[test]
    fn industry_prefetch_size_leaves_headroom_and_is_capped() {
        for n in SECTOR_TOP_N_MIN..=SECTOR_TOP_N_MAX {
            assert!(
                industry_prefetch_size(n) > n,
                "预拉 {} 条不足以在合并后凑出 {} 条",
                industry_prefetch_size(n),
                n
            );
        }
        assert_eq!(industry_prefetch_size(1), 20, "小值走 20 下限");
        assert_eq!(industry_prefetch_size(10), 30);
        assert_eq!(industry_prefetch_size(50), 100, "大值封顶 100");
        assert_eq!(industry_prefetch_size(999), 100, "越界先夹到 50 再算");
    }

    fn day(date: &str, curve: AmountCurve) -> DayCurve {
        DayCurve {
            date: date.to_string(),
            curve,
        }
    }

    /// 腾讯分时一行是「时间 点位 累计成交量 累计成交额」,第四列才是成交额;
    /// 第三列是累计手数,用错会让「增量」变成一个荒谬的量级。
    #[test]
    fn parses_tencent_minute_amount_line() {
        let (t, a) = parse_amount_line("0930 3671.00 4371035 7416126878.20").unwrap();
        assert_eq!(t, 930);
        assert!((a - 7_416_126_878.20).abs() < 0.01);
        assert_eq!(parse_amount_line("坏行"), None);
        assert_eq!(parse_amount_line("0930 3671.00"), None);
    }

    #[test]
    fn parses_minute_and_day_payloads() {
        let minute = serde_json::json!({
            "data": { "sh000001": { "data": {
                "date": "20261027",
                "data": ["0930 3671.00 4371035 7416126878.20", "0931 3674.25 20772190 35232785418.20"]
            }}}
        });
        let parsed = parse_minute_query(&minute, "sh000001").unwrap();
        assert_eq!(parsed.date, "20261027");
        assert_eq!(parsed.curve.len(), 2);
        assert_eq!(parsed.curve[1].0, 931);

        let five = serde_json::json!({
            "data": { "sh000001": { "data": [
                { "date": "20261027", "data": ["0930 3671.00 4371035 7416126878.20"] },
                { "date": "20261026", "data": ["0930 3671.00 4371035 7416126878.20", "1500 3697.52 432794318 746528025716.40"] }
            ]}}
        });
        let days = parse_day_query(&five, "sh000001");
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, "20261027", "接口按时间倒序给");
        assert_eq!(days[1].curve.len(), 2);

        // 码不对 / 结构不对都不该 panic,给空
        assert!(parse_day_query(&five, "sz399106").is_empty());
        assert!(parse_minute_query(&serde_json::json!({"data": {}}), "sh000001").is_none());
    }

    /// 午休与收盘后都要落在正确的那一行上 —— 这是「同时点」对比的全部依据。
    #[test]
    fn cumulative_at_uses_last_point_at_or_before_time() {
        let curve = vec![
            (930, 100.0),
            (935, 200.0),
            (1130, 900.0),
            (1300, 950.0),
            (1500, 1000.0),
        ];
        assert_eq!(cumulative_at(&curve, 930), Some(100.0));
        assert_eq!(cumulative_at(&curve, 940), Some(200.0), "取 <= t 的最后一条");
        assert_eq!(cumulative_at(&curve, 1200), Some(900.0), "午休停在 11:30");
        assert_eq!(cumulative_at(&curve, 1600), Some(1000.0), "收盘后是全天值");
        assert_eq!(cumulative_at(&curve, 925), None, "开盘前没有可比时点");
        assert_eq!(cumulative_at(&[], 1000), None);
    }

    /// 五日分时按时间倒序,「昨天」在今天**后面**一条;今天还没出现时最新那条就是昨天。
    #[test]
    fn picks_previous_trading_day() {
        let days = vec![
            day("20261027", vec![(930, 1.0)]),
            day("20261026", vec![(930, 2.0)]),
            day("20261023", vec![(930, 3.0)]),
        ];
        assert_eq!(
            pick_prev_day(&days, "20261027").map(|d| d.date.as_str()),
            Some("20261026")
        );
        assert_eq!(
            pick_prev_day(&days, "20261026").map(|d| d.date.as_str()),
            Some("20261023")
        );
        // 今天还没出现在五日分时里 → 最新那条就是「昨天」
        assert_eq!(
            pick_prev_day(&days, "20261028").map(|d| d.date.as_str()),
            Some("20261027")
        );
        assert!(pick_prev_day(&[], "20261027").is_none());
        assert!(pick_prev_day(&days[..1], "20261027").is_none(), "只有今天一条时没有基准");
    }

    #[test]
    fn count_parses_int_and_float() {
        assert_eq!(parse_count(Some(&serde_json::json!(1313))), 1313);
        assert_eq!(parse_count(Some(&serde_json::json!(1313.0))), 1313);
        assert_eq!(parse_count(Some(&serde_json::json!("x"))), 0);
        assert_eq!(parse_count(None), 0);
    }
}
