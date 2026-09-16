use tauri::State;
use std::sync::Arc;
use crate::datasource::market::MarketOverviewClient;
use crate::datasource::market_clock::MarketSession;
use crate::domain::MarketOverview;

/// 市场概览的建议轮询间隔(秒),按时段由 `market_clock` 给出。
///
/// 前端用它决定下一次刷新的排期。为什么不全靠 `market-session-changed` 事件:
/// 该事件只在时段**切换**时推送,应用启动时不会补发,所以需要这个命令做初始种子。
#[tauri::command]
pub fn get_overview_interval() -> u64 {
    MarketSession::current().overview_interval()
}

/// 市场概览聚合:总成交额 + 涨跌家数 + 行业/概念板块排名。
///
/// 四块数据来源独立(新浪成交额 + 东财涨跌家数 + 东财行业/概念板块),
/// 任一失败只降级对应字段,面板整体不因单一数据源失败而消失。
#[tauri::command]
pub async fn get_market_overview(
    direction: String,
    client: State<'_, Arc<MarketOverviewClient>>,
) -> Result<MarketOverview, String> {
    // 并行发起:四个请求共用一个 10s 超时的 client,串行时一个慢端点会把整份
    // 概览拖到最坏 ~40s;join! 并发等待后总耗时约等于最慢的那一个请求。
    let (turnover, breadth, industry, concept) = tokio::join!(
        client.fetch_total_turnover(),
        client.fetch_market_breadth(),
        client.fetch_sector_ranking("m:90+t:2", &direction),
        client.fetch_concept_ranking(&direction),
    );

    let turnover = turnover.unwrap_or_else(|e| {
        log::warn!("[market] 成交额获取失败,降级为 0: {}", e);
        0.0
    });

    let (up, down, flat) = breadth.unwrap_or_else(|e| {
        log::warn!("[market] 涨跌家数获取失败,降级为 0: {}", e);
        (0, 0, 0)
    });

    let industry = industry.unwrap_or_else(|e| {
        log::warn!("[market] 行业板块获取失败,降级为空: {}", e);
        Vec::new()
    });

    let concept = concept.unwrap_or_else(|e| {
        log::warn!("[market] 概念板块获取失败,降级为空: {}", e);
        Vec::new()
    });

    Ok(MarketOverview {
        turnover,
        up,
        down,
        flat,
        industry,
        concept,
    })
}

/// Fetch only breadth and retain errors so the ticker can mark stale values.
#[tauri::command]
pub async fn get_ticker_breadth(client: State<'_, Arc<MarketOverviewClient>>) -> Result<(u32, u32, u32), String> {
    client.fetch_market_breadth().await.map_err(|e| e.to_string())
}
