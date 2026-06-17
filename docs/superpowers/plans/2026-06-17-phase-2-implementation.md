# Phase 2 — 完善体验 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the 5 remaining Phase 2 features: stock detail panel (minute chart + depth + summary), Tencent backup datasource, sort/filter in watchlist table, window position/size memory, and trading-hours-aware polling frequency.

**Architecture:** Rust trait extension (fetch_depth + fetch_minute_data added to DataSource) → Eastmoney/Sina adapters implement them → new Tauri commands expose them → Vue components consume them via invoke. Frontend-only changes for sort/filter (NDataTable sorter). Window events → settings persistence for position memory. New market_clock module → Scheduler dynamic interval adjustment.

**Tech Stack:** Rust (async_trait, reqwest, chrono, tokio), Vue 3 (Naive UI, klinecharts), TypeScript

---

## File Structure Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/datasource/mod.rs` | Modify | Add `fetch_depth`, `fetch_minute_data` to DataSource trait |
| `src-tauri/src/datasource/eastmoney.rs` | Modify | Implement depth + minute data fetching |
| `src-tauri/src/datasource/sina.rs` | Modify | Implement depth fetching (stub minute data) |
| `src-tauri/src/datasource/tencent.rs` | Create | Tencent adapter (all DataSource trait methods) |
| `src-tauri/src/datasource/market_clock.rs` | Create | Trading hours detection + interval recommendation |
| `src-tauri/src/cache/mod.rs` | Modify | Integrate market_clock into Scheduler for dynamic interval |
| `src-tauri/src/commands/quote.rs` | Modify | Add `get_depth`, `get_intraday` commands |
| `src-tauri/src/commands/window.rs` | Modify | Add `save_window_state` command |
| `src-tauri/src/lib.rs` | Modify | Register new commands, Tencent adapter, window position save/restore |
| `src/types/index.ts` | Modify | Add Depth, Level, MinuteData interfaces |
| `src/components/detail/StockDetail.vue` | Create | Detail panel container (expandable below row) |
| `src/components/detail/MinuteChart.vue` | Create | Intraday minute chart using klinecharts |
| `src/components/detail/DepthPanel.vue` | Create | Five-level bid/ask display |
| `src/components/detail/StockSummary.vue` | Create | Key metrics summary (open/high/low/volume/turnover) |
| `src/components/watchlist/WatchlistTable.vue` | Modify | Add sorter to columns, add filter toggle, integrate row click → detail |

---

### Task 1: Extend DataSource trait with depth + minute data

**Files:**
- Modify: `src-tauri/src/datasource/mod.rs`

- [ ] **Step 1: Add fetch_depth and fetch_minute_data methods to DataSource trait**

Add two new methods to the `DataSource` trait in `src-tauri/src/datasource/mod.rs`, after the `search` method and before `health_check`:

```rust
// src-tauri/src/datasource/mod.rs — inside #[async_trait] pub trait DataSource

    /// Fetch 5-level depth (bid/ask order book)
    async fn fetch_depth(
        &self,
        _code: &str,
        _market: &str,
    ) -> Result<crate::domain::Depth, String> {
        // Default: return empty depth (adapters that don't support it)
        Ok(crate::domain::Depth {
            code: _code.to_string(),
            bids: vec![],
            asks: vec![],
        })
    }

    /// Fetch intraday minute data for charting
    async fn fetch_minute_data(
        &self,
        _code: &str,
        _market: &str,
    ) -> Result<Vec<crate::domain::MinuteData>, String> {
        // Default: return empty (adapters that don't support it)
        Ok(vec![])
    }
```

Note: use `_code` and `_market` prefix in default implementations to suppress unused variable warnings.

