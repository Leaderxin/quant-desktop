# K线图懒加载历史数据 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用户在K线图上向左拖动到数据起点时自动静默加载更早的历史数据，同时自动刷新改为增量更新末尾，不清除已加载历史。

**Architecture:** 后端 Rust trait + 适配器增加 `end_date`/`count` 分页参数；前端 `useChart.ts` 重写 `DataLoader` 处理 `forward` 懒加载回调，`allData` ref 累积全部已加载数据，自动刷新改为增量合并模式。

**Tech Stack:** Rust (tauri v2, reqwest, serde), TypeScript (Vue 3, klinecharts v10.0.0-beta3)

## Global Constraints

- 不影响分时图（minute chart）现有行为
- 腾讯 API：分页参数 `end_date`/`count` 通过 URL `param={code},{period},,{end_date},{count},qfq` 传递
- 新浪 API：不支持分页，`datalen` 改为 600，`end_date`/`count` 参数接受但不使用
- klinecharts 库：`forward` = 向左拖动（更早数据），图表自动 prepend；`init` = 替换全部数据
- 前端 Tauri invoke 类型为 `KLineData[]`（from `src/types/index.ts`）
- 懒加载每批 100 条，初始加载默认 200 条

---

### Task 1: 更新 DataSource trait — `fetch_kline` 增加 `end_date`/`count` 参数

**Files:**
- Modify: `src-tauri/src/datasource/mod.rs` — lines 112-120

**Interfaces:**
- Produces: `fetch_kline(&self, code, market, period, end_date: Option<&str>, count: Option<u32>) -> Result<Vec<KLineData>, AppError>`

- [ ] **Step 1: 修改 trait 签名**

Replace lines 112-120 in [mod.rs](src-tauri/src/datasource/mod.rs):

```rust
    /// Fetch K-line data for charting (daily/weekly/monthly)
    /// - `end_date`: None → latest data; Some("YYYY-MM-DD") → data up to and including this date
    /// - `count`: None → default 200; Some(n) → return at most n bars
    async fn fetch_kline(
        &self,
        _code: &str,
        _market: &str,
        _period: &str,
        _end_date: Option<&str>,
        _count: Option<u32>,
    ) -> Result<Vec<crate::domain::KLineData>, AppError> {
        Ok(vec![])
    }
```

- [ ] **Step 2: 编译验证**

```bash
cd src-tauri && cargo check 2>&1 | head -20
```

Expected: 编译错误（适配器尚未实现新签名），不要紧，Task 2-3 会修复。

---

### Task 2: 腾讯适配器 — 实现 `end_date`/`count` 分页

**Files:**
- Modify: `src-tauri/src/datasource/tencent.rs` — lines 298-382 (`fetch_kline` 方法)

**Interfaces:**
- Consumes: trait `fetch_kline` 新签名 (Task 1)
- Produces: `Vec<KLineData>` — 按日期升序排列的 K 线数据

- [ ] **Step 1: 修改 `fetch_kline` 方法签名和 URL 构建**

Replace lines 298-320 in [tencent.rs](src-tauri/src/datasource/tencent.rs):

```rust
    async fn fetch_kline(
        &self,
        code: &str,
        market: &str,
        period: &str,
        end_date: Option<&str>,
        count: Option<u32>,
    ) -> Result<Vec<crate::domain::KLineData>, AppError> {
        let tc_code = if code.starts_with("s_") {
            code[2..].to_string()
        } else {
            Self::code_to_tencent(code, market)
        };

        // Map period to Tencent API parameter
        let period_param = match period {
            "weekly" => "week",
            "monthly" => "month",
            _ => "day",
        };

        let cnt = count.unwrap_or(200);
        let end_date_str = end_date.unwrap_or("");

        let url = format!(
            "http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={},{},,{},{},qfq",
            tc_code, period_param, end_date_str, cnt
        );
```

The rest of the method (lines 322-381) remains unchanged — same JSON parsing, same volume normalization.

- [ ] **Step 2: 编译验证**

```bash
cd src-tauri && cargo check 2>&1
```

Expected: 如果 Task 1 已完成，腾讯适配器编译通过；新浪适配器仍有编译错误。

---

### Task 3: 新浪适配器 — `datalen` 改为 600，接受新参数

**Files:**
- Modify: `src-tauri/src/datasource/sina.rs` — lines 378-399 (`fetch_kline` 方法)

**Interfaces:**
- Consumes: trait `fetch_kline` 新签名 (Task 1)
- Produces: `Vec<KLineData>` — 最多 600 条日K数据

- [ ] **Step 1: 修改 `fetch_kline` 方法签名和 datalen**

Replace lines 378-400 in [sina.rs](src-tauri/src/datasource/sina.rs):

