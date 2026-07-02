# K线图懒加载历史数据 — 设计文档

**日期：** 2026-07-02
**状态：** 已确认

---

## 1. 目标

用户在 K 线图（日K/周K/月K）上向左拖动到当前数据起点时，自动静默加载更早的历史数据，实现"无限向左滚动"体验。

---

## 2. 需求

| ID | 需求 | 说明 |
|----|------|------|
| R1 | 自动触发 | 用户向左拖动到数据边界时自动加载，无需任何手动操作 |
| R2 | 增量历史 | 每批加载 100 条，数据拼接到已有数据头部，无深度上限 |
| R3 | 增量刷新 | 定时刷新（30s/60s）只更新末尾最新 K 线，不清除已加载的历史数据 |
| R4 | 双源适配 | 腾讯 API 支持分页懒加载；新浪 API 不支持分页，改为一次性加载 600 条 |

---

## 3. 涉及文件

| 文件 | 改动类型 |
|------|----------|
| `src/composables/useChart.ts` | 重写 DataLoader、增量刷新、allData 累积 |
| `src-tauri/src/commands/quote.rs` | `get_kline` 增加 `end_date`、`count` 可选参数 |
| `src-tauri/src/datasource/mod.rs` | trait `fetch_kline` 增加 `end_date`、`count` 参数 |
| `src-tauri/src/datasource/tencent.rs` | 实现 `end_date` + `count` 分页 |
| `src-tauri/src/datasource/sina.rs` | `datalen` 改为 600 |

---

## 4. 数据流

```
用户向左拖动 → 到达数据边界
  → klinecharts 触发 getBars({ type: 'forward', timestamp: 最早K线时间戳 })
    → useChart DataLoader: 取 allData 中最早日期的前一天作为 end_date
      → invoke('get_kline', { code, market, period, endDate, count: 100 })
        → Rust get_kline 命令 → 适配器 fetch_kline(code, market, period, Some(end_date), Some(100))
          → 腾讯 API: param={code},{period},,{end_date},100,qfq
          → 返回更早的 100 条 K 线
    ← 拼接到 allData 头部
    ← callback(bars, { forward: <返回数<100>, backward: false })
```

自动刷新（增量）：

```
定时器触发 (30s/60s)
  → invoke('get_kline', { code, market, period })  // 无 end_date，默认最新
  → 取返回的最新 ~5 条
    → 与 allData 末尾对比去重
    → chart.applyNewData(newBars)  // 只更新末尾
```

---

## 5. 详细设计

### 5.1 前端 useChart.ts

#### 5.1.1 数据结构

```ts
// 累积全部已加载的 K 线数据（初始 + 历次懒加载 + 增量刷新），按时间升序
const allData = ref<KCLineData[]>([]);

// 标记是否还有更多历史数据可加载
const hasMoreForward = ref(true);
```

#### 5.1.2 DataLoader 重写

```ts
const dataLoader: DataLoader = {
  getBars: async (params) => {
    if (params.type === 'init') {
      // 首次加载/切换股票：返回已加载的全部数据
      params.callback(allData.value, { forward: hasMoreForward.value, backward: false });
    } else if (params.type === 'forward') {
      // 用户向左拖动，需要更早的历史数据
      if (!hasMoreForward.value) {
        params.callback([], { forward: false, backward: false });
        return;
      }
      loading.value = true;
      try {
        const earliest = allData.value[0];
        const endDate = earliest
          ? formatDate(new Date(earliest.timestamp - 86400000)) // 最早日期的前一天
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
          const existing = new Set(allData.value.map(d => d.timestamp));
          const unique = newBars.filter(d => !existing.has(d.timestamp));
          allData.value = [...unique, ...allData.value];
        }
        hasMoreForward.value = newBars.length >= 100;
        params.callback(newBars, { forward: hasMoreForward.value, backward: false });
      } catch (e) {
        console.error('[useChart] forward load failed:', e);
        params.callback([], { forward: true, backward: false });
      } finally {
        loading.value = false;
      }
    } else if (params.type === 'update') {
      // 定时刷新：只取最新数据
      // 由外部 setInterval 驱动，此处可做轻量增量更新
      params.callback([], { forward: hasMoreForward.value, backward: false });
    } else {
      params.callback([], { forward: hasMoreForward.value, backward: false });
    }
  },
};
```

#### 5.1.3 自动刷新改为增量模式