- [ ] **Step 2: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build (Sina and Eastmoney adapters will use the default trait implementations)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasource/mod.rs
git commit -m "feat: add fetch_depth and fetch_minute_data to DataSource trait with defaults"
```

---

### Task 2: Implement Eastmoney depth + minute data

**Files:**
- Modify: `src-tauri/src/datasource/eastmoney.rs`

- [ ] **Step 1: Add constants and implement fetch_depth for EastmoneyAdapter**

Add these constants after the existing ones in `eastmoney.rs`:

```rust
const EASTMONEY_DEPTH_URL: &str = "https://push2.eastmoney.com/api/qt/stock/get";
const EASTMONEY_TREND_URL: &str = "https://push2.eastmoney.com/api/qt/stock/trends2/get";
```

Add the method implementation inside `impl DataSource for EastmoneyAdapter` block, after `search` and before `health_check`:

```rust
    async fn fetch_depth(
        &self,
        code: &str,
        market: &str,
    ) -> Result<crate::domain::Depth, String> {
        let secid = Self::code_to_secid(code, market);
        let params = [
            ("secid", &secid),
            ("fields", "f43,f44,f45,f46,f47,f48,f55,f56,f57,f58"),
        ];

        #[derive(Deserialize)]
        struct RawResponse {
            data: Option<RawDepth>,
        }
        #[derive(Deserialize)]
        struct RawDepth {
            #[serde(rename = "f43")]
            price: Option<f64>,
            #[serde(rename = "f44")]
            bid1_p: Option<f64>,
            #[serde(rename = "f45")]
            bid1_v: Option<u64>,
            #[serde(rename = "f46")]
            bid2_p: Option<f64>,
            #[serde(rename = "f47")]
            bid2_v: Option<u64>,
            #[serde(rename = "f48")]
            bid3_p: Option<f64>,
            #[serde(rename = "f55")]
            ask1_p: Option<f64>,
            #[serde(rename = "f56")]
            ask1_v: Option<u64>,
            #[serde(rename = "f57")]
            ask2_p: Option<f64>,
            #[serde(rename = "f58")]
            ask2_v: Option<u64>,
        }

        let resp = self
            .client
            .get(EASTMONEY_DEPTH_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Depth request failed: {:#}", e))?;

        let body: RawResponse = resp.json().await.map_err(|e| format!("Parse depth failed: {}", e))?;

        match body.data {
            Some(d) => {
                let mut bids = Vec::new();
                let mut asks = Vec::new();
                if let (Some(p), Some(v)) = (d.bid1_p, d.bid1_v) { bids.push(crate::domain::Level { price: p, volume: v }); }
                if let (Some(p), Some(v)) = (d.bid2_p, d.bid2_v) { bids.push(crate::domain::Level { price: p, volume: v }); }
                if let (Some(p), Some(v)) = (d.bid3_p, d.bid3_v) { bids.push(crate::domain::Level { price: p, volume: v }); }
                if let (Some(p), Some(v)) = (d.ask1_p, d.ask1_v) { asks.push(crate::domain::Level { price: p, volume: v }); }
                if let (Some(p), Some(v)) = (d.ask2_p, d.ask2_v) { asks.push(crate::domain::Level { price: p, volume: v }); }
                Ok(crate::domain::Depth { code: code.to_string(), bids, asks })
            }
            None => Ok(crate::domain::Depth { code: code.to_string(), bids: vec![], asks: vec![] }),
        }
    }

    async fn fetch_minute_data(
        &self,
        code: &str,
        market: &str,
    ) -> Result<Vec<crate::domain::MinuteData>, String> {
        let secid = Self::code_to_secid(code, market);
        let params = [
            ("secid", &secid),
            ("fields1", "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11"),
            ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58"),
            ("ndays", "1"),
        ];

        #[derive(Deserialize)]
        struct RawResponse {
            data: Option<RawTrend>,
        }
        #[derive(Deserialize)]
        struct RawTrend {
            trends: Option<Vec<String>>,
        }

        let resp = self
            .client
            .get(EASTMONEY_TREND_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Trend request failed: {:#}", e))?;

        let body: RawResponse = resp.json().await.map_err(|e| format!("Parse trend failed: {}", e))?;

        let trends = body
            .data
            .and_then(|d| d.trends)
            .unwrap_or_default();

        let data: Vec<crate::domain::MinuteData> = trends
            .iter()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 8 {
                    Some(crate::domain::MinuteData {
                        time: parts[0].to_string(),
                        price: parts[2].parse().unwrap_or(0.0),
                        volume: parts[5].parse().unwrap_or(0),
                        avg_price: parts[7].parse().unwrap_or(0.0),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(data)
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasource/eastmoney.rs
git commit -m "feat: implement fetch_depth and fetch_minute_data for Eastmoney adapter"
```

---

### Task 3: Implement Sina depth (stub minute data)

**Files:**
- Modify: `src-tauri/src/datasource/sina.rs`

- [ ] **Step 1: Implement fetch_depth for SinaAdapter**

Sina provides depth via `http://hq.sinajs.cn/list=buy_sh600519,sell_sh600519`. Add this inside the `impl DataSource for SinaAdapter` block after `search`:

```rust
    async fn fetch_depth(
        &self,
        code: &str,
        market: &str,
    ) -> Result<crate::domain::Depth, String> {
        let sina_code = Self::code_to_sina(code, market);
        // Sina depth URLs: buy_xxx for bids, sell_xxx for asks
        let url = format!("{}{},{}",
            SINA_URL,
            format!("buy_{}", sina_code),
            format!("sell_{}", sina_code),
        );

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina depth request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Sina read failed: {:#}", e))?;
        let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
            .map_err(|e| format!("Sina GBK decode failed: {}", e))?;

        let mut bids = Vec::new();
        let mut asks = Vec::new();

        // Parse buy/sell lines
        for line in body.lines() {
            if let Some(eq_pos) = line.find('=') {
                let var_part = &line[..eq_pos];
                let quote_start = line[eq_pos + 1..].find('"').map(|p| eq_pos + 1 + p + 1);
                if let Some(start) = quote_start {
                    let quote_end = line[start..].find('"').unwrap_or(0);
                    let data = &line[start..start + quote_end];
                    let fields: Vec<&str> = data.split(',').collect();
                    if var_part.contains("buy_") && fields.len() >= 2 {
                        let price = fields[0].parse::<f64>().unwrap_or(0.0);
                        let volume = fields[1].parse::<u64>().unwrap_or(0);
                        if price > 0.0 {
                            bids.push(crate::domain::Level { price, volume });
                        }
                    } else if var_part.contains("sell_") && fields.len() >= 2 {
                        let price = fields[0].parse::<f64>().unwrap_or(0.0);
                        let volume = fields[1].parse::<u64>().unwrap_or(0);
                        if price > 0.0 {
                            asks.push(crate::domain::Level { price, volume });
                        }
                    }
                }
            }
        }

        // Sort bids descending, asks ascending
        bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal));
        asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));

        Ok(crate::domain::Depth { code: code.to_string(), bids, asks })
    }

    // No fetch_minute_data override — falls through to default trait (empty vec)
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasource/sina.rs
git commit -m "feat: implement fetch_depth for Sina adapter"
```

---

### Task 4: Add Rust commands for depth and intraday data

**Files:**
- Modify: `src-tauri/src/commands/quote.rs`

- [ ] **Step 1: Add get_depth and get_intraday commands**

Replace the entire content of `src-tauri/src/commands/quote.rs`:

```rust
use tauri::State;
use std::sync::Arc;
use crate::cache::QuoteCache;
use crate::datasource::DataSourceManager;
use crate::domain::{Quote, IndexQuote, Depth, MinuteData};

#[tauri::command]
pub fn get_quotes(cache: State<'_, Arc<QuoteCache>>) -> Vec<Quote> {
    cache.get_all_quotes()
}

#[tauri::command]
pub fn get_indices(cache: State<'_, Arc<QuoteCache>>) -> Vec<IndexQuote> {
    cache.get_indices()
}

#[tauri::command]
pub async fn get_depth(
    code: String,
    market: String,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Depth, String> {
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_depth(&code, &market).await
}

#[tauri::command]
pub async fn get_intraday(
    code: String,
    market: String,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Vec<MinuteData>, String> {
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_minute_data(&code, &market).await
}
```

- [ ] **Step 2: Register new commands in lib.rs**

In `src-tauri/src/lib.rs`, add the new commands to the `invoke_handler`:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::quote::get_quotes,
            commands::quote::get_indices,
            commands::quote::get_depth,
            commands::quote::get_intraday,
            commands::watchlist::get_watchlist,
            // ... rest unchanged
        ])
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/quote.rs src-tauri/src/lib.rs
git commit -m "feat: add get_depth and get_intraday Tauri commands"
```

---

### Task 5: Add TypeScript types for Depth and MinuteData

**Files:**
- Modify: `src/types/index.ts`

- [ ] **Step 1: Add Depth, Level, MinuteData interfaces**

After the `StockBrief` interface in `src/types/index.ts`, add:

```typescript
export interface Level {
  price: number;
  volume: number;
}

export interface Depth {
  code: string;
  bids: Level[];
  asks: Level[];
}

export interface MinuteData {
  time: string;
  price: number;
  volume: number;
  avg_price: number;
}
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit 2>&1`
Expected: No new type errors (there may be pre-existing ones)

- [ ] **Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "feat: add Depth, Level, MinuteData TypeScript types"
```

---

### Task 6: Create StockSummary component

**Files:**
- Create: `src/components/detail/StockSummary.vue`

- [ ] **Step 1: Write StockSummary.vue**

```vue
<script setup lang="ts">
import type { Quote } from '@/types';

const props = defineProps<{
  quote: Quote;
}>();

const items = [
  { label: '开盘', value: props.quote.open?.toFixed(2) ?? '--' },
  { label: '最高', value: props.quote.high?.toFixed(2) ?? '--' },
  { label: '最低', value: props.quote.low?.toFixed(2) ?? '--' },
  { label: '成交量', value: (props.quote.volume / 10000).toFixed(0) + '万手' },
  { label: '成交额', value: (props.quote.turnover / 100000000).toFixed(2) + '亿' },
  {
    label: '换手率',
    value: props.quote.turnover_rate != null ? props.quote.turnover_rate.toFixed(2) + '%' : '--'
  },
];
</script>

<template>
  <div class="stock-summary">
    <div class="summary-row" v-for="item in items" :key="item.label">
      <span class="summary-label">{{ item.label }}</span>
      <span class="summary-value">{{ item.value }}</span>
    </div>
  </div>
</template>

<style scoped>
.stock-summary {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  padding: 12px;
  background: var(--color-bg-card, rgba(255,255,255,0.04));
  border-radius: 6px;
}
.summary-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.summary-label {
  font-size: 11px;
  color: var(--color-text-tertiary, #888);
}
.summary-value {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary, #e0e0e0);
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add src/components/detail/StockSummary.vue
git commit -m "feat: add StockSummary component for key metrics display"
```

---

### Task 7: Create DepthPanel component

**Files:**
- Create: `src/components/detail/DepthPanel.vue`

- [ ] **Step 1: Write DepthPanel.vue**

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Depth, Level } from '@/types';

const props = defineProps<{
  code: string;
  market: string;
}>();

const depth = ref<Depth | null>(null);
const loading = ref(false);
const error = ref('');

async function fetchDepth() {
  loading.value = true;
  error.value = '';
  try {
    depth.value = await invoke<Depth>('get_depth', { code: props.code, market: props.market });
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => fetchDepth());

function formatVol(v: number): string {
  if (v >= 10000) return (v / 10000).toFixed(0) + '万';
  return v.toString();
}

// Pad to 5 levels
const paddedBids: (Level | null)[] = depth.value
  ? Array.from({ length: 5 }, (_, i) => depth.value!.bids[i] ?? null)
  : Array.from({ length: 5 }, () => null);
const paddedAsks: (Level | null)[] = depth.value
  ? Array.from({ length: 5 }, (_, i) => depth.value!.asks[i] ?? null)
  : Array.from({ length: 5 }, () => null);
</script>

<template>
  <div class="depth-panel">
    <div class="depth-title">五档盘口</div>
    <div v-if="loading" class="depth-loading">加载中...</div>
    <div v-else-if="error" class="depth-error">{{ error }}</div>
    <div v-else class="depth-body">
      <!-- Bids (buy side) -->
      <div class="depth-half bids">
        <div class="depth-header-row">
          <span>买价</span><span>买量</span>
        </div>
        <div
          v-for="(level, i) in paddedBids"
          :key="'b' + i"
          class="depth-row"
          :class="{ 'depth-empty': !level }"
        >
          <span class="depth-price bid-price">{{ level?.price?.toFixed(2) ?? '--' }}</span>
          <span class="depth-vol">{{ level ? formatVol(level.volume) : '--' }}</span>
        </div>
      </div>
      <!-- Asks (sell side) -->
      <div class="depth-half asks">
        <div class="depth-header-row">
          <span>卖价</span><span>卖量</span>
        </div>
        <div
          v-for="(level, i) in paddedAsks"
          :key="'a' + i"
          class="depth-row"
          :class="{ 'depth-empty': !level }"
        >
          <span class="depth-price ask-price">{{ level?.price?.toFixed(2) ?? '--' }}</span>
          <span class="depth-vol">{{ level ? formatVol(level.volume) : '--' }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.depth-panel {
  min-width: 260px;
  padding: 8px;
}
.depth-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--color-text-primary, #e0e0e0);
}
.depth-loading, .depth-error {
  font-size: 12px;
  color: var(--color-text-tertiary, #888);
}
.depth-body {
  display: flex;
  gap: 12px;
}
.depth-half {
  flex: 1;
}
.depth-header-row {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--color-text-tertiary, #888);
  padding: 2px 0;
  border-bottom: 1px solid var(--color-border, rgba(255,255,255,0.08));
  margin-bottom: 4px;
}
.depth-row {
  display: flex;
  justify-content: space-between;
  padding: 3px 0;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.depth-empty { opacity: 0.25; }
.bid-price { color: #ef5350; }
.ask-price { color: #66bb6a; }
.depth-vol { color: var(--color-text-secondary, #ccc); }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add src/components/detail/DepthPanel.vue
git commit -m "feat: add DepthPanel component for 5-level bid/ask display"
```

---

### Task 8: Create MinuteChart component

**Files:**
- Create: `src/components/detail/MinuteChart.vue`

- [ ] **Step 1: Write MinuteChart.vue**

```vue
<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { init, dispose } from 'klinecharts';
import type { MinuteData } from '@/types';

const props = defineProps<{
  code: string;
  market: string;
}>();

const chartRef = ref<HTMLElement | null>(null);
let chart: ReturnType<typeof init> | null = null;
const loading = ref(false);

async function loadData() {
  loading.value = true;
  try {
    const data = await invoke<MinuteData[]>('get_intraday', { code: props.code, market: props.market });
    if (!chart || !data.length) return;

    // The new klinecharts API expects applyNewData with typed array
    const klineData = data.map(d => ({
      timestamp: Date.now(),
      open: d.price,
      high: d.price,
      low: d.price,
      close: d.price,
      volume: d.volume,
    }));

    chart.applyNewData(klineData);
  } catch (e) {
    console.error('Failed to load intraday data:', e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  if (chartRef.value) {
    chart = init(chartRef.value);
    chart.setStyles({
      grid: { horizontal: { color: 'rgba(255,255,255,0.05)' }, vertical: { color: 'rgba(255,255,255,0.05)' } },
      candle: { bar: { upColor: '#ef5350', downColor: '#66bb6a' } },
    });
    loadData();
  }
});

onUnmounted(() => {
  if (chart) { dispose(chartRef.value!); chart = null; }
});

watch(() => props.code, () => { if (chart) loadData(); });
</script>

<template>
  <div class="minute-chart">
    <div v-if="loading" class="chart-loading">加载分时图...</div>
    <div ref="chartRef" class="chart-container"></div>
  </div>
</template>

<style scoped>
.minute-chart {
  flex: 1;
  min-height: 300px;
  position: relative;
}
.chart-container {
  width: 100%;
  height: 100%;
}
.chart-loading {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 12px;
  color: var(--color-text-tertiary, #888);
}
</style>
```

> **Note on klinecharts API:** The klinecharts v10 API may differ from what's shown above. The actual API will be verified against the installed `klinecharts@^10.0.0-beta3` package. Key methods: `init(container)`, `applyNewData(data)`, `setStyles(styles)`, `dispose(container)`. The data format may need adjustment — klinecharts typically expects `{ timestamp, open, high, low, close, volume }[]` for candles, but for minute data we may use the formatted data directly or use a different chart type.

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit 2>&1`
Expected: No new type errors

- [ ] **Step 3: Commit**

```bash
git add src/components/detail/MinuteChart.vue
git commit -m "feat: add MinuteChart component using klinecharts for intraday data"
```

---

### Task 9: Create StockDetail container component

**Files:**
- Create: `src/components/detail/StockDetail.vue`

- [ ] **Step 1: Write StockDetail.vue**

```vue
<script setup lang="ts">
import { refresh } from '@tauri-apps/api/core';
import type { WatchItem } from '@/types';
import { useQuoteStore } from '@/stores/quote';
import StockSummary from './StockSummary.vue';
import DepthPanel from './DepthPanel.vue';
import MinuteChart from './MinuteChart.vue';

const props = defineProps<{
  item: WatchItem;
}>();

const emit = defineEmits<{
  close: [];
}>();

const quoteStore = useQuoteStore();
</script>

<template>
  <div class="stock-detail">
    <div class="detail-header">
      <div class="detail-title">
        <span class="detail-name">{{ item.name }}</span>
        <span class="detail-code">{{ item.code }}</span>
      </div>
      <button class="detail-close" @click="emit('close')">&times;</button>
    </div>

    <div class="detail-content">
      <div class="detail-left">
        <StockSummary :quote="quoteStore.getQuote(item.code, item.market)!" />
        <DepthPanel :code="item.code" :market="item.market" />
      </div>
      <div class="detail-right">
        <MinuteChart :code="item.code" :market="item.market" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.stock-detail {
  border-top: 1px solid var(--color-border, rgba(255,255,255,0.08));
  background: var(--color-bg-elevated, rgba(255,255,255,0.02));
  padding: 12px 16px;
}
.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.detail-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.detail-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-primary, #e0e0e0);
}
.detail-code {
  font-size: 12px;
  color: var(--color-text-tertiary, #888);
}
.detail-close {
  background: none;
  border: none;
  color: var(--color-text-tertiary, #888);
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}
.detail-close:hover { color: var(--color-text-primary, #e0e0e0); }
.detail-content {
  display: flex;
  gap: 16px;
}
.detail-left {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex-shrink: 0;
}
.detail-right {
  flex: 1;
  min-width: 0;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add src/components/detail/StockDetail.vue
git commit -m "feat: add StockDetail container with summary, depth, and minute chart"
```

---

### Task 10: Integrate StockDetail into WatchlistTable

**Files:**
- Modify: `src/components/watchlist/WatchlistTable.vue`

- [ ] **Step 1: Add row click to expand detail and sorter to columns**

First, add the import and ref for selected row:

```typescript
// Add near other refs at top of <script setup>
import StockDetail from '@/components/detail/StockDetail.vue';
const selectedRow = ref<WatchItem | null>(null);

function toggleRow(row: WatchItem) {
  selectedRow.value = selectedRow.value?.id === row.id ? null : row;
}
```

Add sorter to the columns that need sorting. Since quotes are looked up from quoteStore, we need custom sorter functions. Update the columns array:

```typescript
const columns: DataTableColumns<WatchItem> = [
  { title: '代码', key: 'code', width: 80 },
  {
    title: '名称', key: 'name', width: 100, ellipsis: true,
    sorter: (a: WatchItem, b: WatchItem) => a.name.localeCompare(b.name),
  },
  {
    title: '最新价', key: 'price', width: 100,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.price ?? 0) - (qb?.price ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      return q?.price?.toFixed(2) ?? '--';
    }
  },
  {
    title: '涨跌幅', key: 'change_pct', width: 100,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.change_pct ?? 0) - (qb?.change_pct ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change_pct;
      const color = v >= 0 ? '#f85149' : '#3fb950';
      return h('span', { style: { color, fontWeight: 500 } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`);
    }
  },
  {
    title: '涨跌额', key: 'change', width: 90,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.change ?? 0) - (qb?.change ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change;
      const color = v >= 0 ? '#f85149' : '#3fb950';
      return h('span', { style: { color } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}`);
    }
  },
  {
    title: '换手率', key: 'turnover_rate', width: 80,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.turnover_rate ?? 0) - (qb?.turnover_rate ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q || q.turnover_rate == null) return '--';
      return h('span', `${q.turnover_rate.toFixed(2)}%`);
    }
  },
];
```

Add `@row-click` handler to `NDataTable` and the detail panel below the table. Update the template:

```vue
    <NDataTable
      v-else
      :columns="columns"
      :data="watchlist.items"
      :bordered="false"
      :single-line="true"
      size="small"
      :row-props="(row: WatchItem) => ({
        style: `height: 36px; cursor: pointer; ${selectedRow?.id === row.id ? 'background: var(--color-bg-elevated, rgba(255,255,255,0.04))' : ''}`,
        onContextmenu: (e: MouseEvent) => handleContextMenu(e, row)
      })"
      flex-height
      class="watchlist-table"
      @update:sorter="() => {}"
    />

    <StockDetail
      v-if="selectedRow"
      :item="selectedRow"
      @close="selectedRow = null"
    />
```

Note: `@update:sorter="() => {}"` is needed to prevent NDataTable from throwing warnings about uncontrolled sorter state. The component will handle sorting internally.

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit 2>&1`
Expected: No new type errors

- [ ] **Step 3: Commit**

```bash
git add src/components/watchlist/WatchlistTable.vue
git commit -m "feat: add row click detail expansion, column sorting to WatchlistTable"
```

---

### Task 11: Create Tencent data source adapter

**Files:**
- Create: `src-tauri/src/datasource/tencent.rs`

- [ ] **Step 1: Write tencent.rs**

Tencent stock API format: `http://qt.gtimg.cn/q=sh600519`
Response: `v_sh600519="1~平安银行~000001~28.50~0.15~0.53~..."` (fields separated by `~`)

```rust
use async_trait::async_trait;
use reqwest::Client;
use encoding::{Encoding, DecoderTrap};
use encoding::all::GBK;
use crate::domain::*;
use super::DataSource;

const TENCENT_URL: &str = "http://qt.gtimg.cn/q=";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct TencentAdapter {
    client: Client,
}

impl TencentAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_default(),
        }
    }

    fn code_to_tencent(code: &str, market: &str) -> String {
        if market == "CN" {
            if code.starts_with("6") || code.starts_with("5") || code.starts_with("9") {
                format!("sh{}", code)
            } else {
                format!("sz{}", code)
            }
        } else {
            code.to_string()
        }
    }

    fn parse_quote_line(line: &str) -> Option<Quote> {
        // Format: v_sh600519="1~name~code~price~prev_close~change_pct~..."
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        // Extract code from "v_shXXXXXX" or "v_szXXXXXX"
        let code_raw = var_part.strip_prefix("v_")?;
        let market = if code_raw.starts_with("sh") { "CN" } else if code_raw.starts_with("sz") { "CN" } else { "CN" };
        let code = if code_raw.len() >= 2 { code_raw[2..].to_string() } else { code_raw.to_string() };

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split('~').collect();

        if fields.len() < 38 { return None; }

        let name = fields[1].to_string();
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let prev_close = fields[4].parse::<f64>().unwrap_or(0.0);
        let change_pct = fields[32].parse::<f64>().unwrap_or(0.0);
        let change = price - prev_close;
        let open = fields[5].parse::<f64>().unwrap_or(0.0);
        let high = fields[33].parse::<f64>().unwrap_or(0.0);
        let low = fields[34].parse::<f64>().unwrap_or(0.0);
        let volume = fields[6].parse::<u64>().unwrap_or(0);
        let turnover = fields[37].parse::<f64>().unwrap_or(0.0);
        // Tencent uses "手" for volume, multiply by 100
        let volume_shares = volume * 100;

        Some(Quote {
            code,
            market: market.to_string(),
            name,
            price,
            change: (change * 100.0).round() / 100.0,
            change_pct,
            open,
            high,
            low,
            volume: volume_shares,
            turnover: (turnover * 10000.0 * 100.0).round() / 100.0,
            turnover_rate: None,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    fn parse_index_line(line: &str) -> Option<IndexQuote> {
        let eq_pos = line.find('=')?;
        let var_part = &line[..eq_pos];
        let name_raw = var_part.strip_prefix("v_")?;

        let quote_start = line[eq_pos + 1..].find('"')? + eq_pos + 2;
        let quote_end = line[quote_start..].find('"')?;
        let data = &line[quote_start..quote_start + quote_end];
        let fields: Vec<&str> = data.split('~').collect();

        if fields.len() < 32 { return None; }

        let name = fields[1].to_string();
        let price = fields[3].parse::<f64>().unwrap_or(0.0);
        let change_pct = fields[32].parse::<f64>().unwrap_or(0.0);
        let change = fields[31].parse::<f64>().unwrap_or(0.0);
        let volume = fields[6].parse::<u64>().unwrap_or(0);
        let turnover = fields[37].parse::<f64>().unwrap_or(0.0);

        Some(IndexQuote {
            code: name_raw.to_string(),
            name,
            price,
            change,
            change_pct,
            volume: volume * 100,
            turnover,
        })
    }
}

#[async_trait]
impl DataSource for TencentAdapter {
    fn name(&self) -> &str { "tencent" }

    fn display_name(&self) -> &str { "腾讯证券" }

    async fn fetch_realtime(
        &self,
        codes: &[String],
        market: &str,
    ) -> Result<Vec<Quote>, String> {
        let tenc_codes: Vec<String> = codes
            .iter()
            .map(|c| Self::code_to_tencent(c, market))
            .collect();
        let url = format!("{}{}", TENCENT_URL, tenc_codes.join(","));

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Tencent request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
        let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
            .map_err(|e| format!("Tencent GBK decode failed: {}", e))?;

        let quotes: Vec<Quote> = body
            .lines()
            .filter_map(Self::parse_quote_line)
            .collect();

        Ok(quotes)
    }

    async fn fetch_indices(&self) -> Result<Vec<IndexQuote>, String> {
        // Major A-share indices via Tencent codes
        let index_codes = "s_sh000001,s_sz399001,s_sz399006,s_sh000688";
        let url = format!("{}{}", TENCENT_URL, index_codes);

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .send()
            .await
            .map_err(|e| format!("Tencent indices request failed: {:#}", e))?;

        let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
        let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
            .map_err(|e| format!("Tencent GBK decode failed: {}", e))?;

        let indices: Vec<IndexQuote> = body
            .lines()
            .filter_map(Self::parse_index_line)
            .collect();

        Ok(indices)
    }

    async fn search(
        &self,
        keyword: &str,
        market: &str,
    ) -> Result<Vec<StockBrief>, String> {
        // Direct code lookup only (same strategy as Sina)
        let trimmed = keyword.trim();
        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            let tc_code = Self::code_to_tencent(trimmed, market);
            let url = format!("{}{}", TENCENT_URL, tc_code);
            let resp = self
                .client
                .get(&url)
                .header("Referer", "https://gu.qq.com")
                .send()
                .await
                .map_err(|e| format!("Tencent search request failed: {:#}", e))?;
            let body_bytes = resp.bytes().await.map_err(|e| format!("Tencent read failed: {:#}", e))?;
            let body = GBK.decode(&body_bytes, DecoderTrap::Replace)
                .map_err(|e| format!("Tencent GBK decode failed: {}", e))?;

            for line in body.lines() {
                if let Some(quote) = Self::parse_quote_line(line) {
                    if !quote.name.is_empty() {
                        return Ok(vec![StockBrief {
                            code: quote.code,
                            market: quote.market,
                            name: quote.name,
                        }]);
                    }
                }
            }
        }
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<bool, String> {
        let codes = vec!["000001".to_string()];
        self.fetch_realtime(&codes, "CN")
            .await
            .map(|q| !q.is_empty())
    }
}
```

- [ ] **Step 2: Register Tencent module**

Add to `src-tauri/src/datasource/mod.rs`:

```rust
pub mod tencent;
```

- [ ] **Step 3: Register TencentAdapter in lib.rs**

In `src-tauri/src/lib.rs`, after the Eastmoney registration line, add:

```rust
            ds_manager.register(Box::new(
                crate::datasource::tencent::TencentAdapter::new(),
            ));
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/datasource/tencent.rs src-tauri/src/datasource/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add Tencent data source adapter"
```

---

### Task 12: Create market_clock module for trading hours

**Files:**
- Create: `src-tauri/src/datasource/market_clock.rs`

- [ ] **Step 1: Write market_clock.rs**

```rust
use chrono::{Local, Datelike, NaiveTime, Weekday};

/// A-share market trading session
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketSession {
    /// Before 9:30 AM — pre-market
    PreOpen,
    /// 9:30–11:30 AM — morning trading
    MorningTrade,
    /// 11:30 AM–1:00 PM — lunch break
    LunchBreak,
    /// 1:00–3:00 PM — afternoon trading
    AfternoonTrade,
    /// After 3:00 PM or weekend/holiday — closed
    Closed,
}

impl MarketSession {
    /// Determine the current A-share market session (China Standard Time)
    pub fn current() -> Self {
        let now = Local::now();

        // Check weekend
        match now.weekday() {
            Weekday::Sat | Weekday::Sun => return Self::Closed,
            _ => {}
        }

        let time = now.time();

        // Morning session: 9:30 - 11:30
        let morning_start = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        let morning_end = NaiveTime::from_hms_opt(11, 30, 0).unwrap();

        // Afternoon session: 13:00 - 15:00
        let afternoon_start = NaiveTime::from_hms_opt(13, 0, 0).unwrap();
        let afternoon_end = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

        if time < morning_start {
            Self::PreOpen
        } else if time < morning_end {
            Self::MorningTrade
        } else if time < afternoon_start {
            Self::LunchBreak
        } else if time < afternoon_end {
            Self::AfternoonTrade
        } else {
            Self::Closed
        }
    }

    /// Recommended polling interval in seconds for this session
    pub fn recommended_interval(&self) -> u64 {
        match self {
            Self::MorningTrade | Self::AfternoonTrade => 2,  // Active trading: 2s
            Self::PreOpen => 5,                                // Pre-market: 5s
            Self::LunchBreak => 10,                            // Lunch break: 10s
            Self::Closed => 30,                                // Closed/weekend: 30s
        }
    }

    /// Human-readable session name
    pub fn name(&self) -> &str {
        match self {
            Self::PreOpen => "盘前",
            Self::MorningTrade => "早盘",
            Self::LunchBreak => "午休",
            Self::AfternoonTrade => "午盘",
            Self::Closed => "休市",
        }
    }
}
```

- [ ] **Step 2: Add market_clock module to datasource**

In `src-tauri/src/datasource/mod.rs`, add:

```rust
pub mod market_clock;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/datasource/market_clock.rs src-tauri/src/datasource/mod.rs
git commit -m "feat: add market_clock module for A-share trading session detection"
```

---

### Task 13: Integrate market_clock into Scheduler for dynamic polling

**Files:**
- Modify: `src-tauri/src/cache/mod.rs`

- [ ] **Step 1: Update Scheduler::spawn to use dynamic interval**

Modify the `spawn` function signature and body in `src-tauri/src/cache/mod.rs`. Replace the existing `spawn` method:

```rust
use crate::datasource::market_clock::MarketSession;

impl Scheduler {
    /// Spawn the global polling loop in a background tokio task.
    pub fn spawn(
        data_manager: Arc<crate::datasource::DataSourceManager>,
        cache: Arc<QuoteCache>,
        db: Arc<crate::db::Database>,
        app_handle: tauri::AppHandle,
        base_interval_secs: u64,
    ) {
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(base_interval_secs));
            let mut last_session = MarketSession::current();

            loop {
                interval.tick().await;

                // Check market session and adjust interval dynamically
                let session = MarketSession::current();
                if session != last_session {
                    let new_interval = session.recommended_interval();
                    interval = tokio::time::interval(Duration::from_secs(new_interval));
                    last_session = session;
                    // Emit session change for frontend awareness
                    let _ = app_handle.emit("market-session-changed", serde_json::json!({
                        "session": session.name(),
                        "interval_secs": new_interval,
                    }));
                }

                // 1. Get watchlist codes
                let codes = match db.get_watch_codes() {
                    Ok(c) if !c.is_empty() => c,
                    _ => {
                        Self::fetch_and_emit_indices(&data_manager, &cache, &app_handle).await;
                        continue;
                    }
                };

                // Skip quote polling outside trading hours if no cache
                if session == MarketSession::Closed {
                    let cached = cache.get_all_quotes();
                    if cached.is_empty() {
                        // On very first run with no cache, still try fetching
                    } else {
                        // Outside trading hours with cache: emit cached data and continue
                        let _ = app_handle.emit("quotes-updated", &cached);
                        Self::fetch_and_emit_indices(&data_manager, &cache, &app_handle).await;
                        continue;
                    }
                }

                // 2. Group by market
                let mut cn_codes: Vec<String> = Vec::new();
                for (code, market) in &codes {
                    if market == "CN" {
                        cn_codes.push(code.clone());
                    }
                }

                // 3. Batch fetch quotes
                if !cn_codes.is_empty() {
                    if let Some(source) = data_manager.active_source() {
                        match source.fetch_realtime(&cn_codes, "CN").await {
                            Ok(quotes) => {
                                cache.update_quotes(&quotes);
                                let _ = app_handle.emit("quotes-updated", &quotes);
                            }
                            Err(_e) => {
                                let cached = cache.get_all_quotes();
                                if !cached.is_empty() {
                                    let _ = app_handle.emit("quotes-updated", &cached);
                                }
                            }
                        }
                    }
                }

                // 4. Refresh indices every cycle
                Self::fetch_and_emit_indices(&data_manager, &cache, &app_handle).await;
            }
        });
    }

    // ... fetch_and_emit_indices remains unchanged
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cache/mod.rs
git commit -m "feat: integrate market_clock into Scheduler for dynamic polling intervals"
```

---

### Task 14: Implement window position/size save and restore

**Files:**
- Modify: `src-tauri/src/lib.rs` (window event handlers + restore on startup)
- Modify: `src-tauri/src/commands/window.rs` (save command)

- [ ] **Step 1: Add window position save logic to lib.rs**

In the `setup` closure of `src-tauri/src/lib.rs`, replace the main window close handler section with full position tracking. Replace the section that currently reads:

```rust
            // Prevent main window from closing — hide instead of destroy
            if let Some(main) = app.get_webview_window("main") {
                let main_clone = main.clone();
                let _ = main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_clone.hide();
                    }
                });
            }
```

With:

```rust
            // Main window: hide on close, save position/size on move/resize
            if let Some(main) = app.get_webview_window("main") {
                let main_clone = main.clone();
                let db_clone = db.clone();
                let _ = main.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            // Save position before hiding
                            if let Ok(pos) = main_clone.outer_position() {
                                let _ = db_clone.set_setting("window_x", &pos.x.to_string());
                                let _ = db_clone.set_setting("window_y", &pos.y.to_string());
                            }
                            if let Ok(size) = main_clone.outer_size() {
                                let _ = db_clone.set_setting("window_width", &size.width.to_string());
                                let _ = db_clone.set_setting("window_height", &size.height.to_string());
                            }
                            let _ = main_clone.hide();
                        }
                        tauri::WindowEvent::Moved(pos) => {
                            let _ = db_clone.set_setting("window_x", &pos.x.to_string());
                            let _ = db_clone.set_setting("window_y", &pos.y.to_string());
                        }
                        tauri::WindowEvent::Resized(size) => {
                            let _ = db_clone.set_setting("window_width", &size.width.to_string());
                            let _ = db_clone.set_setting("window_height", &size.height.to_string());
                        }
                        _ => {}
                    }
                });

                // Restore saved window position and size (after window is created)
                if let Ok(Some(w)) = db.get_setting("window_width") {
                    if let Ok(Some(h)) = db.get_setting("window_height") {
                        if let (Ok(w_val), Ok(h_val)) = (w.parse::<u32>(), h.parse::<u32>()) {
                            let _ = main.set_size(tauri::PhysicalSize::new(w_val, h_val));
                        }
                    }
                }
                if let Ok(Some(x)) = db.get_setting("window_x") {
                    if let Ok(Some(y)) = db.get_setting("window_y") {
                        if let (Ok(x_val), Ok(y_val)) = (x.parse::<i32>(), y.parse::<i32>()) {
                            let _ = main.set_position(tauri::PhysicalPosition::new(x_val, y_val));
                        }
                    }
                }
            }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: save and restore main window position and size"
```

---

### Task 15: Final Integration Test

**Files:** None (test and verify)

- [ ] **Step 1: Full build verification**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1`
Expected: Successful build, no warnings

- [ ] **Step 2: TypeScript type-check**

Run: `npx vue-tsc --noEmit 2>&1`
Expected: No new type errors

- [ ] **Step 3: Final review checklist**

Verify each Phase 2 feature is complete:

1. ✅ 模糊搜索添加自选 (already complete)
2. ✅ 个股详情：分时图 + 五档盘口 + 基本面概要 (Tasks 1-10)
3. ✅ 日间/夜间主题切换 (already complete)
4. ✅ 腾讯备用数据源 + 切换功能 (Task 11)
5. ✅ 涨跌排序和筛选 (Task 10)
6. ✅ 窗口位置/大小记忆 (Task 14)
7. ✅ 开盘/收盘/午休自动调整轮询频率 (Tasks 12-13)

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "feat: complete Phase 2 — improved experience features"
```

---

## Verification Plan

After all tasks complete, verify end-to-end:

1. **Stock Detail**: Run `npm run tauri dev`, add a stock to watchlist, click row → detail panel expands with summary + depth + chart
2. **Tencent Data Source**: Open settings, switch to "腾讯证券" datasource, verify quotes load
3. **Sort/Filter**: Click column headers in watchlist table → data sorts ascending/descending
4. **Window Memory**: Resize and move main window, close (hide), re-open → position and size are restored
5. **Trading Hours**: Check scheduler logs/behavior during different sessions — faster polling during trading, slower otherwise
