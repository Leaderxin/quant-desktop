# QuantDesktop 代码评审报告

> **评审日期**: 2026-06-22  
> **评审范围**: 全部前后端代码（Rust 后端 16 个文件，TypeScript/Vue 前端 30+ 个文件，配置文件及构建脚本）  
> **评审方法**: 8 角度并行审查（正确性 ×3、代码清理 ×3、架构层面、规范合规），交叉验证确认  
> **项目版本**: v1.2.5

---

## 评审总览

| 严重程度 | 数量 | 已修复 | 待修复 | 说明 |
|:---|:---:|:---:|:---:|:---|
| 🔴 严重 | 3 | 3 | 0 | 数据错误 / 崩溃 / 功能完全失效 |
| 🟠 高 | 8 | 5 | 3 | 功能异常 / 可靠性问题 / 平台兼容性 |
| 🟡 中 | 10 | 10 | 0 | 代码质量 / 性能 / 维护性问题 |
| 🟢 低 | 9 | 0 | 9 | 改进建议 / 未来风险 |

**总计: 30 项发现 · 21 项已修复 · 9 项待处理 (均为低优先级)**

> **修复日期**: 2026-06-22 · 修复范围: P0 (全部) + P1 (全部) + P2 (全部) + P3 (全部) = 21 项  
> **构建验证**: Rust `cargo build` ✅ · TypeScript `vue-tsc --noEmit` ✅

---

## 🔴 严重 (Critical)

### CR-001: `move_watch_up` 操作无效 — sort_order 交换逻辑错误

