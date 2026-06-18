# QuantDesktop 全栈代码评审报告（终版）

> **日期**: 2025-06-18
> **分支**: master
> **提交**: v0.3.1 (cf434a7 + 五轮修复)
> **评审范围**: 全项目 — Rust 后端 (14 文件) + TypeScript/Vue 前端 (19 文件) + 配置/构建 (16 文件)
> **修复历史**: 4 Critical ✅ | 7 High ✅ | 15 Medium ✅ | 5 v0.3.1 ✅ | 7 v0.4.0 ✅ | 6 Phase 3+ ✅

---

## 目录

- [五轮修复总览](#五轮修复总览)
- [当前状态](#当前状态)
- [🟠 高优先级](#-高优先级)
- [🟡 中优先级](#-中优先级)
- [🟢 低优先级](#-低优先级)
- [统计总览](#统计总览)
- [修复建议](#修复建议)

---

## 五轮修复总览

| 轮次 | 修复数 | 关键成果 |
|------|--------|---------|
| **Critical** (v0.3.1) | 4 | CSP、数据源持久化、encoding→encoding_rs、HTTP 超时 |
| **High** (v0.3.1) | 7 | 日志框架、quote响应式、spawn_blocking、CSS变量、timer泄漏、quoteStore error、序列化日志 |
| **Medium** (v0.3.1) | 15 | 死依赖清理、日志补全(12处)、事务优化、DB错误区分、Store error、try/catch(5处)、onUnmounted、emit()/aria、error token、竞态守卫、空状态设计 |
| **v0.3.1 收尾** | 5 | expect()替换unwrap_or_default、切片守卫、volume=0修正、tsconfig strict、CI路径修正 |
| **v0.4.0** (短期) | 7 | --color-warning token、z-index scale、--radius-full、TickerBar重试、搜索错误区分、Store error消费、AbortController、死代码清理 |
| **Phase 3+** (中长期) | 6 | 公共常量提取(INDEX_CODES/USER_AGENT/TICKER_SIZE)、AtomicBool互斥锁、all_sources()通用回退、unwrap→expect、CLAUDE.md更新、HTML/CSS细节(6处) |
| **合计** | **44** | |

---

## 当前状态

### 问题趋势

```
轮次              Critical  High  Medium  Low   Total
──────────────────────────────────────────────────────
初版 (cf434a7)        4       8     13     22  =  47
第 2 轮更新            4       7     15     13  =  39
第 3 轮再检            0       6     11     24  =  41
第 4 轮 v0.3.1        0       1      9     23  =  33
第 5 轮 终版           0       0      7     18  =  25  ↓47%
```

- **Critical**: 4 → 0 — 五轮持续为零 ✅
- **High**: 8 → 0 — 首次清零 ✅
- **Medium**: 13 → 7 — 净减少 6
- **Low**: 22 → 18 — 净减少 4
- **总量**: 47 → 25 — **减少 47%**

---

## 🟠 高优先级

**本轮未发现新的 High 或 Critical 级别问题。** 首次实现零 Critical + 零 High。

---

## 🟡 中优先级（7 项）

### M1. TickerBar 重试时 timer/listener 泄漏

**文件**: [TickerBar.vue:112-120](../src/components/ticker/TickerBar.vue#L112-L120)

```typescript
// handleClick retry flow calls startCycle/startThemeListen/startDatasourceListen/startWatchlistPoll
// WITHOUT first cleaning up old timers/listeners from failed init
```

**问题**: 每次重试创建新的 interval/listener，旧的未清理。多次重试导致后台负载倍增。

**修复**: 在重试前调用已有的清理逻辑（`clearInterval` + `unlisten?.()`）。

---

### M2. 硬编码颜色在浅色主题下不一致

| 文件:行 | 问题 |
|---------|------|
| [WatchlistTable.vue:141,157](../src/components/watchlist/WatchlistTable.vue#L141) | `'#f85149'` / `'#3fb950'` 硬编码（render 函数中）— 浅色主题应为 `#d1242f` / `#1a7f64` |
| [MinuteChart.vue:244-271](../src/components/detail/MinuteChart.vue#L244) | 错误遮罩使用硬编码 `#ffa657`/`#d29922` 而非 `var(--color-warning)` token |
| [TopBar.vue:31-41](../src/components/layout/TopBar.vue#L31) | 品牌 SVG 硬编码 `#30363d` — 浅色背景下基准线不可见 |

---

### M3. Store 错误处理缺失

| 文件:行 | 问题 |
|---------|------|
| [settings.ts:24-27](../src/stores/settings.ts#L24-L27) | `setSetting()` 无 try/catch，invoke 失败后本地状态与后端脱离 |
| [settings.ts:29-36](../src/stores/settings.ts#L29-L36) | `switchDatasource()` 无 try/catch，乐观更新后可能不一致 |
| [watchlist.ts:27-34](../src/stores/watchlist.ts#L27-L34) | `addStock`/`removeStock` 无 try/catch，invoke 成功但 `fetchWatchlist` 失败后无错误反馈 |

---

### M4. Reactivity 监听不完整

| 文件:行 | 问题 |
|---------|------|
| [MinuteChart.vue:196](../src/components/detail/MinuteChart.vue#L196) | `watch(() => props.code, ...)` — 只监听 code，忽略 market |
| [DepthPanel.vue:28](../src/components/detail/DepthPanel.vue#L28) | 同上 |

---

### M5. WatchlistTable render 函数中使用硬编码颜色

**文件**: [WatchlistTable.vue:141,157](../src/components/watchlist/WatchlistTable.vue#L141)

```typescript
const color = v >= 0 ? '#f85149' : '#3fb950';
```

render 函数中返回 `h('span', { style: { color } }, ...)` 无法使用 CSS 变量（需运行时解析），但硬编码值在浅色主题下不正确。应使用 CSS class 替代 inline style。

---

### M6. MinuteChart 卸载时未 abort

**文件**: [MinuteChart.vue:189-194](../src/components/detail/MinuteChart.vue#L189-L194)

`onUnmounted` 只 dispose chart，不调用 `abortController.abort()`。组件销毁时如有进行中的 `invoke`，回调会写入已销毁的响应式状态。

---

### M7. Ticker Y 偏移不一致

**文件**: [lib.rs:129 vs 279](../src-tauri/src/lib.rs#L129)

托盘菜单中 ticker Y 偏移为 `+46`，初始启动中为 `+60`。切换 ticker 可见性时窗口会垂直跳动 14 像素。

---

## 🟢 低优先级（18 项）

### Rust 后端

| # | 文件:行 | 问题 |
|---|---------|------|
| L1 | [lib.rs](../src-tauri/src/lib.rs) | 18 处 `let _ =` 静默忽略窗口操作错误 |
| L2 | [domain/mod.rs](../src-tauri/src/domain/mod.rs#L5-L30) | `Market` enum + `from_str()` + `as_prefix()` — 从未被使用 |
| L3 | [cache/mod.rs:18,88](../src-tauri/src/cache/mod.rs#L18) | `cached_at` 字段 + `get_quote()` 方法 — `#[allow(dead_code)]` |
| L4 | [lib.rs:232-262](../src-tauri/src/lib.rs) | 窗口默认尺寸、最小尺寸、位置验证边距 — magic numbers |
| L5 | [market_clock.rs:52-56](../src-tauri/src/datasource/market_clock.rs#L52-L56) | 轮询间隔硬编码 `2/5/10/30` 秒 |
| L6 | [datasource/sina.rs:305-375](../src-tauri/src/datasource/sina.rs#L305-L375) | `fetch_depth()` 通过 Tencent API 实现 — 违反 adapter 抽象 |
| L7 | [commands/watchlist.rs:85-118](../src-tauri/src/commands/watchlist.rs#L85-L118) | `search_stocks` 中的数据源路由逻辑应属于 DataSourceManager |
| L8 | [lib.rs:23-283](../src-tauri/src/lib.rs#L23-L283) | `setup` 闭包 260 行 — 应分解为独立函数 |

### Vue 前端

| # | 文件:行 | 问题 |
|---|---------|------|
| L9 | [MinuteChart.vue:44-82](../src/components/detail/MinuteChart.vue#L44) | ~15 处 `as any` 绕过 klinecharts 类型检查 |
| L10 | [WatchlistTable.vue:260-269](../src/components/watchlist/WatchlistTable.vue#L260-L269) | 右键菜单无键盘替代（Shift+F10） |
| L11 | [TickerBar.vue:48,129](../src/components/ticker/TickerBar.vue#L48) | 轮询和 click handler 的 `.catch(() => {})` 完全吞没错误 |
| L12 | [TopBar.vue:142](../src/components/layout/TopBar.vue#L142) | `filter: brightness(1.4)` — CSS filter 触发 GPU 合成，可能有性能/模糊问题 |
| L13 | [WatchlistTable.vue:243-247](../src/components/watchlist/WatchlistTable.vue#L243-L247) | `row-props` 内联 style 字符串每次渲染重建 |

### 配置/CI

| # | 文件 | 问题 |
|---|------|------|
| L14 | [release.yml:30](../.github/workflows/release.yml#L30) | Node.js 20 已于 2026 年 4 月 EOL，应升级到 22 |
| L15 | [Cargo.toml:29](../src-tauri/Cargo.toml#L29) | `tokio = { features = ["full"] }` — 仅需 `rt-multi-thread` + `macros` + `sync` + `time` |
| L16 | [tsconfig.json:3](../tsconfig.json#L3) | `"target": "ES2020"` — Tauri 2 webview 支持 ES2022 |
| L17 | [Cargo.toml:5](../src-tauri/Cargo.toml#L5) | `authors = ["you"]` — 占位符 |
| L18 | [package.json:25](../package.json#L25) | `"typescript": "~5.6.2"` — tilde 限制太紧，应改为 `^5.6.2` |

---

## 统计总览

```
┌──────────┬──────────┬──────────┬──────────┬──────┐
│ 严重级别  │ Rust 后端 │ Vue 前端 │ 配置/构建 │ 合计  │
├──────────┼──────────┼──────────┼──────────┼──────┤
│ 🔴 Critical │    0     │    0     │    0     │  0   │
│ 🟠 High     │    0     │    0     │    0     │  0   │
│ 🟡 Medium   │    1     │    5     │    1     │  7   │
│ 🟢 Low      │    8     │    5     │    5     │ 18   │
├──────────┼──────────┼──────────┼──────────┼──────┤
│ 合计        │    9     │   10     │    6     │ 25   │
└──────────┴──────────┴──────────┴──────────┴──────┘
```

### 趋势对比

```
初版:  4🔴 + 8🟠 + 13🟡 + 22🟢 = 47
v0.3.1: 0🔴 + 0🟠 +  7🟡 + 18🟢 = 25  ↓47%
```

---

## 修复建议

### 🚨 立即（下一迭代）

| # | 问题 | 风险 |
|---|------|------|
| M1 | TickerBar 重试时清理旧 timer/listener | 内存泄漏 + 后端负载倍增 |
| M2 | WatchlistTable 硬编码涨跌色 → CSS class | 浅色主题下颜色错误 |
| M3 | Store setSetting/switchDatasource 添加 try/catch + 回滚 | 本地状态与后端不一致 |
| M4 | MinuteChart/DepthPanel watch 同时监听 code+market | 切换市场时数据不刷新 |

### 📋 短期

| # | 问题 |
|---|------|
| M5 | WatchlistTable 涨跌列改用 CSS class |
| M6 | MinuteChart onUnmounted 调用 abort |
| M7 | 统一 ticker Y 偏移（46 vs 60）|
| L2 | 清理 Market enum 死代码 |
| L9 | MinuteChart as any → 类型适配层 |

### 🔮 中长期

| # | 问题 |
|---|------|
| L1 | lib.rs setup 闭包分解 |
| L3 | 清理 `#[allow(dead_code)]` 注解 |
| L6 | SinaAdapter.fetch_depth 文档化/重构 |
| L14-L18 | CI/配置现代化（Node 22、tokio features、ES2022） |
