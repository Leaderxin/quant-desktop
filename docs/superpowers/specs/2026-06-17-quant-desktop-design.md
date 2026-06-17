# QuantDesktop — 跨平台桌面股票看盘软件 设计文档

> 创建日期: 2026-06-17
> 状态: 设计评审通过

---

## 1. 产品概述

### 1.1 产品定位

一款轻量级、跨平台的桌面股票看盘工具。A 股投资者日常盯盘利器，像 Windows 天气一样静默驻留在桌面底部，需要时一键唤起完整看盘界面。

### 1.2 核心功能

| 优先级 | 功能 | 描述 |
|--------|------|------|
| P0 | 行情条悬浮窗 | 贴任务栏上方，滚动显示自选股票最新价和涨跌幅 |
| P0 | 系统托盘驻留 | 托盘图标常驻，双击唤起主界面，右键菜单操作 |
| P0 | 指数看板 | 主要指数（上证/深证/创业板/科创50）实时行情 |
| P0 | 自选列表 | 表格展示自选标的行情，支持增删排序 |
| P1 | 模糊搜索添加 | 按股票代码或名称模糊搜索，添加到自选 |
| P1 | 个股详情页 | 分时图 + 五档盘口 + 基本面概要 |
| P1 | 涨跌排序筛选 | 自选列表按涨跌幅/价格/成交量排序 |
| P1 | 多数据源切换 | 东方财富（默认）/新浪/腾讯，设置界面一键切换 |
| P1 | 日间/夜间主题 | Naive UI 原生主题切换，CSS 变量驱动 |
| P2 | K 线图 | 日/周/月 K 线 + 技术指标叠加（MA/BOLL/MACD） |
| P2 | 价格预警通知 | 自选标的突破预设价格时托盘通知 |
| P2 | 开机自启 | 系统启动时自动运行 |

### 1.3 市场范围

- **Phase 1（当前）**：A 股（沪深北）
- **Phase 2（预留）**：港股（HK）、美股（US）
- 架构通过 `Market` 枚举和 `DataSource` trait 预留扩展点

---

## 2. 技术栈

### 2.1 整体选型

| 层级 | 技术 | 理由 |
|------|------|------|
| 桌面框架 | Tauri 2.x | 轻量(~10MB)，Rust 后端，系统托盘原生支持 |
| 前端框架 | Vue 3 + TypeScript | 中文社区活跃，Tauri 集成成熟 |
| UI 组件库 | Naive UI | Tree-shaking 好，暗色/亮色主题原生支持 |
| 图表库 | KLineChart | A 股场景专用，分时+K 线一体，20+ 指标内置 |
| 状态管理 | Pinia | Vue 3 官方推荐 |
| 构建工具 | Vite | 快速 HMR，Tauri 官方推荐 |
| 后端语言 | Rust | 性能好，类型安全，与 Tauri 深度集成 |
| HTTP 客户端 | reqwest + tokio | Rust 生态标准异步 HTTP |
| 数据库 | SQLite (rusqlite) | 嵌入式，零配置，足够轻量 |

### 2.2 版本要求

- Rust: stable 1.75+
- Node.js: 20 LTS+
- Tauri CLI: 2.x
- 目标平台: Windows 10+ / macOS 12+ / Linux (Wayland/X11)

---

## 3. 架构设计

### 3.1 分层架构

```
┌── Vue 3 前端层 ──────────────────────────────────────────┐
│  主界面 / 行情条 / 托盘菜单 / 设置                         │
├── Tauri Bridge (invoke / listen) ─────────────────────────┤
├── Rust 核心层 ────────────────────────────────────────────┤
│  Commands → Domain → Cache → DataSource Trait             │
├── 持久化 ─────────────────────────────────────────────────┤
│  SQLite: watchlist / settings / quote_cache                │
└───────────────────────────────────────────────────────────┘
```

