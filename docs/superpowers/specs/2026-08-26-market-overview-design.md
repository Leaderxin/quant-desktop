# 市场概览面板设计(成交额 / 涨跌家数 / 板块排名)

日期:2026-08-26

## 背景与目标

主界面在指数卡片行下方新增一个**可折叠的市场概览面板**,聚合三块全市场数据:

1. 沪深两市总成交额
2. A股涨跌家数(涨 / 跌 / 平)
3. 板块涨跌排名(行业板块 Top5 + 概念板块 Top5,可切换涨/跌方向)

指数卡片行**保留常显**,三块信息**整体折叠**成一行标题栏。

## 数据源决策(本机实测验证)

| 数据 | 来源 | 端点 | 状态 |
|------|------|------|------|
| 总成交额 | 新浪 | `hq.sinajs.cn/list=sh000001,sz399106`(GBK,第9字段=成交额) | ✅ |
| 涨跌家数 | 东财 | `push2ex.eastmoney.com/getTopicZDFenBu`(涨跌分布自算) | ✅ |
| 行业板块 | 东财 | `push2delay.eastmoney.com/api/qt/clist/get` `fs=m:90+t:2` | ✅ |
| 概念板块 | 东财 | `push2delay.eastmoney.com/api/qt/clist/get` `fs=m:90+t:3` | ✅ |

### 关键域名坑(本环境特有)

| 域名 | 状态 |
|------|------|
| `push2.eastmoney.com`(akshare 默认) | ❌ 被拦截,clist 返回 HTTP 000 / `curl (52) Empty reply` |
| `push2delay.eastmoney.com` | ✅ 完全可用(板块列表) |
| `push2ex.eastmoney.com` | ✅ 可用(涨跌分布) |
| `hq.sinajs.cn` / `qt.gtimg.cn` | ✅ 可用 |

因此 akshare 的 `stock_board_industry_name_em()` 在本环境会因写死 `push2` 而报错。

### 成交额口径

用**深证综指 `sz399106`**(覆盖深市全市场),不用深证成指 `sz399001`(成分股指数,会少算)。实测:沪 8572.23 亿 + 深 9515.00 亿 = 1.81 万亿。

## 后端设计

### `datasource/market.rs` —— `MarketOverviewClient`

独立聚合客户端,**不实现 `DataSource` trait**(不提供个股报价,与报价数据源解耦)。复用 `shared_client()` 和 `headers::with_browser_headers`。共 4 个 HTTP 请求:

```rust
pub struct MarketOverviewClient { client: Client }

impl MarketOverviewClient {
    // 新浪指数 → 上证指数 + 深证综指 成交额相加(元)
    async fn fetch_total_turnover(&self) -> Result<f64, AppError>;

    // 东财 push2ex getTopicZDFenBu → (涨, 跌, 平)
    async fn fetch_market_breadth(&self) -> Result<(u32, u32, u32), AppError>;

    // 东财 push2delay clist,fs 行业/概念,po 控制涨跌方向,取前 5
    async fn fetch_sector_ranking(&self, fs: &str, direction: &str)
        -> Result<Vec<SectorItem>, AppError>;
}
```

- 涨跌分布解析:`data.fenbu` 数组,元素 key 为涨跌幅区间(负=跌/0=平/正=涨),value 为家数,累加。
- 板块字段:`f12`=代码 `f14`=名称 `f3`=涨跌幅% `f128`=领涨股 `f136`=领涨股涨跌幅%。
- 方向:`po=1` 降序(涨幅榜)、`po=0` 升序(跌幅榜)。

### 新增 domain 类型

```rust
pub struct SectorItem { code, name, change_pct, leader_name: Option<String>, leader_pct: Option<f64> }
pub struct MarketOverview { turnover: f64, up: u32, down: u32, flat: u32,
                            industry: Vec<SectorItem>, concept: Vec<SectorItem> }
```

### `commands/market.rs` —— `get_market_overview(direction)`

四块数据来源独立,任一失败只降级对应字段(0 / 空列表),面板整体不消失。

### `lib.rs`

- `app.manage(Arc<MarketOverviewClient>)`
- 注册 `get_market_overview`
- `NO_PROXY` 追加 `eastmoney.com`

## 前端设计

### `components/market/MarketOverviewPanel.vue`

位于 `IndexBar` 与 `WatchlistTable` 之间。指数行保留常显,面板本体整体折叠:

- **收起态**:标题栏(标题 + 成交额 + 涨跌家数摘要 + 展开箭头),隐藏涨/跌切换与两个 Top5 列表。
- **展开态**:涨/跌切换 + 行业/概念两个 Top5 列表(红涨绿跌,领涨股在右侧)。
- 涨/跌切换 → 重新 `invoke('get_market_overview', { direction })`。
- 自动刷新:展开时 60s 一次,收起时暂停。
- 折叠状态默认展开(未持久化)。

### `stores/market.ts`

`useMarketStore` 持有 `overview / direction / expanded / loading / error`,提供 `fetchOverview / toggleDirection / setExpanded / startRefresh / stopRefresh`。

### 类型与工具

- `types/index.ts`:`SectorItem`、`MarketOverview`
- `utils/format.ts`:`formatAmount`(元 → 万亿/亿/万)

## 错误处理

任一数据源失败 → 对应字段降级(成交额 0、涨跌家数 0/0/0、板块空列表),前端显示 `--` 或"暂不可用"。后端 `log::warn` 记录。

## 测试

Rust 单测(纯函数,无网络):新浪成交额解析、东财涨跌分布解析、clist 板块解析、计数字段容错。`cargo test --lib` 全绿(23 个)。

## 文件清单

新增 5:
- `src-tauri/src/datasource/market.rs`
- `src-tauri/src/commands/market.rs`
- `src/components/market/MarketOverviewPanel.vue`
- `src/stores/market.ts`
- 本 spec

修改 7:
- `src-tauri/src/domain/mod.rs`
- `src-tauri/src/datasource/mod.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`
- `src/types/index.ts`
- `src/utils/format.ts`
- `src/components/layout/AppLayout.vue`

(顺带修复既有 bug:`src-tauri/src/datasource/tencent.rs` 单测传 `true` → `Some(1)`,原任何 `cargo test` 都会编译失败。)