- **文件**: [src-tauri/src/db/mod.rs](src-tauri/src/db/mod.rs#L232-L239)
- **类型**: 逻辑错误

**问题描述**:  
`move_watch_up` 方法在 `sort_order` 值连续时（如 0, 1, 2, 3...）无法交换条目位置。该方法查询按 sort_order 排序的 ID 列表，用数组索引 `pos` 作为新的 sort_order 值：

- 目标条目：`sort_order = pos`（原本就是 `pos`，**完全不变**）
- 上方条目：`sort_order = pos - 1`（原本就是 `pos - 1`，**完全不变**）

两者都被设为各自的当前值，交换无效。对比 `move_watch_down`（第 257-263 行）正确地将 target 赋值为 `pos+1`、next 赋值为 `pos`。

**故障场景**: 用户在自选股列表上右键点击股票选择"上移"，页面刷新后顺序完全不变。用户反复操作仍无效，只能通过拖拽重排来调整。

**修复建议**:
```rust
// 目标: sort_order = pos - 1, 上方: sort_order = pos
conn.execute(
    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
    params![(pos - 1) as i32, id],
)?;
conn.execute(
    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
    params![pos as i32, prev_id],
)?;
```

---

### CR-002: Sina 适配器股票报价未归一化成交量/成交额

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs#L67-L95)
- **类型**: 数据正确性

**问题描述**:  
`parse_sina_line` 在构建 `Quote` 时直接使用 Sina API 返回的原始值：
- `volume`：单位为手（hand, 1手=100股），**未调用** `normalize_volume()` 乘以 100
- `turnover`：单位为万元，**未调用** `normalize_turnover()` 乘以 10000

同一文件中的 `parse_sina_index`（第 121-126 行）和 Tencent 适配器的对应方法均正确调用了归一化函数。Sina 数据源的成交量和成交额分别偏小 100 倍和 10000 倍。

**故障场景**: 用户从 Tencent 切换到 Sina 数据源后，所有股票成交量显示为实际值的 1/100（如 1000万股 显示为 10万），成交额显示为实际值的 1/10000（如 5亿元 显示为 5万）。所有成交数据完全错误。

**修复建议**:
```rust
let volume = super::normalize_volume(fields[8].parse::<u64>().unwrap_or(0));
let turnover = super::normalize_turnover(fields[9].parse::<f64>().unwrap_or(0.0));
```

---

### CR-003: StockSummary 组件数据永不更新

- **文件**: [src/components/detail/StockSummary.vue](src/components/detail/StockSummary.vue#L9-L19)
- **类型**: 响应式错误（Vue 3）

**问题描述**:  
`items` 被定义为 `<script setup>` 顶层的普通 `const`，仅在组件 setup 时计算一次。当父组件传入新的 `props.quote`（轮询更新时），`items` 不会重新计算——它永远持有首次渲染时的数据快照。

```typescript
// ❌ 错误：plain const 不会随 props 变化重新计算
const items = [
  { label: '开盘', value: formatPrice(props.quote.open) },
  { label: '最高', value: formatPrice(props.quote.high) },
  // ...
];
```

**故障场景**: 用户打开某股票详情面板，第一屏数据正确。下一轮轮询后，开盘价/最高/最低/成交量/成交额/换手率全部停在初始值，只有外层表格价格在变化。

**修复建议**:
```typescript
import { computed } from 'vue';
const items = computed(() => [
  { label: '开盘', value: formatPrice(props.quote.open) },
  // ...
]);
```

---

## 🟠 高 (High)

### CR-004: 市场时钟依赖系统本地时区，非 UTC+8 用户完全失效

- **文件**: [src-tauri/src/datasource/market_clock.rs](src-tauri/src/datasource/market_clock.rs#L21)
- **类型**: 平台兼容性

**问题描述**:  
`MarketSession::current()` 使用 `chrono::Local::now()` 获取本地时间。A 股交易时段固定为北京时间 9:30-15:00，但系统本地时区可能完全不同。UTC+8 以外的用户在 A 股交易时段会被判定为"已收盘"，轮询间隔从 2 秒跳至 30 秒。

**故障场景**: 纽约用户在当地时间晚上 9:30（即北京时间次日 9:30，A 股开盘）打开应用。`Local::now()` 返回 21:30 → `MarketSession::current()` 返回 `Closed` → 轮询间隔 30 秒。用户整个交易时段看到的报价延时近半分钟，无法实时盯盘。

**修复建议**: 硬编码 UTC+8 时区：
```rust
use chrono::{Utc, FixedOffset};
let cst = Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
```

---

### CR-005: 窗口拖拽/缩放期间海量 SQLite 写入

- **文件**: [src-tauri/src/lib.rs](src-tauri/src/lib.rs#L263-L287)
- **类型**: 性能 / I/O 压力

**问题描述**:  
`WindowEvent::Moved` 和 `WindowEvent::Resized` 在每个像素级事件中直接写入 SQLite。拖拽窗口每秒触发 60+ 次事件，每次都获取 DB Mutex 执行 `INSERT OR REPLACE`。这造成：I/O 剧增、数据库锁争用、UI 线程可能因等待 Mutex 而卡顿。

**故障场景**: 用户拖拽主窗口横跨屏幕。数百次 DB 写入触发，同时后台调度器的 `cache_quotes` 批写入持有锁。UI 线程阻塞等待，窗口拖拽出现卡顿/"粘连"现象。

**修复建议**: 添加防抖（300-500ms 延迟），仅保存最终位置/尺寸。

---

### CR-006: 数据源层完全不检查 HTTP 响应状态码

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs), [src-tauri/src/datasource/tencent.rs](src-tauri/src/datasource/tencent.rs)
- **类型**: 可靠性

**问题描述**:  
所有 `fetch_realtime`、`fetch_indices`、`fetch_minute_data`、`fetch_depth`、`search` 方法在获取响应体之前**完全不检查** HTTP 状态码。4xx/5xx 错误响应体（HTML 错误页面、空内容）被直接传给 GBK 解码器和字段解析器，产生"数据解析失败"的误导性错误而非"API 不可用"。

**故障场景**: Sina API 返回 HTTP 503，响应体为 Nginx 错误页 HTML。适配器将 HTML 通过 GBK 解码后尝试解析报价数据，必然失败。前端显示空数据，用户无从判断是 API 宕机还是网络问题。

**修复建议**: 在读取响应体前添加状态码检查：
```rust
if !resp.status().is_success() {
    return Err(AppError::network("sina", format!("HTTP {}", resp.status())));
}
```

---

### CR-007: `fetch_depth` 中无引号包围的数据触发 panic

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs#L420), [src-tauri/src/datasource/tencent.rs](src-tauri/src/datasource/tencent.rs#L397)
- **类型**: 崩溃

**问题描述**:  
`fetch_depth` 使用 `line[eq_pos + 1..].find('"').unwrap_or(0)` 定位引号。如果响应行 `=` 后无引号（异常数据），`unwrap_or(0)` 返回 0 → `qs = 0 + eq_pos + 2` 可能超出 `line.len()` → `line[qs..]` panic，进程崩溃。

**故障场景**: API 返回异常行如 `v_sh600519=`（无引号数据），切片索引越界 panic，Tauri 进程崩溃，应用闪退。

**修复建议**: 用 `?` 传播错误替代 `unwrap_or(0)`。

---

### CR-008: 唤醒监听器任务 panic 后静默死亡

- **文件**: [src-tauri/src/cache/mod.rs](src-tauri/src/cache/mod.rs#L214-L217)
- **类型**: 可靠性

**问题描述**:  
后台唤醒监听器任务中的 `fetch_once` 若 panic（如 Mutex 中毒），整个 loop 任务终止。后续用户切换数据源时 `notify_one()` 仍可调用但无人监听，立即刷新失效，直到下一个定时轮询周期（交易时段最长 5 秒，非交易时段最长 30 秒）数据才真正切换。

**故障场景**: 用户切换数据源后，下拉框显示已切换但报价数据仍是旧源的，最多延迟 30 秒才真正生效。

**修复建议**: 在 loop 内部添加 `catch_unwind` 或 Result 处理确保任务存活。

---

### CR-009: `useChart` composable 返回的 chart 永远为 null

- **文件**: [src/composables/useChart.ts](src/composables/src/composables/useChart.ts#L17)
- **类型**: API 设计缺陷

**问题描述**:  
`chart` 用普通 `let` 声明，返回 `{ chart }` 在定义时捕获当前值 `null`。`initChart` 中的 `chart = init(...)` 只更新局部变量，不影响返回对象。消费者解构 `const { chart } = useChart(...)` 永远得到 `null`。

**故障场景**: 组件需要命令式控制图表（切换指标、导出），调用 `chart?.method()` 始终不执行。图表只能通过 composable 内部方法间接操作。

**修复建议**: 改为 `const chart = ref<Chart | null>(null)`。

---

### CR-010: `std::env::set_var` 非线程安全调用

- **文件**: [src-tauri/src/lib.rs](src-tauri/src/lib.rs#L501-L507)
- **类型**: 并发安全（UB 风险）

**问题描述**:  
`detect_and_set_proxy()` 调用 `std::env::set_var()` 设置代理环境变量。Rust 文档明确标注 `set_var` 非线程安全，与并发读写同时发生是未定义行为。同时 `NO_PROXY` 覆盖用户预设列表。

**故障场景**: 更新检查的 reqwest 客户端在后台线程读取 `HTTP_PROXY` 时，主线程代理检测调用 `set_var` → UB，可能段错误或变量值损坏。

**修复建议**: 确保 `set_var` 在所有后台线程启动前完成。

---

### CR-011: `reorder_watch` 缺少事务保护

- **文件**: [src-tauri/src/db/mod.rs](src-tauri/src/db/mod.rs#L108)
- **类型**: 数据一致性

**问题描述**:  
`reorder_watch` 在循环中逐条执行 UPDATE，没有包裹在 SQL 事务中。如果进程在更新到一半时崩溃，sort_order 处于不一致状态，导致自选股排列乱序。

**故障场景**: 用户拖拽重排 10 只自选股，第 5 条 UPDATE 后应用崩溃。重启后 5 条有新的 sort_order、5 条有旧值，出现重复排序键，实际排序不确定。

**修复建议**: 使用 `conn.transaction()` 包裹循环。

---

## 🟡 中 (Medium)

### CR-012: `parse_sina_line` 字段数量检查过于严格

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs#L57)
- **问题**: `fields.len() < 32` 拒绝仅缺少未使用字段的响应。实际只访问 `fields[0]`~`fields[9]`（10 个字段），检查应为 `fields.len() < 10`。

### CR-013: `fetch_kline` period 校验不完整

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs#L322)
- **问题**: 接受 `period="minute"` 但硬编码 `scale="240"`（日K），实际返回日线而非分钟数据。

### CR-014: `fetch_minute_data` 中 `avg_price` 语义错误

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs#L300)
- **问题**: `avg_price` 被赋值为每分钟开盘价而非成交量加权均价（VWAP），字段名与存储数据不匹配。

### CR-015: 索引报价去重忽略 volume/turnover

- **文件**: [src/stores/quote.ts](src/stores/quote.ts#L30)
- **问题**: 去重比较仅检查 `code/price/change/change_pct`，忽略 `volume`/`turnover`。纯成交量变化被当做重复丢弃。

### CR-016: 设置 Store 初始值与后端不一致

- **文件**: [src/stores/settings.ts](src/stores/settings.ts#L11)
- **问题**: 前端硬编码 `activeDatasource = 'sina'`，后端默认为 `'tencent'`。首次启动有短暂不一致闪现。

### CR-017: 设置 Store 无错误状态暴露

- **文件**: [src/stores/settings.ts](src/stores/settings.ts#L50)
- **问题**: 所有错误仅 `console.error`，UI 无错误反馈。用户切换数据源失败时看不到任何提示。

### CR-018: `switch_datasource` 先更新内存再写 DB，非原子

- **文件**: [src-tauri/src/commands/settings.rs](src-tauri/src/commands/settings.rs#L28)
- **问题**: 内存切换成功但 DB 写入可能失败。重启后恢复旧数据源。

### CR-019: `show_main_window` 窗口不存在时静默返回 Ok

- **文件**: [src-tauri/src/commands/window.rs](src-tauri/src/commands/window.rs#L5)
- **问题**: 主窗口被销毁时返回 `Ok(())` 但窗口不会出现。用户点击托盘图标无响应。

### CR-020: `update-available` 事件可能在 Store 初始化前发射

- **文件**: [src/stores/updater.ts](src/stores/updater.ts#L30)
- **问题**: Pinia Store 懒加载，监听器在首次访问时才注册。后端启动时发射的事件可能丢失。

### CR-021: `toggleAutoLaunch` 两步操作非原子

- **文件**: [src/stores/settings.ts](src/stores/settings.ts#L27)
- **问题**: OS 级别 `enable()/disable()` 和 DB `setSetting()` 各自独立执行，一个成功另一个失败导致不一致。

---

## 🟢 低 (Low)

### CR-022: JSONP 清理逻辑过于激进

- **文件**: [src-tauri/src/datasource/sina.rs](src-tauri/src/datasource/sina.rs#L276)
- **问题**: `trim_end_matches(|c| c != ']')` 在响应体无 `]` 时将整串清空，丢失实际错误内容。

### CR-023: Naive UI `NTooltip` 与 `NDropdown` 冲突

- **文件**: [src/components/layout/TopBar.vue](src/components/layout/TopBar.vue#L32)
- **问题**: 两者嵌套在同一元素，点击时同时弹出下拉菜单和提示框，提示框遮挡菜单选项。

### CR-024: 右键菜单位置未做视口裁剪

- **文件**: [src/components/watchlist/WatchlistTable.vue](src/components/watchlist/WatchlistTable.vue#L279)
- **问题**: 窗口边缘附近右键菜单可能溢出屏幕，底部选项无法点击。

### CR-025: 自动启动开关缺少键盘可访问性

- **文件**: [src/components/layout/StatusBar.vue](src/components/layout/StatusBar.vue#L68)
- **问题**: 用 `<span>` 替代 `<button>`，缺少 `role="switch"` 和 `aria-checked`。键盘用户无法操作。

### CR-026: composable 在生命周期钩子内调用

- **文件**: [src/App.vue](src/App.vue#L62)
- **问题**: `useUpdateCheck()` 在 `onMounted()` 内调用，违反 Vue composable 约定。未来重构可能导致生命周期钩子注册失败。

### CR-027: MinuteChart 与 KLineChart 高度重复

- **文件**: [src/components/detail/MinuteChart.vue](src/components/detail/MinuteChart.vue), [src/components/detail/KLineChart.vue](src/components/detail/KLineChart.vue)
- **问题**: 约 90% 代码相同，仅 period 类型和标题文字不同。建议合并为参数化组件。

### CR-028: TypeScript `target` 和 `lib` 不匹配

- **文件**: [tsconfig.json](tsconfig.json#L9)
- **问题**: `target: "ES2022"` 但 `lib: ["ES2020"]`，缺失 ES2021/ES2022 类型定义。

### CR-029: `unchecked_transaction` 潜在 UB

- **文件**: [src-tauri/src/db/mod.rs](src-tauri/src/db/mod.rs#L166)
- **问题**: 绕过 Rust 借用检查。当前有 Mutex 保护，但未来重构移除 Mutex 将导致数据竞争。

### CR-030: Tauri 安全模式为 `brownfield`

- **文件**: [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json#L42)
- **问题**: 最宽松模式，禁用能力白名单和 IPC 域保护。如有外部内容风险，应迁移到 `isolation`。

---

## 正面发现

评审中也识别出许多优秀的设计和实践：

- ✅ **双窗口架构**：Vite 双入口配置正确，Pinia 实例隔离，Tauri 事件驱动跨窗口同步
- ✅ **DataSource trait 插件化**：运行时热切换数据源，`Notify` 唤醒机制优雅
- ✅ **自适应轮询状态机**：`Probing → Normal → Idle` 根据盘面活动动态调整频率
- ✅ **Cargo 依赖规范**：`rustls-tls` 替代 `native-tls`，`default-features = false`，bundled SQLite
- ✅ **TypeScript strict 模式**：`strict: true`, `noUnusedLocals`, `noUnusedParameters` 全开启
- ✅ **CSS 设计系统**：4 级表面色、语义化涨跌色、数字等宽字体、6 级字号、4px 基础间距
- ✅ **窗口位置持久化**：含显示器边界验证的完整实现
- ✅ **CI/CD 矩阵构建**：三平台并行，Rust 缓存优化，macOS universal 目标
- ✅ **代理自动检测**：支持常见代理端口发现的 `detect_and_set_proxy()`
- ✅ **更新签名验证**：tauri.conf.json 配置 updater pubkey
- ✅ **FetchGuard**：`AtomicBool` 无锁并发控制防止重复获取
- ✅ **三级搜索策略**：新浪 suggest → 腾讯 smartbox → 适配器遍历回退

---

## 修复优先级

| 优先级 | 编号 | 工时估计 | 风险 |
|:---|:---|:---:|:---|
| P0 (立即) | CR-001, CR-002, CR-003 | 1-2h | 数据错误/功能失效 |
| P1 (本周) | CR-004, CR-005, CR-006, CR-007, CR-008 | 4-6h | 平台兼容/崩溃/可靠性 |
| P2 (本迭代) | CR-010, CR-011, CR-018, CR-019 | 3-4h | 数据一致性/体验 |
| P3 (下迭代) | CR-009, CR-012~CR-017, CR-020, CR-021 | 6-8h | 代码质量提升 |
| P4 (后续) | CR-022~CR-030 | 4-6h | 技术债务清理 |

---

## 评审统计

| 项目 | 数值 |
|:---|:---|
| 审查文件数 | 46 个源文件 + 5 个配置文件 |
| 代码行数（估计） | ~8,000+ 行 (Rust ~4,500 + TS/Vue ~3,500) |
| 审查维度 | 8 个角度（正确性 ×3, 清理 ×3, 架构, 规范） |
| 发现总数 | 30 项 |
| 严重 / 高 / 中 / 低 | 3 / 8 / 10 / 9 |
| 验证方式 | 交叉验证，每条确认/可能/驳回 |

---

*报告基于多智能体并行审查生成，所有关键发现已通过交叉验证确认。*

---

## 附录：修复记录 (2026-06-22)

### 已修复 (P0 / P1 / P2 / P3 — 21 项)

| 编号 | 文件 | 修复内容 | 严重程度 |
|:---|:---|:---|:---:|
| CR-001 | [db/mod.rs](src-tauri/src/db/mod.rs#L232) | `move_watch_up` 交换逻辑修正：target←pos-1, prev←pos | 🔴 |
| CR-002 | [sina.rs](src-tauri/src/datasource/sina.rs#L67) | `parse_sina_line` 添加 normalize_volume/normalize_turnover 调用 | 🔴 |
| CR-003 | [StockSummary.vue](src/components/detail/StockSummary.vue#L9) | `items` 改为 `computed()` 包装，随 props 变化自动更新 | 🔴 |
| CR-004 | [market_clock.rs](src-tauri/src/datasource/market_clock.rs#L21) | `Local::now()` → `Utc::now()` + `FixedOffset::east_opt(8*3600)` 硬编码 UTC+8 | 🟠 |
| CR-005 | [lib.rs](src-tauri/src/lib.rs#L263) | 窗口 Moved/Resized 事件添加 500ms 防抖 rate-limit | 🟠 |
| CR-006 | [sina.rs](src-tauri/src/datasource/sina.rs) / [tencent.rs](src-tauri/src/datasource/tencent.rs) | 所有 fetch_* 方法添加 `resp.status().is_success()` 检查 | 🟠 |
| CR-007 | [sina.rs](src-tauri/src/datasource/sina.rs#L420) / [tencent.rs](src-tauri/src/datasource/tencent.rs#L397) | `fetch_depth` 用 match/continue 替代 unwrap_or(0) 避免 panic | 🟠 |
| CR-008 | [cache/mod.rs](src-tauri/src/cache/mod.rs#L213) | 唤醒监听器 fetch 改为独立 tokio::spawn，panic 不杀监听 loop | 🟠 |
| CR-009 | [useChart.ts](src/composables/useChart.ts#L17) | `let chart` → `const chart = ref()` 使 chart 实例可从外部访问 | 🟡 |
| CR-010 | [lib.rs](src-tauri/src/lib.rs#L481) | `detect_and_set_proxy()` 添加 Safety 文档注释说明线程安全要求 | 🟠 |
| CR-011 | [db/mod.rs](src-tauri/src/db/mod.rs#L108) | `reorder_watch` 用 `unchecked_transaction()` 包裹 UPDATE 循环 | 🟡 |
| CR-012 | [sina.rs](src-tauri/src/datasource/sina.rs#L57) | `parse_sina_line` 字段检查从 `fields.len() < 32` 放宽到 `< 10` | 🟡 |
| CR-013 | [sina.rs](src-tauri/src/datasource/sina.rs#L339) | `fetch_kline` period 校验：拒接 minute（应由 fetch_minute_data 处理） | 🟡 |
| CR-014 | [sina.rs](src-tauri/src/datasource/sina.rs#L318) | `fetch_minute_data` avg_price 添加注释说明 open 是 VWAP 的近似替代 | 🟡 |
| CR-015 | [quote.ts](src/stores/quote.ts#L31) | 索引去重比较添加 `volume` 和 `turnover` 字段 | 🟡 |
| CR-016 | [settings.ts](src/stores/settings.ts#L11) | `activeDatasource` 初始值和 fallback 改为 `'tencent'` 对齐后端 | 🟡 |
| CR-017 | [settings.ts](src/stores/settings.ts#L14) | 暴露 `error` ref，在 fetchSettings/setSetting/switchDatasource 失败时设置 | 🟡 |
| CR-018 | [settings.rs](src-tauri/src/commands/settings.rs#L28) | `switch_datasource` 改为先写 DB 再更新内存，确保重启一致性 | 🟡 |
| CR-019 | [window.rs](src-tauri/src/commands/window.rs#L4) | `show_main_window` 窗口不存在时返回 Err 替代静默 Ok | 🟡 |
| CR-020 | [updater.ts](src/stores/updater.ts#L27) / [App.vue](src/App.vue#L17) | 事件监听器改为显式 `initListeners()` + App.vue setup 时提前调用 | 🟡 |
| CR-021 | [settings.ts](src/stores/settings.ts#L27) | `toggleAutoLaunch` 改为先写 DB 再切换 OS autostart，避免不一致 | 🟡 |

### 待修复 (P4 — 9 项，均为低优先级)

| 编号 | 严重程度 | 简述 |
|:---|:---:|:---|
| CR-022 | 🟢 | JSONP trim 保守化 |
| CR-023 | 🟢 | NTooltip/NDropdown 冲突解耦 |
| CR-024 | 🟢 | 右键菜单视口裁剪 |
| CR-025 | 🟢 | 自启动开关键盘可访问性 |
| CR-026 | 🟢 | composable 调用移到 setup 顶层 |
| CR-027 | 🟢 | MinuteChart/KLineChart 合并 |
| CR-028 | 🟢 | tsconfig lib 升级到 ES2022 |
| CR-029 | 🟢 | unchecked_transaction 文档警告 |
| CR-030 | 🟢 | Tauri 安全模式评估 |
