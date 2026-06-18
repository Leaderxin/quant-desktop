# QuantDesktop 全栈代码评审报告（终版）

> **日期**: 2025-06-18
> **分支**: master
> **提交**: v0.3.1 (cf434a7 + 六轮修复)
> **评审范围**: 全项目 — Rust 后端 (14 文件) + TypeScript/Vue 前端 (19 文件) + 配置/构建 (16 文件)
> **修复历史**: 4 Critical ✅ | 7 High ✅ | 15 Medium ✅ | 5 v0.3.1 ✅ | 7 v0.4.0 ✅ | 6 Phase 3+ ✅ | 13 终版 ✅

---

## 目录

- [六轮修复总览](#六轮修复总览)
- [当前状态](#当前状态)
- [🟢 低优先级（13 项）](#-低优先级13-项)
- [统计总览](#统计总览)

---

## 六轮修复总览

| 轮次 | 修复数 | 关键成果 |
|------|--------|---------|
| **Critical** | 4 | CSP 安全策略、active_datasource 持久化、encoding→encoding_rs（8 处）、HTTP 超时（2 处） |
| **High** | 7 | log+simplelog 双输出、quote→computed()、spawn_blocking（2 处）、CSS 变量缺失、timer 泄漏、quoteStore error、序列化 warn 日志 |
| **Medium** | 15 | env_logger 删除、日志补全（12 处）、cache_quotes 事务、DB 错误区分（4 分支）、Store error ref、try/catch（5 处）、onUnmounted、emit()/aria-label（5 处）、--color-error token、emoji→SVG（2 处）、useTauriEvent 竞态 |
| **v0.3.1 收尾** | 5 | expect() 替代 unwrap_or_default、sina.rs 切片长度守卫、volume/turnover=0 修正、tsconfig.node.json strict、release.yml Windows 路径修正 |
| **v0.4.0 短期** | 7 | --color-warning/--radius-full/z-index scale、TickerBar 点击重试、AddStockDialog 搜索错误区分、watchlistStore.error 消费、AbortController 防竞态、死代码清理（useTauriEvent/useTheme/reorder） |
| **Phase 3+ 长期** | 6 | INDEX_CODES/USER_AGENT/TICKER_SIZE 公共常量、AtomicBool fetch 互斥锁、all_sources() 通用回退、unwrap→expect（4 处）、CLAUDE.md 更新、HTML/CSS 细节（6 处） |
| **终版收尾** | 13 | TickerBar 重试防泄漏、硬编码色→CSS class、Store try/catch+回滚、watch code+market、MinuteChart abort 清理、Y 偏移统一、Market 枚举删除、get_quote/_cached_at 清理、.catch→console.error（2 处）、Node 22/tokio 精简/ES2022/authors/TypeScript semver |

**六轮累计修复: 57 项**

---

## 当前状态

### 🔴 Critical: 0 | 🟠 High: 0 | 🟡 Medium: 0

**所有 Critical、High、Medium 问题已清零。**

### 问题趋势

```
轮次              Critical  High  Medium  Low   Total
──────────────────────────────────────────────────────
初版 (cf434a7)        4       8     13     22  =  47
第 2 轮更新            4       7     15     13  =  39  ↓17%
第 3 轮再检            0       6     11     24  =  41
第 4 轮 v0.3.1        0       1      9     23  =  33  ↓20%
第 5 轮 终版           0       0      7     18  =  25  ↓24%
第 6 轮 终版           0       0      0     13  =  13  ↓48%
──────────────────────────────────────────────────────
累计改善:                                47→13  ↓72%
```

---

## 🟢 低优先级（13 项）

均为渐进优化项，不影响功能正确性或安全性。按类别分组：

### 代码组织

| # | 文件 | 问题 | 说明 |
|---|------|------|------|
| L1 | [lib.rs](../src-tauri/src/lib.rs#L23-L283) | 260 行 setup 闭包 | 可分解为独立函数 |
| L2 | [lib.rs](../src-tauri/src/lib.rs) | 18 处 `let _ =` 窗口操作 | 失败极少，日志化收益低 |
| L3 | [commands/watchlist.rs](../src-tauri/src/commands/watchlist.rs#L85-L118) | search_stocks 中的数据源路由在 command 层 | 当前实现简单清晰 |

### Magic Numbers

| # | 文件 | 值 |
|---|------|-----|
| L4 | [lib.rs](../src-tauri/src/lib.rs#L232-L262) | 窗口默认尺寸 `1100/680`、最小 `400/300`、位置验证边距 |
| L5 | [market_clock.rs](../src-tauri/src/datasource/market_clock.rs#L52-L56) | 轮询间隔 `2/5/10/30` 秒 |
| L6 | [market_clock.rs](../src-tauri/src/datasource/market_clock.rs#L31-L34) | 交易时段 `9:30/11:30/13:00/15:00` |

### 第三方限制

| # | 文件 | 问题 | 说明 |
|---|------|------|------|
| L7 | [MinuteChart.vue](../src/components/detail/MinuteChart.vue#L44-L82) | ~15 处 `as any` | klinecharts 库类型定义不完整 |
| L8 | [WatchlistTable.vue](../src/components/watchlist/WatchlistTable.vue#L260-L269) | 右键菜单无键盘替代 | Naive UI NDropdown 限制 |

### 性能微优化

| # | 文件 | 问题 |
|---|------|------|
| L9 | [TopBar.vue](../src/components/layout/TopBar.vue#L142) | `filter: brightness(1.4)` 可改为 background 过渡 |
| L10 | [WatchlistTable.vue](../src/components/watchlist/WatchlistTable.vue#L243-L247) | `row-props` 内联 style 字符串每次渲染重建 |

### 架构文档

| # | 文件 | 问题 |
|---|------|------|
| L11 | [sina.rs](../src-tauri/src/datasource/sina.rs#L305-L375) | `fetch_depth()` 通过 Tencent API 实现（已注释说明） |
| L12 | [cache/mod.rs](../src-tauri/src/cache/mod.rs#L133) | `Notify::notify_one` 在快速连续切换时可能丢失通知（非 correctness bug） |

### 微配置

| # | 文件 | 问题 |
|---|------|------|
| L13 | [TopBar.vue](../src/components/layout/TopBar.vue#L31-L41) | 品牌 SVG 使用硬编码色（装饰性，可接受） |

---

## 统计总览

```
┌──────────┬──────────┬──────────┬──────────┬──────┐
│ 严重级别  │ Rust 后端 │ Vue 前端 │ 配置/构建 │ 合计  │
├──────────┼──────────┼──────────┼──────────┼──────┤
│ 🔴 Critical │    0     │    0     │    0     │  0   │
│ 🟠 High     │    0     │    0     │    0     │  0   │
│ 🟡 Medium   │    0     │    0     │    0     │  0   │
│ 🟢 Low      │    7     │    4     │    2     │ 13   │
├──────────┼──────────┼──────────┼──────────┼──────┤
│ 合计        │    7     │    4     │    2     │ 13   │
└──────────┴──────────┴──────────┴──────────┴──────┘
```

---

## ✅ 已确认修复清单（57 项）

### Critical（4 项）
CSP `null`→`default-src 'self'`、active_datasource 持久化 bug、`encoding`→`encoding_rs` 迁移（8 处）、reqwest Client 超时 + expect()

### High（7 项）
log+simplelog 双输出（stderr+文件）、StockDetail quote→computed()、get_watch_codes+persist_quotes spawn_blocking、窗口 DB 写入+warn 日志（8 处）、Tauri emit 失败 warn 日志（6 处）、toggleTheme await、事件监听 catch→console.error、CSS token `--color-bg-elevated`/`--color-bg-card`（双主题）、TopBar 硬编码色→`var(--color-accent-dim)`、volume/turnover=0 修正、sina.rs 切片守卫

### Medium（15 项）
env_logger 删除、create_dir_all 日志、缓存行反序列化日志、搜索失败日志、cache_quotes 事务、DB 错误区分（4 分支）、watchlistStore error ref、WatchlistTable try/catch（5 处）、App.vue onUnmounted、emit() 替代 $emit、switchDatasource 同步 settings.value、useTauriEvent 竞态守卫、aria-label（5 处）、--color-error token（双主题）、emoji→SVG（2 处）、DepthPanel error 颜色语义修正

### v0.3.1 收尾（5 项）
reqwest Client `.unwrap_or_default()`→`.expect()`（2 处）、sina.rs `code_raw[2..]` 长度守卫、volume/turnover=0 显示修正、tsconfig.node.json strict+types、release.yml Windows 产物路径

### v0.4.0 短期（7 项）
--color-warning/--radius-full/z-index scale（3 token）、TickerBar 点击重试、AddStockDialog 搜索错误区分、watchlistStore.error 在 WatchlistTable 消费、MinuteChart AbortController、死代码清理（3 项）

### Phase 3+ 长期（6 项）
INDEX_CODES/USER_AGENT/TICKER_SIZE 公共常量、AtomicBool fetch 互斥锁+FetchGuard RAII、all_sources() 通用回退迭代器、market_clock unwrap→expect（4 处）、CLAUDE.md 更新（4 处）、index.html/IndexBar/StockDetail/WatchlistTable CSS 细节（6 处）

### 终版收尾（13 项）
TickerBar 重试前清理旧 timer/listener、WatchlistTable render 硬编码色→CSS class `.pct-col.up/.down`、settings.ts `setSetting`/`switchDatasource` try/catch+回滚、watchlist.ts `addStock`/`removeStock` try/catch+error ref、MinuteChart+DepthPanel `watch([code, market])`、MinuteChart `onUnmounted` abort、ticker Y 偏移 46→60 统一、Market enum+QuotesResponse 删除（domain/mod.rs）、get_quote 方法删除+`_cached_at` 重命名（cache/mod.rs）、TickerBar `.catch(()→{})`→`console.error`（2 处）、CI Node 20→22、tokio full→精简 4 features、tsconfig ES2020→ES2022、Cargo.toml authors 修正、TypeScript ~5.6.2→^5.6.2