```rust
    async fn fetch_kline(
        &self,
        code: &str,
        market: &str,
        period: &str,
        end_date: Option<&str>,
        count: Option<u32>,
    ) -> Result<Vec<crate::domain::KLineData>, AppError> {
        let symbol = if code.starts_with("s_") {
            code[2..].to_string()
        } else {
            Self::code_to_sina(code, market)
        };

        // Sina only supports daily K-line; reject minute/weekly/monthly.
        // Minute data should use fetch_minute_data instead.
        if period != "daily" {
            return Err(AppError::Unsupported("新浪数据源不支持周K/月K/分钟K线，请切换到腾讯数据源查看".into()));
        }

        if end_date.is_some() || count.is_some() {
            log::warn!("Sina adapter does not support end_date/count pagination; ignoring");
        }

        let scale = "240";

        let url = format!(
            "http://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData?symbol={}&scale={}&ma=no&datalen=600",
            symbol, scale
        );
```

The rest of the method (lines 401-462) remains unchanged — same JSON parsing.

- [ ] **Step 2: 编译验证**

```bash
cd src-tauri && cargo check 2>&1
```

Expected: 所有适配器编译通过（无错误）。

---

### Task 4: 更新 `get_kline` 命令 — 透传 `end_date`/`count`

**Files:**
- Modify: `src-tauri/src/commands/quote.rs` — lines 39-49

**Interfaces:**
- Consumes: trait `fetch_kline` 新签名 (Task 1)
- Produces: Tauri command `get_kline(code, market, period, end_date?, count?)`

- [ ] **Step 1: 修改命令签名**

Replace lines 39-49 in [quote.rs](src-tauri/src/commands/quote.rs):

```rust
#[tauri::command]
pub async fn get_kline(
    code: String,
    market: String,
    period: String,
    end_date: Option<String>,
    count: Option<u32>,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Vec<KLineData>, String> {
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_kline(&code, &market, &period, end_date.as_deref(), count).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 全量编译验证**

```bash
cd src-tauri && cargo check 2>&1
```

Expected: 全量编译通过，无错误。

- [ ] **Step 3: 验证前端类型检查**

```bash
npx vue-tsc --noEmit 2>&1 | head -30
```

Expected: 可能报 `get_kline` 调用缺少新参数 — 这是预期的，Task 5 会修复。确认没有其他错误即可。

---

### Task 5: 前端 useChart.ts — 重写 DataLoader + 增量刷新

**Files:**
- Modify: `src/composables/useChart.ts`

**Interfaces:**
- Consumes: `get_kline` 命令新签名 (Task 4), `KLineData` from `src/types/index.ts`, klinecharts `DataLoader`/`Chart` types
- Produces: 对外接口不变 (`chart`, `loading`, `error`, `klineData`, `initChart`, `loadData`, `disposeChart`, `applyTheme`)

- [ ] **Step 1: 添加辅助函数和新增状态变量**

在 `useChart.ts` 的 line 22（`const currentPeriod` 之后）插入 `allData` 和 `hasMoreForward`，并添加 `formatDate` 辅助函数到 line 23-37 之间：

```typescript
  // 累积全部已加载的 K 线数据（初始 + 历次懒加载），按时间升序
  const allData = ref<KCLineData[]>([]);
  // 标记是否还有更多历史数据可加载
  const hasMoreForward = ref(true);

  /** 将时间戳格式化为 YYYY-MM-DD 字符串，用于 API end_date 参数 */
  function formatDate(ts: number): string {
    const d = new Date(ts);
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  }

  /** 将后端 KLineData 映射为 klinecharts 需要的 KLineData 格式 */
  function mapKLineToChart(data: KLineData[]): KCLineData[] {
    return data.map((d) => {
      const ts = new Date(d.date).getTime();
      return {
        timestamp: isNaN(ts) ? 0 : ts,
        open: d.open,
        high: d.high,
        low: d.low,
        close: d.close,
        volume: d.volume,
      };
    });
  }