```ts
async function loadIncrementalUpdate() {
  try {
    const data = await invoke<KLineData[]>('get_kline', {
      code: unref(options.code),
      market: unref(options.market),
      period: currentPeriod.value,
      count: 10, // 只取最新 10 条用于增量更新
    });
    const newBars = mapKLineToChart(data);
    // 从末尾匹配，替换/追加有变化的 bar
    if (chart.value && newBars.length > 0) {
      chart.value.applyNewData(newBars);
      // 更新 allData 末尾
      const existing = new Map(allData.value.map(d => [d.timestamp, d]));
      for (const bar of newBars) {
        existing.set(bar.timestamp, bar);
      }
      allData.value = [...existing.values()].sort((a, b) => a.timestamp - b.timestamp);
    }
  } catch (e) {
    // 静默失败，不影响用户浏览
    console.error('[useChart] incremental update failed:', e);
  }
}
```

#### 5.1.4 初始加载逻辑调整

`loadData` 中 K 线首次加载仍取 200 条（默认），但改为写入 `allData` 而非只用一次：

```ts
// 首次加载后
allData.value = mappedChartData;
hasMoreForward.value = true; // 腾讯 API 始终有更多历史
```

### 5.2 Rust 命令层

#### 5.2.1 Tauri 命令 — `get_kline`

```rust
#[tauri::command]
pub async fn get_kline(
    code: String,
    market: String,
    period: String,
    end_date: Option<String>,   // 新增：截止日期，取此日期及之前的数据
    count: Option<u32>,          // 新增：返回条数
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Vec<KLineData>, String> {
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_kline(
        &code,
        &market,
        &period,
        end_date.as_deref(),
        count,
    ).await.map_err(|e| e.to_string())
}
```

### 5.3 数据源 trait — `fetch_kline`

```rust
async fn fetch_kline(
    &self,
    _code: &str,
    _market: &str,
    _period: &str,
    _end_date: Option<&str>,  // 新增
    _count: Option<u32>,       // 新增
) -> Result<Vec<crate::domain::KLineData>, AppError> {
    Ok(vec![])
}
```

- `end_date`: `None` → 取最新数据；`Some(date)` → 取该日期及之前的数据
- `count`: `None` → 默认 200（初始加载）；`Some(n)` → 指定条数

### 5.4 腾讯适配器

#### URL 格式

```
初始加载（无 end_date）:
http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={code},{period},,,200,qfq

懒加载（带 end_date）:
http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={code},{period},,{end_date},{count},qfq
```

腾讯 K-line API 参数位置：
```
param={code},{period},,{end_date},{count},{adjust}
```

- 位置 3：留空
- 位置 4：end_date（空 = 最新）
- 位置 5：count
- 位置 6：qfq（前复权）

#### 实现

```rust
async fn fetch_kline(
    &self,
    code: &str,
    market: &str,
    period: &str,
    end_date: Option<&str>,
    count: Option<u32>,
) -> Result<Vec<KLineData>, AppError> {
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
    // ... 其余解析逻辑不变
}
```

### 5.5 新浪适配器

- `datalen` 从 200 改为 600
- `end_date`/`count` 参数接受但不使用（记录 warn 日志）
- 周K/月K 仍返回 `Unsupported` 错误

```rust
async fn fetch_kline(
    &self,
    code: &str,
    market: &str,
    period: &str,
    end_date: Option<&str>,
    count: Option<u32>,
) -> Result<Vec<KLineData>, AppError> {
    if end_date.is_some() || count.is_some() {
        log::warn!("Sina adapter does not support pagination parameters");
    }
    // datalen 改为 600
    let url = format!(
        "http://money.finance.sina.com.cn/...&datalen=600",
        ...
    );
    // ...
}
```

---

## 6. 边界情况

| 场景 | 处理 |
|------|------|
| 懒加载时网络错误 | `params.callback([], { forward: true })` —— 告诉图表"还有更多"但暂时加载失败，用户再次拖动会重试 |
| 腾讯 API 返回空 | `hasMoreForward = false`，图表停止请求 |
| 数据去重 | 懒加载返回的 K 线可能与已有数据边界重叠，按 `timestamp` 去重 |
| 切换股票/周期 | `allData` 和 `hasMoreForward` 重置，DataLoader 重新创建 |
| 新浪适配器 | 首次 600 条一次性返回，`hasMoreForward = false` |
| API 返回数据倒序 | 腾讯 API 返回数据按日期降序（最新在前），需要反转后合并 |
| 交易日边界 | `end_date` 为节假日时 API 自动取该日期之前最近的交易日 |

---

## 7. 自审清单

- [x] 无 TBD/TODO 占位符
- [x] 前后端参数一致
- [x] 边界情况已覆盖
- [x] 不影响分时图（minute chart）
- [x] 涉及文件完整列出
