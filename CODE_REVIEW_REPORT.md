# QuantDesktop 代码审查报告

**项目**: QuantDesktop v1.1.0  
**审查日期**: 2026-06-20  
**审查范围**: 全部前端 (Vue/TypeScript) 和后端 (Rust/Tauri) 代码  
**审查人**: Claude Code (deepseek-v4-pro)

---

## 目录

1. [总体评估](#1-总体评估)
2. [架构与设计](#2-架构与设计)
3. [Rust 后端详细审查](#3-rust-后端详细审查)
4. [Vue/TypeScript 前端详细审查](#4-vuetypescript-前端详细审查)
5. [配置与构建审查](#5-配置与构建审查)
6. [安全问题](#6-安全问题)
7. [性能分析](#7-性能分析)
8. [发现汇总](#8-发现汇总)

---

## 1. 总体评估

| 维度 | 评分 (1-5) | 说明 |
|------|-----------|------|
| 架构设计 | ⭐⭐⭐⭐ | 清晰的分层架构，DataSource 插件化设计优秀 |
| 代码质量 | ⭐⭐⭐ | 整体良好，存在改进空间（错误处理、重复代码） |
| 安全性 | ⭐⭐⭐⭐ | CSP 严格，权限最小化，IPC 边界清晰 |
| 性能 | ⭐⭐⭐⭐ | 缓存策略合理，自适应轮询机制设计精良 |
| 错误处理 | ⭐⭐⭐ | String 错误类型不够结构化，部分静默吞错 |
| 可维护性 | ⭐⭐⭐⭐ | 模块化良好，CLAUDE.md 文档详尽 |
| 类型安全 | ⭐⭐⭐⭐ | TypeScript strict 模式，Rust 类型系统充分利用 |

**总体评价**: 项目整体质量较高，架构设计清晰合理，数据流和模块边界定义明确。主要改进方向集中在：错误处理的结构化、代码复用减少重复、以及测试覆盖。

---

## 2. 架构与设计

### 2.1 整体架构 ✅ 优秀

```
┌─────────────────────────────────────────────────┐
│                    前端                          │
│  main window (Vue 3 + Pinia)                    │
│  ticker window (独立 Vue 应用)                    │
│       ↕ IPC invoke / Tauri events               │
├─────────────────────────────────────────────────┤
│                Rust 后端 (Tauri)                  │
│  commands/ ──► datasource/ ──► 外部 API          │
│       │              │                           │
│       ▼              ▼                           │
│     db/           cache/                         │
│   (SQLite)    (HashMap + 持久化)                  │
└─────────────────────────────────────────────────┘
```

**亮点**:
- DataSource trait 抽象允许运行时切换数据源（新浪 ↔ 腾讯），设计优雅
- 双窗口架构（主窗口 + 迷你 ticker）通过 Tauri 事件同步状态
- 自适应轮询状态机（Probing → Normal → Idle）根据盘面活动动态调整轮询频率
- QuoteCache 内存 + SQLite 双写策略实现启动时即时数据展示

### 2.2 数据源架构 ⭐⭐⭐⭐⭐ 优秀

`DataSource` trait 的插件化设计是项目的核心亮点：

```rust
pub trait DataSource: Send + Sync {
    async fn fetch_realtime(&self, codes: &[String], market: &str) -> Result<Vec<Quote>, String>;
    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String>;
    // ...
}
```

`DataSourceManager` 通过 `RwLock` + `Notify` 实现运行时可切换：

```
switch_datasource → set_active(name) → Notify::notify_one()
                                     → db.set_setting("active_datasource", name)
```

### 2.3 数据流清晰

```
DataSource API (新浪/腾讯)
  → Scheduler (tokio 后台轮询, 动态间隔 via market_clock)
    → QuoteCache (内存 HashMap + SQLite 双写)
      → Tauri events: quotes-updated / indices-updated / market-session-changed
        → Pinia Stores (Vue 响应式)
          → 组件渲染
```

### 2.4 设计建议

| # | 建议 | 优先级 |
|---|------|--------|
| D1 | 引入 `thiserror` 定义结构化错误类型，替代全项目使用的 `String` 错误 | 中 |
| D2 | 考虑将 `reqwest::Client` 单例化 — 当前每个 adapter 创建独立 Client，共 3 个实例 | 低 |
| D3 | 新浪 adapter 的深度数据静默回退到腾讯 API，应在前端提示用户数据来源 | 低 |
| D4 | 前后端共享类型通过手动维护 `domain/mod.rs` 和 `types/index.ts` 双份定义，可考虑代码生成（如 ts-rs） | 低 |

---

## 3. Rust 后端详细审查

### 3.1 文件清单 (15 个源文件)

| 文件 | 行数 | 职责 |
|------|------|------|
| `main.rs` | ~7 | 入口 |
| `lib.rs` | ~428 | 应用初始化、窗口管理、Tray 设置 |
| `domain/mod.rs` | ~76 | 数据模型定义 |
| `db/mod.rs` | ~217 | SQLite 数据库 CRUD |
| `datasource/mod.rs` | ~156 | DataSource trait + Manager |
| `datasource/sina.rs` | ~467 | 新浪财经适配器 |
| `datasource/tencent.rs` | ~449 | 腾讯证券适配器 |
| `datasource/search.rs` | ~401 | 多级搜索策略 |
| `datasource/market_clock.rs` | ~70 | 交易时段检测 |
| `datasource/headers.rs` | ~44 | HTTP 请求头反屏蔽 |
| `cache/mod.rs` | ~415 | 缓存 + 后台轮询调度器 |
| `commands/mod.rs` | ~5 | 命令模块声明 |
| `commands/quote.rs` | ~50 | 行情 IPC 命令 |
| `commands/watchlist.rs` | ~136 | 自选股 CRUD IPC 命令 |
| `commands/settings.rs` | ~41 | 设置 IPC 命令 |
| `commands/window.rs` | ~11 | 窗口 IPC 命令 |

### 3.2 lib.rs - 应用初始化

**审查结果**: ✅ 良好

**观察**:
- 初始化流程清晰有序：logger → db → datasources → cache → scheduler → tray → windows
- 使用 `expect()` 处理不可恢复的启动错误，这是正确的做法
- `CloseRequested` 事件拦截实现关闭到托盘，逻辑正确
- 窗口位置恢复包含显示器边界验证，处理了多显示器配置变更的情况

### 3.3 domain/mod.rs - 数据模型

**审查结果**: ✅ 良好

**观察**:
- `market` 字段使用 `String` 而非枚举，简化了序列化但降低了类型安全性
- `Debug + Clone + Serialize + Deserialize` 派生完整
- `turnover_rate` 使用 `Option<f64>` 处理指数无换手率的情况

### 3.4 db/mod.rs - 数据库层

**审查结果**: ✅ 良好

**发现**:

| # | 类型 | 描述 |
|---|------|------|
| B1 | 🟡 改进 | `Database` 使用 `std::sync::Mutex` 包装连接。当前通过 `spawn_blocking` 安全使用，但如果未来开发者在异步上下文中直接调用 DB 方法会阻塞 tokio worker 线程。建议添加文档注释说明必须在 `spawn_blocking` 中调用。 |
| B2 | 🟡 改进 | `init_defaults()` 通过检查 key 是否存在判断首次运行。在并发场景下存在 TOCTOU 竞态（实际影响低，因为启动是单线程的） |

### 3.5 datasource/ - 数据源

**审查结果**: ✅ 良好，有改进空间

**SinaAdapter**:
| # | 类型 | 描述 |
|---|------|------|
| B3 | 🟡 改进 | 新浪深度 API 已失效，`fetch_depth()` 静默回退到腾讯 API。这意味着即使选择新浪，深度数据仍来自腾讯。应添加日志警告或在 UI 中提示。 |
| B4 | 🔴 缺陷 | K线周期限制：`fetch_kline()` 对周K/月K返回错误，这由前端 `ChartSwitcher` 的交互逻辑通过数据源切换处理。如果用户在详情面板手动调用 `get_kline` 且数据源为新浪，将收到错误消息。 |
| B5 | 🟡 改进 | 硬编码交易时段（9:30-11:30, 13:00-15:00）。无节假日日历。自适应 `PollingState` 通过价格停滞检测缓解此问题，这是一个实用的折中方案，但应记录在案。 |

**TencentAdapter**:
| # | 类型 | 描述 |
|---|------|------|
| B6 | 🟡 改进 | 成交量统一乘以 100（手→股），成交额乘以 10000（万元→元）。此归一化逻辑在两个适配器中重复。建议提取到 `DataSource` trait 的默认方法或 `domain` 的辅助函数中。 |

**search.rs**:
| # | 类型 | 描述 |
|---|------|------|
| B7 | 🟢 表扬 | 三级搜索策略设计良好：新浪 suggest → 腾讯 smartbox → 适配器全遍历回退。`OnceLock<Client>` 静态单例正确使用。 |

### 3.6 cache/mod.rs - 缓存与调度器

**审查结果**: ⭐⭐⭐⭐⭐ 优秀

**亮点**:
- **自适应轮询状态机** (`PollingState`): `Probing{remaining: 3}` → `Normal{unchanged_streak}` → `Idle`，这是检测市场开盘/收盘的巧妙方法
- **FetchGuard**: 使用 `AtomicBool` 和 Acquire/Release 语义防止并发获取，正确处理两个异步循环（定时器 + 唤醒通知）的竞态条件
- **价格变更检测**: 通过比较获取前后的快照价格确定 `FetchOutcome`，正确使用 `f64::EPSILON`
- **缓存持久化**: `spawn_blocking` 包装 SQLite I/O，避免阻塞异步运行时

| # | 类型 | 描述 |
|---|------|------|
| B8 | 🟡 改进 | `f64::EPSILON` (~2.2e-16) 作为价格变化阈值过于严格。股价通常精确到 2-3 位小数。两个因浮点噪声差异 1e-16 的价格会被检测为变化。实际上这不会造成问题，但语义不够清晰。建议使用 `1e-10` 或基于价格精度动态计算。 |
| B9 | 🟡 改进 | 获取失败时无指数退避重试。网络瞬断会导致不必要的失败轮询周期。 |
| B10 | 🟡 改进 | 毒化的 `Mutex` 通过 `unwrap_or_else(|e| e.into_inner())` 恢复是正确做法，但静默忽略导致毒化的 panic。建议记录 warn 级别日志。 |

### 3.7 commands/ - IPC 命令处理器

**审查结果**: ✅ 良好

**watchlist.rs**:
| # | 类型 | 描述 |
|---|------|------|
| B11 | 🟡 改进 | `move_watch_top` 实现：先从列表删除条目，再以 `sort_order=0` 重新插入。两次数据库操作非原子性。如果第二步失败，条目丢失。应使用事务或单条 UPDATE SQL。 |
| B12 | 🟢 表扬 | `search_stocks` 三级搜索回退策略：新浪 suggest → 腾讯 smartbox → 适配器逐个尝试。失败时优雅降级而非直接报错。|

---

## 4. Vue/TypeScript 前端详细审查

### 4.1 文件清单 (30 个源文件)

| 类别 | 文件 | 职责 |
|------|------|------|
| 入口 | `main.ts`, `ticker.ts` | 两个独立的 Vue 应用入口 |
| 组件 | `App.vue` | 根组件（主题配置、初始化、事件监听） |
| 布局 | `AppLayout.vue`, `TopBar.vue`, `StatusBar.vue` | 主窗口布局 |
| 指数 | `IndexBar.vue`, `IndexCard.vue` | 指数展示 |
| 自选 | `WatchlistTable.vue`, `AddStockDialog.vue` | 自选股表格和搜索 |
| 详情 | `StockDetail.vue`, `IndexDetail.vue`, `StockSummary.vue`, `DepthPanel.vue`, `ChartSwitcher.vue`, `MinuteChart.vue`, `KLineChart.vue` | 详情面板 |
| Ticker | `TickerBar.vue` | 浮动迷你行情条 |
| 状态 | `stores/quote.ts`, `stores/watchlist.ts`, `stores/settings.ts` | Pinia 状态管理 |
| 工具 | `composables/useChart.ts`, `utils/format.ts`, `utils/keys.ts` | 组合式函数和工具 |
| 样式 | `variables.css`, `dark.css`, `chart.css` | CSS 设计系统 |

### 4.2 组件审查

**App.vue**:
| # | 类型 | 描述 |
|---|------|------|
| F1 | 🟡 改进 | 初始化使用 try/catch 包裹，但错误仅设置 `initError.value = String(e)`。子组件的未捕获错误无全局边界处理。建议添加 `onErrorCaptured` 钩子。 |
| F2 | 🟡 改进 | `initReady` 为 false 时布局直接渲染空数据，无骨架屏或加载指示器。 |

**TickerBar.vue**:
| # | 类型 | 描述 |
|---|------|------|
| F3 | 🟡 改进 | 每 3 秒轮询 `watchlist.fetchWatchlist()` 来获取变化，即使自选股未修改。这是一个不必要的网络/DB 开销。后端可通过 Tauri 事件推送自选股变更。 |
| F4 | 🔴 缺陷 | 拖拽移动的 catch 块为空（第 195 行附近），静默吞掉所有错误。如果显示器检测失败，窗口可能被拖到屏幕外。 |
| F5 | 🟡 改进 | `initFailed` 切换存在短暂竞态窗口：设置 `initFailed = false` 后但异步初始化完成前，模板会切换显示空数据状态。 |

**WatchlistTable.vue**:
| # | 类型 | 描述 |
|---|------|------|
| F6 | 🟡 改进 | 无键盘导航支持（方向键无法在表格行间移动）。 |
| F7 | 🟡 改进 | 右键菜单选项数组混合类型（普通选项 + `{type:'divider'}` 分隔符），未定义专用类型。 |

**AddStockDialog.vue**:
| # | 类型 | 描述 |
|---|------|------|
| F8 | 🟢 表扬 | 300ms 防抖搜索设计合理，避免每次按键触发 IPC 调用。 |
| F9 | 🟡 改进 | 搜索结果无"无结果"空状态提示，列表为空时用户无法区分"搜索中"和"无结果"。 |

**MinuteChart.vue / KLineChart.vue**:
| # | 类型 | 描述 |
|---|------|------|
| F10 | 🟡 改进 | 每次 code/market/period 变化时销毁并重建图表。可改为更新现有图表的数据和样式，减少 DOM 操作。 |
| F11 | 🟡 改进 | K-line 图表的 `dataLoader` 仅处理 `'init'` 情况，`'more'` 情况返回空数组。这意味着用户无法加载更多历史数据。 |
| F12 | 🟡 改进 | 图表高度硬编码 320px，无响应式适配。在较小窗口上可能导致布局问题。 |

**DepthPanel.vue**:
| # | 类型 | 描述 |
|---|------|------|
| F13 | 🟢 表扬 | 买卖盘深度条宽度按该档成交量相对于最大成交量的比例动态计算，视觉直观。 |

### 4.3 状态管理审查

**Quote Store** (`stores/quote.ts`):
| # | 类型 | 描述 |
|---|------|------|
| F14 | 🟡 改进 | `quotes` ref 包裹 `Map`，每次 `quotes-updated` 事件替换整个 Map。使用 `shallowRef` 更合适，因为 Map 是整体替换而非增量更新。 |

**Watchlist Store** (`stores/watchlist.ts`):
| # | 类型 | 描述 |
|---|------|------|
| F15 | 🟡 改进 | 每次 mutation（add/remove）后完整重新获取自选股列表。乐观更新模式可避免不必要的网络请求和 UI 闪烁。 |

**Settings Store** (`stores/settings.ts`):
| # | 类型 | 描述 |
|---|------|------|
| F16 | 🟢 表扬 | 主题切换通过 `applyTheme` 和 `toggleTheme` 分离关注点，避免跨窗口事件循环。设计良好。 |
| F17 | 🟢 表扬 | `switchDatasource` 在错误时回滚本地状态，处理了乐观更新的边界情况。 |

### 4.4 useChart Composable 审查

| # | 类型 | 描述 |
|---|------|------|
| F18 | 🟡 改进 | klinecharts 样式配置大量使用 `as any` 类型断言（60+ 行）。klinecharts v10 beta 的类型定义不完整，但这降低了类型安全性。 |
| F19 | 🟢 表扬 | 支持 `AbortController` 中止进行中的请求，避免竞态条件。 |
| F20 | 🟢 表扬 | `syncPrecision()` 根据最后一根 bar 的收盘价自适应小数精度。 |

### 4.5 CSS / 样式审查

| # | 类型 | 描述 |
|---|------|------|
| F21 | 🟢 表扬 | `variables.css` 设计系统完备：4 级表面色、语义化涨跌色（红涨绿跌）、表格数字等宽字体、6 级字号、4px 基础间距。 |
| F22 | 🟡 改进 | `:root` 默认暗色主题。如果 `data-theme` 属性缺失，始终显示暗色——这是合理的默认值，但应在代码中显式处理。 |
| F23 | 🟡 改进 | `dark.css` 滚动条在亮色模式下为固定暗色 thumb，对比度可能不足，但实际效果可接受。 |

### 4.6 类型定义审查

| # | 类型 | 描述 |
|---|------|------|
| F24 | 🟡 改进 | `types/index.ts` 中的类型需与 Rust `domain/mod.rs` 手动保持同步。字段变更时容易遗漏一端。可考虑 `ts-rs` crate 自动生成 TypeScript 类型。 |
| F25 | 🟢 表扬 | TypeScript strict 模式 + `noUnusedLocals` + `noUnusedParameters`——高标准的类型安全配置。 |

---

## 5. 配置与构建审查

### 5.1 TypeScript 配置

**tsconfig.json**: ⭐⭐⭐⭐⭐ 优秀  
- `strict: true` 完整严格模式
- `noUnusedLocals: true`, `noUnusedParameters: true`, `noFallthroughCasesInSwitch: true`
- 未使用的代码会导致构建失败——在类型安全上毫不妥协

### 5.2 Tauri 配置

**tauri.conf.json**: ⭐⭐⭐⭐ 良好

| 配置项 | 评审 |
|--------|------|
| CSP | `default-src 'self'; img-src 'self' asset:; style-src 'self' 'unsafe-inline'; script-src 'self'` — 严格，无 `unsafe-eval`，无外部脚本 |
| 窗口权限 | `capabilities/default.json` 仅授予最小必要权限：core:default, opener, autostart, 窗口定位相关 |
| 文件系统 | 无 fs/shell/http/clipboard 权限——安全态势良好 |
| `style-src 'unsafe-inline'` | Naive UI CSS-in-JS 所需，是实用主义的必要妥协 |

### 5.3 CI/CD

| # | 类型 | 描述 |
|---|------|------|
| C1 | 🟡 改进 | 发布产物无代码签名。Windows 上会触发 SmartScreen 警告，macOS 上会触发 Gatekeeper。 |
| C2 | 🟢 表扬 | 矩阵构建覆盖 Windows/MSVC、macOS/universal、Linux/gnu 三个平台。 |
| C3 | 🟡 改进 | 无自动化测试步骤。CI 仅执行 `tauri build`，无 `cargo test` 或前端测试。 |

### 5.4 依赖评估

| 依赖 | 版本 | 评审 |
|------|------|------|
| `klinecharts` | `^10.0.0-beta3` | 🟡 beta 版本用于生产环境。v10 正式版可能包含 breaking changes。 |
| `reqwest` | `0.12` + `0.13` 共存 | 🟡 Cargo.lock 中同时存在两个 semver 不兼容的 reqwest 版本（0.12.28 和 0.13.4）。增加编译时间。 |
| `@types/node` | `^25.9.3` | 🟡 版本非常新，与 TypeScript 5.6.2 可能有兼容性问题（实际影响小）。 |
| `vue` | `^3.5.13` | ✅ 最新稳定版 |
| `pinia` | `^3.0.4` | ✅ 最新稳定版 |
| `tauri` | `2` (2.11.2) | ✅ 最新稳定版 |

---

## 6. 安全问题

### 6.1 正面安全实践

| # | 类型 | 实践 |
|---|------|------|
| S1 | ✅ CSP | 严格的 Content Security Policy，禁止外部脚本和 eval |
| S2 | ✅ 权限 | Tauri 权限最小化，无 fs/shell/http/clipboard 访问 |
| S3 | ✅ 网络 | `reqwest` 使用 `rustls-tls`，无 OpenSSL 依赖 |
| S4 | ✅ TLS | 所有外部 API 调用使用 HTTPS |
| S5 | ✅ SQL | SQLite 使用参数化查询，无 SQL 注入风险 |
| S6 | ✅ 上下文 | 前端禁用默认右键菜单 (`contextmenu` 事件) |

### 6.2 安全建议

| # | 优先级 | 描述 |
|---|--------|------|
| SEC1 | 🟡 中 | CSP `style-src 'unsafe-inline'` 是 Naive UI 所需，但增加了 CSS 注入攻击面。在 Tauri 桌面环境中实际风险有限（无用户生成内容注入），但应记录此妥协。 |
| SEC2 | 🟢 低 | 日志 (`log` + `simplelog`) 可能记录敏感数据（股票代码、设置）。确认日志文件权限设置正确。 |
| SEC3 | 🟢 低 | 无二进制代码签名——用户下载时需要手动信任。建议 CI/CD 中添加签名步骤。 |
| SEC4 | 🟢 低 | GBK 解码使用 `encoding_rs` 的 replacement 模式，不会 panic 但可能静默替换损坏字符。对于金融数据源，建议记录解码警告。 |

---

## 7. 性能分析

### 7.1 后端性能

| 项目 | 评估 | 说明 |
|------|------|------|
| 轮询效率 | ⭐⭐⭐⭐⭐ | 自适应状态机（2s 交易→5s 前/后→10s 午休→30s 闭市）最小化无用请求 |
| 批处理 | ✅ | `group_by_market` 将自选股按市场分组，一次请求获取多个股票 |
| 并发控制 | ✅ | `FetchGuard` 防止重复获取，避免浪费 |
| 缓存策略 | ✅ | 启动时从 SQLite 恢复缓存，失败时回退到缓存数据 |
| 内存 | ✅ | `HashMap` 作为行情缓存，O(1) 查找 |
| 阻塞 I/O | ✅ | 所有 SQLite 调用通过 `spawn_blocking` 包装 |

### 7.2 前端性能

| 项目 | 评估 | 说明 |
|------|------|------|
| 事件驱动更新 | ✅ | 通过 Tauri 事件推送行情，非轮询 |
| 组件懒加载 | ⚠️ | 详情面板条件渲染，但图表库 klinecharts 初始化较重 |
| 表格渲染 | ⚠️ | NDataTable 全量重新渲染，大数据量时可能卡顿 |
| Ticker 轮询 | 🔴 | 每 3 秒不必要的自选股轮询（对应 F3） |
| 图表 | ⚠️ | 切换股票时销毁重建图表（对应 F10） |

### 7.3 性能建议

| # | 优先级 | 描述 |
|---|--------|------|
| PERF1 | 🟡 中 | Ticker 的自选股轮询应替换为事件驱动更新 |
| PERF2 | 🟢 低 | 图表切换时复用现有 klinecharts 实例而非重建 |
| PERF3 | 🟢 低 | 考虑 `shallowRef` 优化行情 Map 的响应式开销 |

---

## 8. 发现汇总

### 8.1 按严重程度统计

| 严重程度 | 数量 | 标记 |
|----------|------|------|
| 🔴 缺陷 (Bug) | 2 | B4, F4 |
| 🟡 改进 (Improvement) | 25 | B1-B3, B5-B6, B8-B11, F1-F3, F5-F7, F9-F12, F14-F15, F18, F22-F24, C1, C3, SEC1, PERF1 |
| 🟢 表扬 (Commendable) | 14 | B7, B12, F8, F13, F16-F17, F19-F21, F25, C2, S1-S6 |

### 8.2 需立即修复的问题

| # | 文件 | 描述 | 严重程度 |
|---|------|------|----------|
| B11 | `commands/watchlist.rs` | `move_watch_top` 非原子操作可能导致条目丢失 | 🟡 中 |
| F4 | `TickerBar.vue` | 拖拽错误处理为空 catch 块，窗口可能拖出屏幕 | 🔴 中 |

### 8.3 主要改进建议（按影响排序）

1. **引入结构化错误类型** (D1, B1, B9) — 使用 `thiserror` 替代全项目 `String` 错误，使前端可编程地区分错误类型
2. **Ticker 事件驱动更新** (F3, PERF1) — 后端推送自选股变更事件，消除每 3 秒的不必要轮询
3. **添加自动化测试** (C3) — 至少添加 `cargo test` 和 vitest 的基础测试覆盖
4. **统一数据归一化逻辑** (B6) — 将成交量×100、成交额×10000 提取到共享位置
5. **共享 reqwest::Client** (D2) — 减少 HTTP 连接池浪费
6. **`move_watch_top` 事务化** (B11) — 确保排序操作的原子性
7. **图表实例复用** (F10, PERF2) — 避免股票切换时的销毁/重建开销
8. **全局错误边界** (F1) — 添加 `onErrorCaptured` 钩子防止静默崩溃

### 8.4 架构亮点

1. ⭐ **DataSource 插件化架构** — 运行时可切换数据源，Notify 唤醒机制
2. ⭐ **自适应轮询状态机** — Probing → Normal → Idle 动态调整频率
3. ⭐ **FetchGuard** — AtomicBool 无锁并发控制
4. ⭐ **三级搜索策略** — 新浪 → 腾讯 → 全适配器回退
5. ⭐ **设计系统** — 完备的 CSS 自定义属性，语义化 token
6. ⭐ **CSP 安全态势** — 严格内容安全策略 + 最小权限
7. ⭐ **双窗口事件同步** — 主题和数据源通过 Tauri 事件跨窗口同步
8. ⭐ **窗口位置持久化** — 位置/大小保存 + 显示器边界验证

### 8.5 代码统计

| 类别 | 文件数 | 估计行数 |
|------|--------|----------|
| Rust 后端 | 15 | ~2,900 |
| Vue/TypeScript 前端 | 22 | ~2,800 |
| CSS | 3 | ~370 |
| 配置/构建 | 8 | ~300 |
| **总计** | **48** | **~6,370** |

---

## 附录 A: 审查清单

- [x] 代码架构和模块划分
- [x] 错误处理模式
- [x] 并发和异步安全性
- [x] 数据库操作正确性
- [x] 前端组件设计
- [x] 状态管理
- [x] 类型安全
- [x] CSS 设计系统
- [x] 安全配置（CSP, 权限）
- [x] 构建配置
- [x] 依赖健康度
- [x] 性能特征
- [x] CI/CD 配置

## 附录 B: 基于 Phase 的观察

根据 CLAUDE.md 中的开发阶段：

- **Phase 1-3** (已完成): 基础架构扎实，代码审查发现的问题主要集中在代码质量的打磨层面
- **Phase 4** (规划中): K线图表功能实际上已经部分实现（`KLineChart.vue`, `get_kline` 命令已存在），但使用 beta 版 klinecharts。建议在 Phase 4 前升级到稳定版
- **Phase 5** (未来): HK/US 市场支持在类型系统（`Market` 枚举为 String）中已预留扩展空间

---

*报告生成于 2026-06-20。审查基于代码库静态分析，未包含运行时测试或负载测试。*