**设计原则**：
- 前后端通过 Tauri `invoke` 通信，不引入 HTTP/WS 中间层
- 数据源 trait 是所有行情数据的统一抽象，切换数据源只需改配置
- Cache 层对前端透明，command 始终返回缓存或最新数据
- 行情条是独立 Tauri 窗口（undecorated + always-on-bottom），通过 Tauri event 接收数据推送
- `Market` 字段当前固定 "CN"，预留 "HK"/"US"

### 3.2 Rust 核心模块

```
src-tauri/src/
├── main.rs              # 入口：托盘 + 窗口 + 轮询启动
├── commands/            # Tauri command handlers (薄路由层)
│   ├── quote.rs         # get_quotes, get_indices, get_depth, get_intraday
│   ├── watchlist.rs     # get_watchlist, add_watch, remove_watch, reorder
│   ├── search.rs        # search_stocks
│   └── settings.rs      # get_settings, set_setting
├── datasource/          # 数据源抽象 + 适配器
│   ├── mod.rs           # DataSource trait + DataSourceManager
│   ├── eastmoney.rs     # 东方财富适配器（默认）
│   ├── sina.rs          # 新浪适配器（备用）
│   └── tencent.rs       # 腾讯适配器（预留）
├── domain/              # 领域模型
│   ├── mod.rs
│   ├── quote.rs         # Quote, IndexQuote, Depth, MinuteData
│   ├── market.rs        # Market enum (CN/HK/US)
│   └── stock.rs         # StockBrief
├── cache/               # 缓存 + 轮询
│   ├── mod.rs           # QuoteCache (内存 HashMap + SQLite)
│   └── scheduler.rs     # 定时轮询调度器
├── db/                  # 持久化
│   ├── mod.rs           # 连接管理 + schema 迁移
│   └── models.rs        # WatchItem, Setting, CachedQuote
└── tray.rs              # 托盘创建 + 右键菜单逻辑
```

### 3.3 Vue 3 前端结构

```
src/
├── App.vue
├── main.ts
├── assets/styles/
│   ├── variables.css
│   ├── dark.css
│   └── light.css
├── components/
│   ├── layout/
│   │   ├── AppLayout.vue        # 主界面布局
│   │   └── TopBar.vue           # 顶栏
│   ├── index/
│   │   ├── IndexBar.vue         # 指数横条
│   │   └── IndexCard.vue        # 单指数卡片
│   ├── watchlist/
│   │   ├── WatchlistTable.vue   # 自选列表表格
│   │   ├── WatchRow.vue         # 单行行情
│   │   └── AddStockDialog.vue   # 添加自选弹窗 (模糊搜索)
│   ├── detail/
│   │   ├── StockDetail.vue      # 个股详情容器
│   │   ├── MinuteChart.vue      # 分时图 (KLineChart)
│   │   ├── DepthPanel.vue       # 五档盘口
│   │   └── StockSummary.vue     # 基本面概要
│   ├── ticker/
│   │   ├── TickerBar.vue        # 行情条窗口 (独立入口)
│   │   └── TickerItem.vue       # 单条滚动项
│   └── settings/
│       ├── SettingsDialog.vue
│       ├── DatasourceTab.vue
│       └── GeneralTab.vue
├── stores/                      # Pinia
│   ├── quote.ts
│   ├── watchlist.ts
│   ├── settings.ts
│   └── theme.ts
├── composables/
│   ├── useTauriEvent.ts
│   ├── usePolling.ts
│   └── useTheme.ts
└── types/
    └── index.ts                 # 与 Rust domain 类型对应
```

### 3.4 组件树

```
App.vue
├─ AppLayout.vue
│   ├─ TopBar.vue (logo + 数据源 + 刷新间隔 + 设置入口 + 主题切换)
│   ├─ IndexBar.vue
│   │   └─ IndexCard.vue × N
│   ├─ WatchlistTable.vue
│   │   └─ WatchRow.vue × N (点击展开 StockDetail)
│   ├─ StockDetail.vue (展开面板)
│   │   ├─ MinuteChart.vue
│   │   ├─ DepthPanel.vue
│   │   └─ StockSummary.vue
│   └─ AddStockDialog.vue (模糊搜索 + 添加确认)
├─ TickerBar.vue (独立窗口)
│   └─ TickerItem.vue × N
└─ SettingsDialog.vue
    ├─ GeneralTab.vue
    └─ DatasourceTab.vue
```

