# QuantDesktop 全栈代码评审报告（v3 — 终版）

> **日期**: 2025-06-18
> **分支**: master
> **提交**: v0.3.1 (cf434a7 + 四轮修复)
> **评审范围**: 全项目 — Rust 后端 (14 文件) + TypeScript/Vue 前端 (23 文件) + 配置/构建 (16 文件)
> **修复历史**: 4 Critical ✅ | 7 High ✅ | 15 Medium ✅ | 5 v0.3.1 ✅

---

## 目录

- [修复历程回顾](#修复历程回顾)
- [🔴 严重问题](#-严重问题)
- [🟠 高优先级](#-高优先级)
- [🟡 中优先级](#-中优先级)
- [🟢 低优先级](#-低优先级)
- [统计总览与趋势](#统计总览与趋势)
- [最终修复建议](#最终修复建议)

---

## 修复历程回顾

| 轮次 | 修复数量 | 累计修复 |
|------|---------|---------|
| 第 1 轮 (Critical) | 4 | C1-C4: CSP、数据源持久化、encoding迁移、HTTP超时 |
| 第 2 轮 (High) | 7 | H1-H7: 日志框架、quote响应式、spawn_blocking、CSS变量、timer泄漏、Store错误处理、序列化日志 |
| 第 3 轮 (Medium) | 15 | M1-M15: 删除死依赖、日志补全（12处）、事务优化、DB错误区分、Store错误态、try/catch（5处）、onUnmounted、emit()/aria-label、error color token、useTauriEvent竞态、空状态设计 |

**三轮累计修复: 26 个问题**。问题总数从初版 47 → 本轮 51（新增发现 25 个，主要是 v0.3.0 新增组件引入的问题和配置/文档长期欠账）。

---

## 🔴 严重问题

本轮未发现新的 Critical 级别问题。

---

## 🟠 高优先级

### H1. `reqwest::Client::builder().unwrap_or_default()` 静默丢弃超时和 UA 配置

**文件**: [sina.rs:23](../src-tauri/src/datasource/sina.rs#L23) / [tencent.rs:23](../src-tauri/src/datasource/tencent.rs#L23)

```rust
client: Client::builder()
    .user_agent(USER_AGENT)
    .timeout(Duration::from_secs(10))
    .build()
    .unwrap_or_default(),  // ← 失败时创建无 UA、无超时的默认 Client
```

**问题**: 如果 TLS 后端初始化失败，`build()` 返回 `Err`，`unwrap_or_default()` 创建一个**没有 User-Agent、没有超时**的默认 Client。这会导致：
- API 请求被 Sina/Tencent 拒绝（缺 UA）
- HTTP 请求可能永久挂起（无超时）
- 问题完全静默，无法诊断

**修复**:
```rust
.build()
.expect("Failed to build reqwest Client — TLS backend missing?")
```

---

### H2. `sina.rs` 字符串切片 panic 风险

**文件**: [sina.rs:55](../src-tauri/src/datasource/sina.rs#L55)

```rust
let code = code_raw[2..].to_string();  // 如果 code_raw 长度 < 3 则 panic
```

**对比**: [tencent.rs:44](../src-tauri/src/datasource/tencent.rs#L44) 有正确的 `.len() >= 2` 守卫：
```rust
let code = if code_raw.len() >= 2 { code_raw[2..].to_string() } else { code_raw.to_string() };
```

**修复**: 给 sina.rs 添加相同的长度守卫。

---

### H3. `WatchlistTable.vue` — `volume=0` 和 `turnover=0` 被当作无数据显示 `--`

**文件**: [WatchlistTable.vue:171,187](../src/components/watchlist/WatchlistTable.vue#L171)

```typescript
if (!q || !q.volume) return '--';    // volume=0 → 显示 --
if (!q || !q.turnover) return '--';  // turnover=0 → 显示 --
```

**问题**: 盘前或停牌时 `volume=0` 是合法值，但 falsy 检查 `!0 === true` 导致显示 `--` 而非 `0手`。

**修复**: `q.volume == null ? '--' : formatVolume(q.volume)`

---

### H4. `MinuteChart.vue` — ~18 处 `as any` 绕过 klinecharts 类型检查

**文件**: [MinuteChart.vue:43,47-54,59-61,67-68,73,75,80-81,161,163](../src/components/detail/MinuteChart.vue)

**问题**: v0.3.0 新增的分时图组件大量使用 `as any` 绕过库的类型定义。如果 klinecharts 升级 API，这些将在运行时静默失败。

**建议**: 创建 `chart-styles.ts` 类型辅助文件，将 `as any` 集中在单一的类型适配层。

---

### H5. `tsconfig.node.json` 缺少 `strict: true`

**文件**: [tsconfig.node.json](../tsconfig.node.json)

`vite.config.ts` 在**无严格类型检查**的环境下编译。应添加 `"strict": true` 和 `"types": ["node"]`（解决 `@ts-expect-error` 对 `process.env` 的抑制）。

---

### H6. `release.yml` — Windows 构建产物路径错误

**文件**: [release.yml:87-88](../.github/workflows/release.yml#L87-L88)

```yaml
src-tauri/target/${{ matrix.target }}/release/bundle/nsis/*.exe
```

Windows MSVC 作为 host target 时，Cargo 不会创建 `x86_64-pc-windows-msvc` 子目录，产物在 `target/release/`。此路径会导致 `if-no-files-found: error`（第 89 行）触发 CI 失败。

---

## 🟡 中优先级

### M1. AppLayout 错误横幅使用硬编码颜色

**文件**: [AppLayout.vue:72-133](../src/components/layout/AppLayout.vue#L72)

```css
color: #ffa657;           /* 琥珀色硬编码 */
background: rgba(255, 166, 87, 0.08);
```

这些颜色既不在 `variables.css` 中定义，也不会随主题切换。应提取为 `--color-warning` / `--color-warning-bg` token。

---

### M2. TickerBar 初始化失败后无恢复机制

**文件**: [TickerBar.vue:123](../src/components/ticker/TickerBar.vue#L123)

`initFailed = true` 时只显示 "QuantDesktop" 文本，无法重试。用户必须重启应用才能恢复。

---

### M3. `AddStockDialog` 搜索失败静默显示"未找到"

**文件**: [AddStockDialog.vue:35](../src/components/watchlist/AddStockDialog.vue#L35)

```typescript
} catch {
  results.value = [];  // 网络错误 → 显示"未找到匹配标的"
}
```

用户无法区分"无结果"和"搜索失败"。

---

### M4. `settings.ts` / `watchlist.ts` 错误状态未在 UI 消费

- `watchlistStore.error` 已定义但 WatchlistTable 不读取
- `settingsStore` 无 error ref，`fetchSettings` 失败静默

---

### M5. 死代码（4 处）

| 文件 | 内容 |
|------|------|
| [useTauriEvent.ts](../src/composables/useTauriEvent.ts) | 整个文件从未被导入（23 行） |
| [useTheme.ts](../src/composables/useTheme.ts) | 整个文件从未被导入（17 行） |
| [watchlist.ts](../src/stores/watchlist.ts#L37) | `reorder()` 方法从未被调用 |
| [types/index.ts](../src/types/index.ts#L39) | `WatchItem.sort_order` 从未在前端读取 |

---

### M6. `MinuteChart` 快速切换股票时无防抖/竞态保护

**文件**: [MinuteChart.vue:179](../src/components/detail/MinuteChart.vue#L179)

```typescript
watch(() => props.code, () => { loadData(); });
```

快速点击多只股票时，多个 `loadData()` 并发执行，可能闪现错误股票的数据。

---

### M7. 设计系统不完整

| 文件 | 问题 |
|------|------|
| [variables.css](../src/assets/styles/variables.css) | 缺少 `--radius-full`、`--color-warning`、z-index scale |
| [dark.css](../src/assets/styles/dark.css) | 文件名误导（实际只是 scrollbar 样式） |
| [TopBar.vue:142](../src/components/layout/TopBar.vue#L142) | hover 使用 `filter: brightness(1.4)`，跨主题行为不可控 |
| [index.html](../index.html) | 标题仍是 "Tauri + Vue + Typescript App"、favicon 是 Vite 默认 |

---

### M8. 硬编码颜色（10+ 处）

| 文件 | 类型 |
|------|------|
| [WatchlistTable.vue:141,157](../src/components/watchlist/WatchlistTable.vue#L141) | `'#f85149'` / `'#3fb950'` 涨跌色（render 函数中） |
| [MinuteChart.vue:227](../src/components/detail/MinuteChart.vue#L227) | `rgba(22,27,34,0.92)` 暗色专属背景 |
| [TopBar.vue:30-41](../src/components/layout/TopBar.vue#L30) | 品牌 SVG 硬编码色 |
| [TickerBar.vue:153](../src/components/ticker/TickerBar.vue#L153) | `rgba(255,255,255,0.03)` hover 仅适配暗色 |

---

### M9. 重复代码

| 内容 | 文件 |
|------|------|
| 指数代码列表（7 个指数） | [sina.rs:187](../src-tauri/src/datasource/sina.rs#L187) + [tencent.rs:162](../src-tauri/src/datasource/tencent.rs#L162) |
| `USER_AGENT` 常量 | [sina.rs:10](../src-tauri/src/datasource/sina.rs#L10) + [tencent.rs:10](../src-tauri/src/datasource/tencent.rs#L10) |
| Ticker 窗口尺寸 `230, 38` | [lib.rs:127,277](../src-tauri/src/lib.rs) 两处硬编码 |
| `contextmenu` 事件抑制 | [main.ts:8](../src/main.ts#L8) + [ticker.ts:9](../src/ticker.ts#L9) |

---

### M10. `search()` 实现不完整

**文件**: [sina.rs:212-246](../src-tauri/src/datasource/sina.rs#L212-L246) / [tencent.rs:183-215](../src-tauri/src/datasource/tencent.rs#L183-L215)

两个数据源的 `search()` 只支持 6 位数字代码精确查询，不支持名称模糊搜索。而 trait 签名暗示支持 fuzzy search。

---

### M11. 双 fetch 循环可导致并发 API 请求

**文件**: [cache/mod.rs:126-152](../src-tauri/src/cache/mod.rs#L126-L152)

Scheduler 同时运行 interval 循环和 wakeup 监听循环，两者都调用 `fetch_once`。数据源切换触发 wakeup 时，可能与 interval 计时器同时发起 API 请求，造成浪费。

---

## 🟢 低优先级

### L1. Magic Numbers（~20 个硬编码值）

| 文件:行 | 典型值 |
|---------|--------|
| [lib.rs](../src-tauri/src/lib.rs) | `1100/680` 窗口尺寸、`400/300` 最小尺寸、`200/100/-200/-50` 位置验证边距 |
| [market_clock.rs](../src-tauri/src/datasource/market_clock.rs) | `2/5/10/30` 轮询间隔、`9:30/11:30/13:00/15:00` 交易时段 |
| [sina.rs](../src-tauri/src/datasource/sina.rs) | `scale=5&datalen=240` 硬编码在 URL 中 |

### L2. 4 处裸 `unwrap()` — [market_clock.rs:31-34](../src-tauri/src/datasource/market_clock.rs#L31-L34)

```rust
NaiveTime::from_hms_opt(9, 30, 0).unwrap();
```
值本身有效不会 panic，但应改为 `.expect("valid time constant")`。

### L3. lib.rs 中 19 处 `let _ =` 窗口操作静默失败

**文件**: [lib.rs](../src-tauri/src/lib.rs)

托盘菜单和窗口 show/hide/set_position 等操作全部用 `let _ =` 忽略错误。虽然这些操作极少失败，但无日志导致问题不可诊断。

### L4. CLAUDE.md 过时内容

- `encoding` 应改为 `encoding_rs`
- `init_defaults` 描述（"forces sina on every startup"）已在 C2 修复中改变
- 建议 `npm ci` 替代 `npm install`
- Phase 3-4 路线图已陈旧

### L5. `@types/node@^25.9.3` 版本可疑

Node.js v25 不存在（2026 年最新 LTS 为 24.x），需验证此版本号是否有效。

### L6. `index.html` / `ticker.html` lang 不一致

`index.html` 用 `lang="en"`，`ticker.html` 用 `lang="zh-CN"`。应统一为 `zh-CN`。

### L7. `watchlist.rs` 回退逻辑脆弱

```rust
let fallback = if active_name == "sina" { "tencent" } else { "sina" };
```
写死了两个数据源名称，不支持第三个源。

### L8. 无障碍性动画问题

[IndexBar.vue:44](../src/components/index/IndexBar.vue#L44) — pulse 动画未尊重 `prefers-reduced-motion`。

### L9. CSS 变量回退值不一致

多处使用 `var(--color-text-primary, #e0e0e0)` 但实际 token 为 `#e6edf3`；回退值永远不生效但数值错误。

### L10. `--tracking-tight` 未定义变量

[WatchlistTable.vue:286](../src/components/watchlist/WatchlistTable.vue#L286) — 引用了不存在的 CSS 变量，规则无效。

---

## 统计总览与趋势

```
┌──────────┬──────────┬──────────┬──────────┬──────┐
│ 严重级别  │ Rust 后端 │ Vue 前端 │ 配置/构建 │ 合计  │
├──────────┼──────────┼──────────┼──────────┼──────┤
│ 🔴 Critical │    0     │    0     │    0     │  0   │
│ 🟠 High     │    2     │    2     │    2     │  6   │
│ 🟡 Medium   │    3     │    4     │    4     │ 11   │
│ 🟢 Low      │    8     │    6     │   10     │ 24   │
├──────────┼──────────┼──────────┼──────────┼──────┤
│ 合计        │   13     │   12     │   16     │ 41   │
└──────────┴──────────┴──────────┴──────────┴──────┘
```

### 四轮评审趋势

```
              Critical  High  Medium  Low   Total
第 1 轮 (初版):   4       8     13     22  =  47
第 2 轮 (更新):   4       7     15     13  =  39  ↓17%
第 3 轮 (再检):   0       6     11     24  =  41
第 4 轮 (v0.3.1): 0       1      9     23  =  33  ↓20%
```

- **Critical**: 4 → 0 — 四轮持续为零 ✅
- **High**: 8 → 1 — H4 (MinuteChart as any) 为长期改进项
- **Medium**: 13 → 9 — 硬编码颜色、死代码、search() 增强为长期项
- **Low**: 22 → 23 — 基本持平

v0.3.1 收尾修复了 5 个高优先级问题，High 从 6 降至 1。

---

## 最终修复建议

### 🚨 立即（v0.3.1 收尾）— ✅ 全部完成

| # | 问题 | 状态 |
|---|------|------|
| H1 | `unwrap_or_default()` on Client builder → `expect()` | ✅ 已修复 |
| H2 | sina.rs `code_raw[2..]` 添加长度守卫 | ✅ 已修复 |
| H3 | volume/turnover=0 显示修正 | ✅ 已修复 |
| H5 | tsconfig.node.json 添加 strict + types | ✅ 已修复 |
| H6 | release.yml Windows 产物路径修正 | ✅ 已修复 |

### 📋 短期（v0.4.0）

| # | 问题 |
|---|------|
| M1 | AppLayout 硬编码颜色 → warning token |
| M2 | TickerBar initFailed 添加重试机制 |
| M3 | AddStockDialog 搜索错误区分 |
| M4 | 消费 watchlistStore.error / settingsStore 添加 error |
| M5 | 清理 4 处死代码 |
| M6 | MinuteChart 添加 AbortController 防竞态 |
| H4 | MinuteChart as any 集中到类型适配层 |
| M7 | 完善设计系统（--radius-full、--color-warning、z-index） |

### 🔮 中长期（Phase 3+）

| # | 问题 |
|---|------|
| M9 | 提取公共常量（指数代码、USER_AGENT、ticker 尺寸） |
| M10 | search() 实现名称模糊搜索 |
| M11 | 添加 fetch 去重/互斥锁 |
| L1-L10 | Magic numbers 整理、文档更新、无障碍动画、CSS 一致性 |

---

## ✅ 已确认修复成功（四轮累计 31 项）

| 轮次 | 问题 |
|------|------|
| Critical | CSP安全策略、active_datasource持久化、encoding→encoding_rs迁移（8处）、HTTP超时（2处） |
| High | log+simplelog双输出日志、quote→computed()、get_watch_codes spawn_blocking、--radius-full回退、debounceTimer清理、quoteStore error ref、序列化warn日志、persist_quotes spawn_blocking、窗口DB写入warn（8处）、emit失败warn（6处）、toggleTheme await、事件监听console.error、CSS token定义（4个）、TopBar color-accent-dim |
| Medium | env_logger删除、create_dir_all日志、缓存行反序列化日志、搜索失败日志、cache_quotes事务、DB错误区分（4分支）、watchlistStore error ref、WatchlistTable try/catch（4处）、App.vue onUnmounted、emit()替代$emit、switchDatasource同步settings.value、useTauriEvent竞态守卫、aria-label（5处）、--color-error token、emoji→SVG（2处）、error颜色语义修正 |
| v0.3.1 | reqwest Client expect()（2处）、sina.rs切片长度守卫、volume/turnover=0修正、tsconfig.node.json strict、vite.config.ts @ts-expect-error移除、release.yml Windows路径修正 |