```

- [ ] **Step 2: 重写 DataLoader**

替换 lines 26-35 的 `dataLoader` 定义：

```typescript
  const dataLoader: DataLoader = {
    getBars: async (params) => {
      if (params.type === 'init') {
        // 首次加载/切换股票：返回全部已加载数据
        params.callback(allData.value, {
          forward: hasMoreForward.value,
          backward: false,
        });
      } else if (params.type === 'forward') {
        // 用户向左拖动到边界 → 加载更早的历史数据
        if (!hasMoreForward.value) {
          params.callback([], { forward: false, backward: false });
          return;
        }
        loading.value = true;
        try {
          const endDate =
            params.timestamp != null
              ? formatDate(params.timestamp - 86400000)
              : undefined;
          const data = await invoke<KLineData[]>('get_kline', {
            code: unref(options.code),
            market: unref(options.market),
            period: currentPeriod.value,
            endDate,
            count: 100,
          });
          const newBars = mapKLineToChart(data);
          if (newBars.length > 0) {
            // 去重后拼接到头部
            const existing = new Set(allData.value.map((d) => d.timestamp));
            const unique = newBars.filter((d) => !existing.has(d.timestamp));
            if (unique.length > 0) {
              allData.value = [
                ...unique.sort((a, b) => a.timestamp - b.timestamp),
                ...allData.value,
              ];
            }
          }
          hasMoreForward.value = newBars.length >= 100;
          params.callback(newBars, {
            forward: hasMoreForward.value,
            backward: false,
          });
        } catch (e) {
          console.error('[useChart] forward load failed:', e);
          // 加载失败时保持 forward: true，用户再次拖动会重试
          params.callback([], { forward: true, backward: false });
        } finally {
          loading.value = false;
        }
      } else {
        // backward / update: 不需要处理
        params.callback([], {
          forward: hasMoreForward.value,
          backward: false,
        });
      }
    },
  };
```

- [ ] **Step 3: 重写 K 线数据加载逻辑 (`loadData` 中 `else` 分支)**

替换 lines 283-303（K 线数据加载的 else 分支）：

```typescript
      } else {
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
        });
        if (signal.aborted) return;

        if (data.length) {
          const mapped = mapKLineToChart(data);
          // 存入 allData（按时间升序）
          allData.value = mapped.sort((a, b) => a.timestamp - b.timestamp);
          // 新浪日K 600 条一次性加载到底，腾讯支持分页懒加载
          hasMoreForward.value = settings.activeDatasource !== 'sina';
          // 保持 klineData 兼容性（对外暴露）
          klineData.value = mapped;
        }
      }
```

- [ ] **Step 4: 重写自动刷新为增量模式**

替换 `startAutoRefresh` 函数（lines 228-243）中的定时器回调逻辑：

```typescript
  function startAutoRefresh(period: PeriodType) {
    stopAutoRefresh();
    const interval = getRefreshInterval(period);
    refreshTimer = setInterval(async () => {
      if (loading.value) return;
      try {
        // 增量刷新：只取最新 10 条，更新末尾
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
          count: 10,
        });
        const newBars = mapKLineToChart(data);
        if (newBars.length > 0) {
          // 合并到 allData：按 timestamp 去重、更新
          const map = new Map(allData.value.map((d) => [d.timestamp, d]));
          for (const bar of newBars) {
            map.set(bar.timestamp, bar);
          }
          allData.value = [...map.values()].sort((a, b) => a.timestamp - b.timestamp);
          // 触发图表增量更新（init 会返回完整 allData）
          if (chart.value) {
            chart.value.setDataLoader(dataLoader);
          }
        }
      } catch (e) {
        // 静默失败，不影响用户浏览
        console.error('[useChart] incremental update failed:', e);
      }
    }, interval);
  }
```

- [ ] **Step 5: 重置状态 — 切换股票/周期时清理 allData**

在 `loadData` 函数开头（line 246 之后）添加状态重置：

```typescript
  async function loadData(period: PeriodType) {
    if (abortController) {
      abortController.abort();
    }
    abortController = new AbortController();
    const { signal } = abortController;

    loading.value = true;
    error.value = '';

    // 重置累积数据和分页状态（切换股票/周期时重新开始）
    allData.value = [];
    hasMoreForward.value = true;

    try {
```

即在 `loading.value = true;` 和 `error.value = '';` 之后、`try {` 之前插入 3 行。

- [ ] **Step 6: 类型检查 + 构建验证**

```bash
npx vue-tsc --noEmit 2>&1
```

Expected: 无类型错误。

```bash
cd src-tauri && cargo check 2>&1
```

Expected: 编译通过。

- [ ] **Step 7: 手动验证**

启动应用，打开任意股票的日K图：

```bash
npm run tauri dev
```

1. **初始加载：** 日K图正常显示 200 条 K 线
2. **懒加载：** 在 K 线图上向左拖动，到达边界时应自动加载更早数据
3. **增量刷新：** 等待 30 秒，已加载的历史数据不会被清除
4. **周K/月K：** 切换到周K或月K，同样支持懒加载
5. **切换股票：** 切换后数据正确重置

---

## 实施顺序

```
Task 1 (trait) → Task 2 (tencent) → Task 3 (sina) → Task 4 (command) → Task 5 (frontend)
```

Task 1 是接口定义，必须先完成。Task 2-3 是适配器实现，互相独立可并行。Task 4 依赖 Task 1-3 全部完成。Task 5 依赖 Task 4 完成后的命令签名。