不引入 Vue Router —— 单窗口应用，用条件渲染切换视图。

---

## 4. 领域模型

### 4.1 Rust 核心类型

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Market {
    CN,  // A股
    HK,  // 港股 (Phase 2)
    US,  // 美股 (Phase 2)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub code: String,
    pub market: Market,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub turnover: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexQuote {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub volume: u64,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Depth {
    pub code: String,
    pub bids: Vec<Level>,   // 买五
    pub asks: Vec<Level>,   // 卖五
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub price: f64,
    pub volume: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteData {
    pub time: String,       // "09:30"
    pub price: f64,
    pub volume: u64,
    pub avg_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBrief {
    pub code: String,
    pub market: Market,
    pub name: String,
}
```

### 4.2 DataSource Trait

```rust
#[async_trait]
pub trait DataSource: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;

    async fn fetch_realtime(
        &self, codes: &[String], market: Market
    ) -> Result<Vec<Quote>>;

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>>;
    async fn search(&self, keyword: &str, market: Market) -> Result<Vec<StockBrief>>;
    async fn fetch_intraday(&self, code: &str, market: Market) -> Result<Vec<MinuteData>>;
    async fn fetch_depth(&self, code: &str, market: Market) -> Result<Depth>;
    async fn health_check(&self) -> Result<bool>;
}

pub struct DataSourceManager {
    sources: HashMap<String, Box<dyn DataSource>>,
    active: RwLock<String>,
    cache: Arc<QuoteCache>,
}
```

- `DataSourceManager` 持有所有注册的适配器，`active` 指向当前使用的源
- 切换数据源只需改 `active` 值，前端无感知
- `QuoteCache` 在 manager 层实现，各适配器不关心缓存逻辑
- 批量请求：同一批 codes 一次调用获取（国内免费 API 支持批量）

### 4.3 TypeScript 对应类型

`src/types/index.ts` 中定义与 Rust 对应的 interface，通过 `@tauri-apps/api` 的 `invoke<T>` 获取类型安全的前端数据。

---

## 5. 数据流

### 5.1 全局轮询流程

```
┌─ Scheduler (tokio task, 每 N 秒) ───────────────────────────┐
│                                                              │
│  1. db::get_all_watchlist_codes()  → Vec<(code, market)>    │
│  2. 按 market 分组 (当前仅 CN)                                │
│  3. manager.fetch_realtime(codes, market)                    │
│     ├─ 3a. 命中缓存 (age < interval) → 直接返回              │
│     └─ 3b. 未命中 → HTTP 批量请求 → 写缓存                   │
│  4. 通过 app_handle.emit("quotes-updated", quotes)          │
│     ├─ 主界面 Pinia store → 响应式渲染                       │
│     └─ 行情条 → 滚动数据更新                                 │
│                                                              │
│  指数轮询 (独立，5s 间隔)                                     │
│  manager.fetch_indices() → emit("indices-updated", ...)     │
└──────────────────────────────────────────────────────────────┘
```

### 5.2 按需请求（个股详情）

```
用户点击自选行
  → StockDetail.vue mounted
  → invoke("get_depth", { code, market })
  → invoke("get_intraday", { code, market })
  → 不参与全局轮询，详情页关闭即停止
```

### 5.3 轮询频率

| 场景 | 频率 | 说明 |
|------|------|------|
| 全局自选行情 | 3s (可配 1-10s) | 交易时段持续轮询 |
| 指数行情 | 5s | 指数变动更慢 |
| 个股盘口 | 按需 | 仅在详情页打开时请求 |
| 分时图 | 按需 | 仅在详情页打开时请求 |
| 搜索 | 按需 | 用户输入时防抖 300ms 后请求 |

### 5.4 缓存策略

- 内存缓存（HashMap）：引用计数 + TTL，3s 内命中直接返回
- SQLite 持久化缓存：启动时恢复上次数据，避免首屏空白
- 缓存失效：每次成功获取后覆盖更新

---

## 6. 窗口与交互设计

### 6.1 三个窗口

| 窗口 | 类型 | 特性 |
|------|------|------|
| 行情条 (TickerBar) | undecorated | always-on-bottom, skip-taskbar, 宽=屏幕宽, 高=32px, 无边框, 无任务栏图标 |
| 主界面 (MainWindow) | decorated | 默认 1000×680, 可记忆位置/大小, 关闭=隐藏 |
| 设置 (SettingsDialog) | modal | 模态弹窗, 从主界面设置按钮唤起 |

### 6.2 托盘交互

- **图标**：始终显示
- **双击**：显示/隐藏主界面
- **右键菜单**：
  - 显示主界面
  - 显示/隐藏行情条
  - 数据源（子菜单：当前激活的源标 √）
  - 开机自启（checkbox）
  - 退出

### 6.3 生命周期

```
应用启动
├─ 托盘图标创建
├─ 行情条窗口创建
├─ 读取用户配置 (数据源/主题/自选)
├─ 启动定时轮询
└─ 不自动打开主界面

关闭主界面:
  └─ 隐藏窗口（不销毁），托盘和行情条继续运行

退出:
  ├─ 保存状态 (窗口位置/自选/配置)
  ├─ 销毁所有窗口
  └─ 进程退出
```

---

## 7. 持久化

### 7.1 SQLite Schema

```sql
CREATE TABLE watchlist (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT NOT NULL,
    market      TEXT NOT NULL DEFAULT 'CN',
    name        TEXT NOT NULL,
    sort_order  INTEGER DEFAULT 0,
    added_at    TEXT NOT NULL,
    UNIQUE(code, market)
);

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL
);

CREATE TABLE quote_cache (
    code        TEXT NOT NULL,
    market      TEXT NOT NULL DEFAULT 'CN',
    data        TEXT NOT NULL,
    cached_at   TEXT NOT NULL,
    PRIMARY KEY (code, market)
);
```

### 7.2 Settings Keys

| Key | Default | Description |
|-----|---------|-------------|
| `active_datasource` | `"eastmoney"` | 当前数据源标识 |
| `refresh_interval` | `"3"` | 刷新间隔（秒） |
| `theme` | `"dark"` | dark / light |
| `ticker_visible` | `"true"` | 行情条显隐 |
| `ticker_stocks` | `"watchlist"` | 行情条内容：watchlist / top5 / indices |
| `auto_launch` | `"false"` | 开机自启 |
| `window_width` | `"1000"` | 主界面宽度 |
| `window_height` | `"680"` | 主界面高度 |
| `window_x` | 无 | 主界面 X 坐标 |
| `window_y` | 无 | 主界面 Y 坐标 |
| `datasource_config` | `"{}"` | 数据源专属配置（JSON, e.g. API Key） |

---

## 8. 数据源

### 8.1 实现计划

| 适配器 | 状态 | 覆盖市场 | 备注 |
|--------|------|----------|------|
| EastmoneyAdapter | Phase 1 | A 股 | 默认源，接口稳定，A 股数据最全 |
| SinaAdapter | Phase 1 | A 股 | 备用源，接口格式简洁 |
| TencentAdapter | Phase 2 | A 股 + 港股 | 港股数据源，A 股备用 |
| WindAdapter | Phase 3 | 全部 | 专业数据源，需 API Key |

### 8.2 数据源切换

- 用户从设置界面选择数据源 → 前端 `invoke("set_setting", { key: "active_datasource", value: "sina" })`
- Rust 端 `DataSourceManager.set_active("sina")` → 后续轮询使用新适配器
- 切换失败回退到上一个可用源，托盘通知用户

### 8.3 降级策略

```
请求 Eastmoney
  ├─ 成功 → 返回数据
  └─ 失败 → 重试 2 次
       ├─ 成功 → 返回数据
       └─ 失败 → 自动切换到备用源 (Sina)
            ├─ 成功 → 返回数据 + 托盘通知"已切换到备用数据源"
            └─ 失败 → 返回缓存数据 + 显示"网络异常"标签
```

---

## 9. 错误处理

| 场景 | 策略 |
|------|------|
| 数据源不可用 | 自动切换到备用源，托盘通知 |
| 网络断开 | 显示最后缓存 + "网络异常"标签，恢复后自动重连 |
| 股票代码无效 | 返回错误，前端 toast 提示，不崩溃 |
| 批量请求部分失败 | 返回成功子集 + 失败列表，前端标灰失败项 |
| SQLite 异常 | 降级为内存模式，托盘通知"数据无法持久化" |
| 窗口创建失败 | 降级到仅托盘模式，右键菜单提供有限操作 |

---

## 10. 主题系统

### 10.1 实现方案

- Naive UI 原生 `darkTheme` / `lightTheme` 切换
- CSS 变量定义颜色语义（涨/跌/背景/文字/边框）
- `src/assets/styles/variables.css` 定义变量，`dark.css` / `light.css` 分别赋值
- Pinia `theme` store 管理主题状态，切换时写 settings 持久化
- 行情条和主界面独立响应主题变化（通过 Tauri event 同步）

### 10.2 颜色语义

| 用途 | 日间 | 夜间 |
|------|------|------|
| 上涨 | `#cf1322` | `#ef5350` |
| 下跌 | `#389e0d` | `#66bb6a` |
| 背景 | `#ffffff` | `#1e1e2e` |
| 卡片背景 | `#fafafa` | `#252536` |
| 文字主色 | `#1a1a1a` | `#e0e0e0` |
| 文字辅色 | `#666666` | `#888888` |

> 注：A 股上涨红/下跌绿，港股美股上涨绿/下跌红。Phase 1 默认 A 股配色，预留市场感知的颜色映射。

---

## 11. 模糊搜索

### 11.1 搜索流程

```
用户输入 "平安"
  → 防抖 300ms
  → invoke("search_stocks", { keyword: "平安", market: "CN" })
  → DataSource.search("平安", CN)
  → API 返回匹配结果
  → 前端下拉列表展示 (代码 + 名称)
  → 用户点击选中 → 添加到自选
```

- 支持代码搜索（"000001"）和名称搜索（"平安银行"）
- 支持拼音搜索（取决于数据源 API 是否支持）
- 搜索结果去重，已加入自选的标灰显示

---

## 12. 数据源接口参考

### 12.1 东方财富 (默认)

- 实时行情: `https://push2.eastmoney.com/api/qt/ulist.npz?fltt=2&fields=f2,f3,f4,f12,f14...&secids=0.000001,1.600519`
- 指数: `https://push2.eastmoney.com/api/qt/ulist.npz?fltt=2&fields=f2,f3,f4,f12,f14&secids=1.000001,0.399001,0.399006`
- 分时图: `https://push2.eastmoney.com/api/qt/stock/trends2/get?secid=0.000001`
- 搜索: `https://searchapi.eastmoney.com/api/suggest/get?input=平安`

### 12.2 新浪 (备用)

- 实时行情: `http://hq.sinajs.cn/list=sh000001,sz000001`
- 盘口和指数格式类似，文档见新浪财经开放接口

### 12.3 接口注意事项

- 使用 `reqwest` 设置标准浏览器 User-Agent，避免被拒
- 请求间隔控制：单个数据源 QPS 限制，通过 scheduler 统一管理
- 交易时段判断：非交易时段降低轮询频率或暂停

---

## 13. 打包与分发

- `tauri build` 生成平台安装包
- Windows: `.msi` 安装包
- macOS: `.dmg`
- Linux: `.AppImage` / `.deb`
- 签名策略：Phase 1 不签名（个人使用），Phase 2 考虑代码签名
- 自动更新：Phase 2 引入 `tauri-plugin-updater`

---

## 14. 非功能需求

| 类别 | 指标 |
|------|------|
| 内存占用 | 空闲 < 50MB，活跃 < 100MB |
| CPU 占用 | 空闲 < 1%，轮询峰值 < 5% |
| 包体大小 | < 15MB (压缩后 < 10MB) |
| 启动时间 | < 3s (到托盘图标就绪) |
| 轮询延迟 | 数据源响应 < 300ms + 缓存命中 < 1ms |
| 支持自选数 | 最多 200 只（批量 API 限制） |

---

## 15. 开发阶段规划

### Phase 1 — MVP（核心看盘）

- [ ] 项目脚手架搭建（Tauri + Vue 3 + Vite）
- [ ] Rust domain 模型 + SQLite 持久化
- [ ] Eastmoney 数据源适配器
- [ ] 系统托盘 + 右键菜单
- [ ] 行情条独立窗口（undecorated + always-on-bottom）
- [ ] 主界面：指数横条 + 自选列表（表格）
- [ ] 添加/删除/排序自选
- [ ] 全局轮询 + 缓存
- [ ] 暗色主题

### Phase 2 — 完善体验

- [ ] 模糊搜索添加自选
- [ ] 个股详情：分时图 + 五档盘口 + 基本面概要
- [ ] 日间/夜间主题切换
- [ ] 新浪备用数据源 + 切换功能
- [ ] 涨跌排序和筛选
- [ ] 窗口位置/大小记忆
- [ ] 开盘/收盘/午休自动调整轮询频率

### Phase 3 — 增强功能

- [ ] K 线图（日/周/月）+ 技术指标（MA/BOLL/MACD）
- [ ] 价格预警通知
- [ ] 自选导入/导出（JSON/CSV）
- [ ] 腾讯数据源
- [ ] 开机自启
- [ ] 打包配置 + 安装包生成
- [ ] 非交易时段智能休眠

### Phase 4 — 扩展（远期）

- [ ] 港股/美股市场支持
- [ ] 专业数据源对接（Wind/Tushare）
- [ ] 自动更新
- [ ] macOS/Linux 适配优化

---

## 16. 风险与应对

| 风险 | 影响 | 应对 |
|------|------|------|
| 免费数据源接口变更 | 行情不可用 | 多源互备，抽象 trait 快速适配 |
| 批量请求频率限制 | 数据延迟 | scheduler 统一节流，合理设置间隔 |
| KLineChart 未来不维护 | K 线功能受阻 | Phase 1-2 暂不依赖其高级功能，保留切换 Lightweight Charts 空间 |
| Windows 任务栏布局变化 | 行情条位置错乱 | 提供手动调整位置选项，监听屏幕变化事件 |

---

## 17. 附录

### A. TypeScript 类型定义示例

```typescript
// src/types/index.ts
export interface Quote {
  code: string;
  market: 'CN' | 'HK' | 'US';
  name: string;
  price: number;
  change: number;
  changePct: number;
  open: number;
  high: number;
  low: number;
  volume: number;
  turnover: number;
  timestamp: number;
}

export interface IndexQuote {
  code: string;
  name: string;
  price: number;
  change: number;
  changePct: number;
  volume: number;
  turnover: number;
}

export interface Depth {
  code: string;
  bids: Level[];
  asks: Level[];
}
```

### B. Tauri 窗口配置要点

```json
// 行情条窗口
{
  "label": "ticker",
  "url": "ticker.html",
  "decorations": false,
  "alwaysOnBottom": true,
  "skipTaskbar": true,
  "width": 1920,
  "height": 32,
  "resizable": false,
  "focus": false
}
```

### C. 项目仓库信息

- 仓库: `github/quant-desktop`
- 当前分支: `master`
- 初始 commit: `e426170`
