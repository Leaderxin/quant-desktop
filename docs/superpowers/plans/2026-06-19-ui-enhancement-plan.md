# UI Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement three UI enhancements: TopBar simplification, chart period switching (minute/daily/weekly/monthly), and global status bar with copyright/contact/QR code.

**Architecture:** Frontend changes use Vue 3 Composition API with a shared `useChart` composable to avoid code duplication between MinuteChart and new KLineChart. Backend extends the DataSource trait pattern with a new `fetch_kline` method, implemented in both Sina and Tencent adapters. The StatusBar is a standalone component integrated at the AppLayout level.

**Tech Stack:** Vue 3, Pinia, Naive UI, klinecharts v10, Tauri 2, Rust, reqwest, serde

---

### Task 1: TopBar — Remove Brand Icon and Program Name

**Files:**
- Modify: `src/components/layout/TopBar.vue`

- [ ] **Step 1: Remove `.brand` template block**

Delete lines 28-44 in the template section:

```vue
<template>
  <header class="top-bar">
    <div class="top-left">
      <div class="brand">
        <svg class="brand-icon" viewBox="0 0 32 32" fill="none">
          <rect x="3" y="10" width="3" height="10" rx="0.5" fill="#f85149"/>
          <line x1="4.5" y1="6" x2="4.5" y2="10" stroke="#f85149" stroke-width="1.5" stroke-linecap="round"/>
          <line x1="4.5" y1="20" x2="4.5" y2="24" stroke="#f85149" stroke-width="1.5" stroke-linecap="round"/>
          <rect x="10" y="14" width="3" height="7" rx="0.5" fill="#3fb950"/>
          <line x1="11.5" y1="8" x2="11.5" y2="14" stroke="#3fb950" stroke-width="1.5" stroke-linecap="round"/>
          <line x1="11.5" y1="21" x2="11.5" y2="25" stroke="#3fb950" stroke-width="1.5" stroke-linecap="round"/>
          <rect x="17" y="8" width="3" height="14" rx="0.5" fill="#f85149"/>
          <line x1="18.5" y1="4" x2="18.5" y2="8" stroke="#f85149" stroke-width="1.5" stroke-linecap="round"/>
          <line x1="18.5" y1="22" x2="18.5" y2="27" stroke="#f85149" stroke-width="1.5" stroke-linecap="round"/>
          <polyline points="24,22 24,8 28,8" stroke="#58a6ff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
          <line x1="19" y1="13" x2="24" y2="8" stroke="#58a6ff" stroke-width="2" stroke-linecap="round"/>
          <line x1="2" y1="28" x2="30" y2="28" stroke="#30363d" stroke-width="1" stroke-linecap="round"/>
        </svg>
        <span class="brand-name">QuantDesktop</span>
      </div>
```

Result should be:

```vue
<template>
  <header class="top-bar">
    <div class="top-left">
```

- [ ] **Step 2: Remove `.brand` and `.brand-icon` and `.brand-name` CSS**

Delete lines 112-125 from `<style scoped>`:

```css
.brand {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.brand-icon {
  width: 20px;
  height: 20px;
}
.brand-name {
  font-weight: var(--font-weight-semibold);
  font-size: var(--text-md);
  color: var(--color-text-primary);
  letter-spacing: -0.01em;
}
```

- [ ] **Step 3: Verify no build errors**

Run: `npx vue-tsc --noEmit`
Expected: No errors related to TopBar.vue.

- [ ] **Step 4: Commit**

```bash
git add src/components/layout/TopBar.vue
git commit -m "refactor: remove brand icon and program name from TopBar"
```

---

### Task 2: StatusBar Component

**Files:**
- Create: `src/components/layout/StatusBar.vue`
- Modify: `src/components/layout/AppLayout.vue`

- [ ] **Step 1: Create StatusBar component**

Create `src/components/layout/StatusBar.vue`:

```vue
<script setup lang="ts">
import { ref } from 'vue';
import { NPopover } from 'naive-ui';

withDefaults(defineProps<{
  version?: string;
  copyright?: string;
  contactEmail?: string;
  qrcodeSrc?: string;
}>(), {
  version: '0.5.3',
  copyright: '© 2026 QuantDesktop',
  contactEmail: 'biz@example.com',
  qrcodeSrc: '',
});

const showQr = ref(false);
</script>

<template>
  <footer class="status-bar">
    <div class="status-left">
      <span class="status-version">v{{ version }}</span>
      <span class="status-divider">|</span>
      <span class="status-copyright">{{ copyright }}</span>
    </div>
    <div class="status-right">
      <a class="status-contact" :href="`mailto:${contactEmail}`" title="商务合作">
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" style="vertical-align: middle; margin-right: 4px;">
          <rect x="1.5" y="3.5" width="13" height="9" rx="1"/>
          <path d="M1.5 4l7 4.5 7-4.5"/>
        </svg>
        {{ contactEmail }}
      </a>
      <NPopover trigger="click" placement="top" :show-arrow="true">
        <template #trigger>
          <button class="status-qr-btn" aria-label="点击入群">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor" style="vertical-align: middle; margin-right: 3px;">
              <path d="M8 2a5 5 0 0 0-5 5v1a2 2 0 0 0-2 2v2a2 2 0 0 0 2 2h1a1 1 0 0 0 1-1V8a1 1 0 0 0-1-1h-.5A4 4 0 0 1 8 3h.5a4 4 0 0 1 4 4v1h-.5a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h1a2 2 0 0 0 2-2v-2a2 2 0 0 0-2-2v-1a5 5 0 0 0-5-5z"/>
            </svg>
            点击入群
          </button>
        </template>
        <div class="qr-popover">
          <img
            v-if="qrcodeSrc"
            :src="qrcodeSrc"
            alt="微信群二维码"
            class="qr-image"
          />
          <div v-else class="qr-placeholder">
            <svg viewBox="0 0 100 100" width="120" height="120" fill="none">
              <rect x="10" y="10" width="30" height="30" rx="2" stroke="currentColor" stroke-width="2"/>
              <rect x="10" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="10" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="60" y="10" width="30" height="30" rx="2" stroke="currentColor" stroke-width="2"/>
              <rect x="60" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="76" y="10" width="14" height="14" fill="currentColor"/>
              <rect x="60" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="76" y="26" width="14" height="14" fill="currentColor"/>
              <rect x="10" y="60" width="30" height="30" rx="2" stroke="currentColor" stroke-width="2"/>
              <rect x="10" y="60" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="60" width="14" height="14" fill="currentColor"/>
              <rect x="10" y="76" width="14" height="14" fill="currentColor"/>
              <rect x="26" y="76" width="14" height="14" fill="currentColor"/>
              <rect x="44" y="44" width="12" height="12" fill="currentColor"/>
              <rect x="60" y="44" width="12" height="12" fill="currentColor"/>
              <rect x="44" y="60" width="12" height="12" fill="currentColor"/>
              <rect x="60" y="60" width="12" height="12" fill="currentColor"/>
            </svg>
            <p style="font-size: 10px; color: var(--color-text-tertiary); margin-top: 6px;">请替换为微信群二维码</p>
          </div>
        </div>
      </NPopover>
    </div>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 28px;
  padding: 0 var(--space-4);
  background: var(--color-surface-1);
  border-top: 1px solid var(--color-border-0);
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}
.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
.status-version {
  font-weight: var(--font-weight-medium);
  color: var(--color-accent);
  font-family: var(--font-mono);
}
.status-divider {
  color: var(--color-border-1);
  user-select: none;
}
.status-copyright {
  color: var(--color-text-tertiary);
}
.status-contact {
  display: inline-flex;
  align-items: center;
  color: var(--color-text-tertiary);
  text-decoration: none;
  transition: color var(--transition-fast);
  cursor: pointer;
}
.status-contact:hover {
  color: var(--color-accent);
}
.status-qr-btn {
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--color-accent);
  color: #fff;
  font-size: 10px;
  font-family: var(--font-sans);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: filter var(--transition-fast);
}
.status-qr-btn:hover {
  filter: brightness(1.2);
}
.qr-popover {
  padding: 8px;
  text-align: center;
}
.qr-image {
  width: 140px;
  height: 140px;
  border-radius: var(--radius-sm);
}
.qr-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  color: var(--color-text-tertiary);
  padding: 8px;
}
</style>
```

- [ ] **Step 2: Integrate StatusBar into AppLayout**

Edit `src/components/layout/AppLayout.vue`:

In template, add `<StatusBar />` after `</main>` and before the closing `</div>`:

```vue
    <main class="main-content">
      <WatchlistTable />
    </main>
    <StatusBar />
  </div>
```

In script, add import:

```typescript
import StatusBar from './StatusBar.vue';
```

- [ ] **Step 3: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/layout/StatusBar.vue src/components/layout/AppLayout.vue
git commit -m "feat: add global StatusBar with version, copyright, contact, and QR code"
```

---

### Task 3: Backend — Add KLineData Domain Type

**Files:**
- Modify: `src-tauri/src/domain/mod.rs`

- [ ] **Step 1: Add KLineData struct**

Add after the `MinuteData` struct (after line 57) in `src-tauri/src/domain/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KLineData {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub turnover: f64,
}
```

- [ ] **Step 2: Verify Rust compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20`
Expected: Compiles successfully (unused warning for KLineData is OK at this stage).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/domain/mod.rs
git commit -m "feat: add KLineData domain type for K-line chart data"
```

---

### Task 4: Backend — Add fetch_kline to DataSource Trait

**Files:**
- Modify: `src-tauri/src/datasource/mod.rs`

- [ ] **Step 1: Add fetch_kline to DataSource trait**

Add after `fetch_minute_data` method in the trait (after line 67) in `src-tauri/src/datasource/mod.rs`:

```rust
    /// Fetch K-line data for charting (daily/weekly/monthly)
    async fn fetch_kline(
        &self,
        _code: &str,
        _market: &str,
        _period: &str,
    ) -> Result<Vec<crate::domain::KLineData>, String> {
        Ok(vec![])
    }
```

- [ ] **Step 2: Verify Rust compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasource/mod.rs
git commit -m "feat: add fetch_kline method to DataSource trait"
```

---

### Task 5: Backend — Add get_kline Tauri Command

**Files:**
- Modify: `src-tauri/src/commands/quote.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add get_kline command**

In `src-tauri/src/commands/quote.rs`, add import for KLineData:

```rust
use crate::domain::{Quote, IndexQuote, Depth, MinuteData, KLineData};
```

Then add the command after `get_intraday` (after line 37):

```rust
#[tauri::command]
pub async fn get_kline(
    code: String,
    market: String,
    period: String,
    manager: State<'_, Arc<DataSourceManager>>,
) -> Result<Vec<KLineData>, String> {
    let source = manager.active_source()
        .ok_or("No active data source")?;
    source.fetch_kline(&code, &market, &period).await
}
```

- [ ] **Step 2: Register get_kline in lib.rs**

In `src-tauri/src/lib.rs`, add the command to the handler:

```rust
            commands::quote::get_kline,
```

Insert after `commands::quote::get_intraday,` on line 367.

- [ ] **Step 3: Verify build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/quote.rs src-tauri/src/lib.rs
git commit -m "feat: add get_kline Tauri command for K-line data"
```

---

### Task 6: Backend — Implement fetch_kline in Tencent Adapter

**Files:**
- Modify: `src-tauri/src/datasource/tencent.rs`

- [ ] **Step 1: Add fetch_kline implementation**

Add after `fetch_minute_data` method (after line 290), before `fetch_depth`:

```rust
    async fn fetch_kline(
        &self,
        code: &str,
        market: &str,
        period: &str,
    ) -> Result<Vec<crate::domain::KLineData>, String> {
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

        let url = format!(
            "http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={},{},,,200,qfq",
            tc_code, period_param
        );

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://gu.qq.com")
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("Tencent kline request failed: {:#}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Tencent kline parse failed: {}", e))?;

        // Extract K-line data array
        // Format: { "data": { "sh600519": { "day": [...] or "qfqday": [...] } } }
        let stock_data = body
            .pointer("/data")
            .and_then(|d| d.as_object())
            .and_then(|obj| obj.values().next());

        let klines = stock_data
            .and_then(|stock| {
                stock.get(period_param)
                    .or_else(|| stock.get(&format!("qfq{}", period_param)))
            })
            .and_then(|arr| arr.as_array())
            .cloned()
            .unwrap_or_default();

        let data: Vec<crate::domain::KLineData> = klines
            .iter()
            .filter_map(|pt| {
                let arr = pt.as_array()?;
                if arr.len() < 6 { return None; }
                // Format: ["2026-06-19", "1845.00", "1860.00", "1840.00", "1850.00", "1234567.00"]
                let date = arr[0].as_str()?.to_string();
                let open: f64 = arr[1].as_str()?.parse().ok()?;
                let high: f64 = arr[3].as_str()?.parse().ok()?;
                let low: f64 = arr[4].as_str()?.parse().ok()?;
                let close: f64 = arr[2].as_str()?.parse().ok()?;
                let volume: f64 = arr[5].as_str()?.parse().unwrap_or(0.0);
                // turnover may be in arr[6] if available
                let turnover: f64 = arr.get(6).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                Some(crate::domain::KLineData {
                    date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    turnover,
                })
            })
            .collect();

        Ok(data)
    }
```

- [ ] **Step 2: Verify build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasource/tencent.rs
git commit -m "feat: implement fetch_kline in Tencent adapter"
```

---

### Task 7: Backend — Implement fetch_kline in Sina Adapter

**Files:**
- Modify: `src-tauri/src/datasource/sina.rs`

- [ ] **Step 1: Add fetch_kline implementation**

Add after `fetch_minute_data` method (after line 313), before `fetch_depth`:

```rust
    async fn fetch_kline(
        &self,
        code: &str,
        market: &str,
        period: &str,
    ) -> Result<Vec<crate::domain::KLineData>, String> {
        let symbol = if code.starts_with("s_") {
            code[2..].to_string()
        } else {
            Self::code_to_sina(code, market)
        };

        // Map period to Sina API scale parameter
        let scale = match period {
            "weekly" => "120",   // Approximate via daily aggregation
            "monthly" => "60",   // Approximate via daily aggregation
            _ => "240",          // Daily
        };

        let url = format!(
            "http://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData?symbol={}&scale={}&ma=no&datalen=200",
            symbol, scale
        );

        let resp = self
            .client
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .map_err(|e| format!("Sina kline request failed: {:#}", e))?;

        let body_text = resp
            .text()
            .await
            .map_err(|e| format!("Sina kline read failed: {:#}", e))?;

        let json_str = body_text.trim_end_matches(|c| c != ']').trim();
        let raw: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| format!("Sina kline parse failed: {} — body: {}", e, &body_text[..body_text.len().min(100)]))?;

        let data: Vec<crate::domain::KLineData> = raw
            .iter()
            .filter_map(|pt| {
                let date = pt.get("day")?.as_str()?.to_string();
                let open: f64 = pt.get("open")?.as_str()?.parse().ok()?;
                let high: f64 = pt.get("high")?.as_str()?.parse().ok()?;
                let low: f64 = pt.get("low")?.as_str()?.parse().ok()?;
                let close: f64 = pt.get("close")?.as_str()?.parse().ok()?;
                let volume: f64 = pt.get("volume")?.as_str()?.parse().unwrap_or(0.0);
                // Sina doesn't provide turnover in K-line data, default to 0
                let turnover: f64 = 0.0;
                Some(crate::domain::KLineData {
                    date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    turnover,
                })
            })
            .collect();

        Ok(data)
    }
```

- [ ] **Step 2: Verify build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20`
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/datasource/sina.rs
git commit -m "feat: implement fetch_kline in Sina adapter"
```

---

### Task 8: Frontend — Add KLineData and PeriodType to TypeScript Types

**Files:**
- Modify: `src/types/index.ts`

- [ ] **Step 1: Add KLineData interface and PeriodType**

Add after `MinuteData` interface (after line 62) in `src/types/index.ts`:

```typescript
export interface KLineData {
  date: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  turnover: number;
}

export type PeriodType = 'minute' | 'daily' | 'weekly' | 'monthly';
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "feat: add KLineData interface and PeriodType to TypeScript types"
```

---

### Task 9: Frontend — Create useChart Composable

**Files:**
- Create: `src/composables/useChart.ts`

- [ ] **Step 1: Create useChart composable**

Create `src/composables/useChart.ts`:

```typescript
import { ref, watch, onUnmounted, type Ref, type MaybeRef, unref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { init, dispose } from 'klinecharts';
import type { Chart, KLineData as KCLineData, DataLoader } from 'klinecharts';
import type { MinuteData, KLineData, PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';

export function useChart(options: {
  chartRef: Ref<HTMLElement | null>;
  code: MaybeRef<string>;
  market: MaybeRef<string>;
  name?: MaybeRef<string>;
}) {
  const settings = useSettingsStore();

  let chart: Chart | null = null;
  const loading = ref(false);
  const error = ref('');
  let abortController: AbortController | null = null;

  const klineData = ref<KCLineData[]>([]);

  const dataLoader: DataLoader = {
    getBars: (params) => {
      if (params.type === 'init') {
        params.callback(klineData.value, false);
      } else {
        params.callback([], false);
      }
    },
  };

  function applyChartStyles() {
    if (!chart) return;
    const isDark = settings.theme === 'dark';
    const lineColor = isDark ? 'rgba(255,255,255,0.25)' : 'rgba(0,0,0,0.35)';
    const gridHColor = isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)';
    const gridVColor = isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.04)';
    const axisColor = isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)';
    const tickColor = isDark ? '#8b949e' : '#656d76';
    const tooltipBg = isDark ? 'rgba(22,27,34,0.95)' : 'rgba(255,255,255,0.95)';
    const tooltipText = isDark ? '#c9d1d9' : '#24292f';
    const separatorColor = isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)';
    const crosshairBg = isDark ? 'rgba(22,27,34,0.9)' : 'rgba(31,35,40,0.85)';
    const crosshairText = isDark ? '#c9d1d9' : '#e6edf3';

    chart.setStyles({
      grid: {
        show: true,
        horizontal: { show: true, color: gridHColor, size: 1, dashedValue: [2, 2] },
        vertical: { show: true, color: gridVColor, size: 1, dashedValue: [2, 2] },
      },
      candle: {
        type: 'area',
        bar: { upColor: '#f85149', downColor: '#3fb950', upBorderColor: '#f85149', downBorderColor: '#3fb950', noChangeColor: '#8b949e', compareRule: 'previous_close' as any },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          labels: ['时间', '开', '高', '低', '收', '量', '额'],
          title: { show: false } as any,
          rect: { position: 'pointer' as any, paddingLeft: 8, paddingTop: 4, paddingRight: 8, paddingBottom: 4, offsetLeft: 12, offsetTop: 8, offsetRight: 0, offsetBottom: 0, borderRadius: 4, borderSize: 0, backgroundColor: tooltipBg } as any,
          text: { size: 11, color: tooltipText, family: 'var(--font-sans)' } as any,
        } as any,
        priceMark: {
          high: { show: false } as any,
          low: { show: false } as any,
          last: { show: false, extendTexts: [] } as any,
        },
      },
      indicator: {
        ohlc: { upColor: '#f85149', downColor: '#3fb950', noChangeColor: '#8b949e', compareRule: 'previous_close' },
        bars: [] as any,
        lastValueMark: { show: false } as any,
        tooltip: { show: true, labels: ['', '', '', '', '', '量', '额'], text: { size: 11, color: tooltipText } } as any,
      },
      xAxis: {
        show: true,
        size: 'auto',
        axisLine: { show: true, color: axisColor, size: 1 },
        tickLine: { show: false } as any,
        tickText: { size: 10, color: tickColor, family: 'var(--font-sans)', marginStart: 0, marginEnd: 0 } as any,
      },
      yAxis: {
        show: true,
        size: 'auto',
        axisLine: { show: false } as any,
        tickLine: { show: false } as any,
        tickText: { size: 10, color: tickColor, family: 'var(--font-sans)' } as any,
      },
      separator: { size: 1, color: separatorColor, fill: false, activeBackgroundColor: 'rgba(255,255,255,0.02)' },
      crosshair: {
        show: true,
        horizontal: { show: true, line: { show: true, color: lineColor, size: 1 }, text: { show: true, size: 10, color: crosshairText, family: 'var(--font-mono)', backgroundColor: crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any } as any,
        vertical: { show: true, line: { show: true, color: lineColor, size: 1 }, text: { show: true, size: 10, color: crosshairText, family: 'var(--font-mono)', backgroundColor: crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any } as any,
      },
    });
  }

  function applyCandlestickStyles() {
    if (!chart) return;
    chart.setStyles({
      candle: {
        type: 'candle_solid',
        bar: { upColor: '#f85149', downColor: '#3fb950', upBorderColor: '#f85149', downBorderColor: '#3fb950', noChangeColor: '#8b949e', compareRule: 'previous_close' as any },
        area: { lineSize: 1.5, lineColor: '#58a6ff' },
        tooltip: {
          labels: ['日期', '开', '高', '低', '收', '量', '额'],
          title: { show: false } as any,
          rect: { position: 'pointer' as any, paddingLeft: 8, paddingTop: 4, paddingRight: 8, paddingBottom: 4, offsetLeft: 12, offsetTop: 8, offsetRight: 0, offsetBottom: 0, borderRadius: 4, borderSize: 0, backgroundColor: '#161b22' } as any,
          text: { size: 11, color: '#c9d1d9', family: 'var(--font-sans)' } as any,
        } as any,
        priceMark: {
          high: { show: false } as any,
          low: { show: false } as any,
          last: { show: false, extendTexts: [] } as any,
        },
      },
    });
  }

  async function initChart(period: PeriodType) {
    if (!options.chartRef.value) return;
    if (!chart) {
      chart = init(options.chartRef.value, {
        locale: 'zh-CN',
        layout: { basicParams: { yAxisInside: true } },
      });
      if (!chart) return;

      chart.overrideIndicator({
        name: 'VOL',
        shortName: '成交量',
        series: 'volume',
        calcParams: [5, 10, 20],
        precision: 0,
        shouldFormatBigNumber: true,
        minValue: 0,
        figures: [
          { key: 'ma1', title: 'MA5: ', type: 'line' },
          { key: 'ma2', title: 'MA10: ', type: 'line' },
          { key: 'ma3', title: 'MA20: ', type: 'line' },
          { key: 'volume', title: 'VOLUME: ', type: 'bar', baseValue: 0, styles: { upColor: 'rgba(248,81,73,0.4)', downColor: 'rgba(63,185,80,0.4)' } } as any,
        ],
      } as any);

      chart.setSymbol({ ticker: unref(options.code), name: unref(options.name) || unref(options.code) });
    }

    if (period === 'minute') {
      chart.setPeriod({ type: 'minute', span: 5 });
      applyChartStyles();
    } else {
      chart.setPeriod({ type: 'day', span: 1 });
      applyCandlestickStyles();
    }
  }

  async function loadData(period: PeriodType) {
    if (abortController) {
      abortController.abort();
    }
    abortController = new AbortController();
    const { signal } = abortController;

    loading.value = true;
    error.value = '';
    try {
      if (period === 'minute') {
        const data = await invoke<MinuteData[]>('get_intraday', {
          code: unref(options.code),
          market: unref(options.market),
        });
        if (signal.aborted) return;

        if (data.length) {
          const today = new Date();
          klineData.value = data.map((d) => {
            let h = 0, m = 0;
            if (d.time.includes(':')) {
              [h, m] = d.time.split(':').map(Number);
            } else if (d.time.length >= 4) {
              h = Number(d.time.slice(0, 2));
              m = Number(d.time.slice(2, 4));
            }
            const ts = new Date(today.getFullYear(), today.getMonth(), today.getDate(), h || 0, m || 0).getTime();
            return {
              timestamp: ts,
              open: d.open ?? d.price,
              high: d.high ?? d.price,
              low: d.low ?? d.price,
              close: d.price,
              volume: d.volume,
            };
          });
        }
      } else {
        const data = await invoke<KLineData[]>('get_kline', {
          code: unref(options.code),
          market: unref(options.market),
          period: period,
        });
        if (signal.aborted) return;

        if (data.length) {
          klineData.value = data.map((d) => {
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
      }

      if (signal.aborted) return;
      if (chart) {
        chart.setDataLoader(dataLoader);
      }
    } catch (e) {
      if (signal.aborted) return;
      error.value = `加载数据失败: ${String(e).slice(0, 80)}`;
      console.error('[useChart] loadData failed:', e);
    } finally {
      if (!signal.aborted) {
        loading.value = false;
      }
    }
  }

  function disposeChart() {
    if (abortController) {
      abortController.abort();
      abortController = null;
    }
    if (chart) {
      dispose(chart);
      chart = null;
    }
  }

  // Watch for code/market changes and reload
  watch(
    () => [unref(options.code), unref(options.market)] as const,
    () => { loadData('minute'); },
  );

  // Watch for theme changes
  watch(() => settings.theme, () => {
    applyChartStyles();
  });

  onUnmounted(() => {
    disposeChart();
  });

  return {
    chart,
    loading,
    error,
    klineData,
    initChart,
    loadData,
    disposeChart,
    applyTheme: applyChartStyles,
    applyCandlestickStyles,
  };
}
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/composables/useChart.ts
git commit -m "feat: create useChart composable for shared klinecharts logic"
```

---

### Task 10: Frontend — Create ChartSwitcher Component

**Files:**
- Create: `src/components/detail/ChartSwitcher.vue`

- [ ] **Step 1: Create ChartSwitcher**

Create `src/components/detail/ChartSwitcher.vue`:

```vue
<script setup lang="ts">
import type { PeriodType } from '@/types';

defineProps<{
  modelValue: PeriodType;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: PeriodType];
}>();

const tabs: { key: PeriodType; label: string }[] = [
  { key: 'minute', label: '分时' },
  { key: 'daily', label: '日K' },
  { key: 'weekly', label: '周K' },
  { key: 'monthly', label: '月K' },
];
</script>

<template>
  <div class="chart-switcher" role="tablist" aria-label="图表类型切换">
    <button
      v-for="tab in tabs"
      :key="tab.key"
      role="tab"
      :aria-selected="modelValue === tab.key"
      class="switcher-tab"
      :class="{ active: modelValue === tab.key }"
      @click="emit('update:modelValue', tab.key)"
    >
      {{ tab.label }}
    </button>
  </div>
</template>

<style scoped>
.chart-switcher {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
  width: fit-content;
}
.switcher-tab {
  padding: 3px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: all var(--transition-fast);
  line-height: 1.4;
}
.switcher-tab:hover {
  color: var(--color-text-secondary);
}
.switcher-tab.active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
</style>
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/detail/ChartSwitcher.vue
git commit -m "feat: create ChartSwitcher component for chart period tabs"
```

---

### Task 11: Frontend — Refactor MinuteChart to Use useChart

**Files:**
- Modify: `src/components/detail/MinuteChart.vue`

- [ ] **Step 1: Rewrite MinuteChart with useChart composable**

Rewrite `src/components/detail/MinuteChart.vue`:

```vue
<script setup lang="ts">
import { ref, onMounted, watch, nextTick } from 'vue';
import { useChart } from '@/composables/useChart';

const props = defineProps<{
  code: string;
  market: string;
  name?: string;
}>();

const chartRef = ref<HTMLElement | null>(null);

const { loading, error, initChart, loadData } = useChart({
  chartRef,
  code: () => props.code,
  market: () => props.market,
  name: () => props.name,
});

onMounted(async () => {
  await nextTick();
  await initChart('minute');
  await loadData('minute');
});

// Reload when code/market changes (useChart already watches, but we need to re-init)
watch(() => [props.code, props.market], async () => {
  await nextTick();
  await initChart('minute');
  await loadData('minute');
});
</script>

<template>
  <div class="minute-chart">
    <div v-if="loading" class="chart-overlay">
      <span class="chart-status-text">加载分时图...</span>
    </div>
    <div v-else-if="error" class="chart-overlay chart-error-overlay" role="alert">
      <svg class="chart-error-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
        <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
        <path d="M8 4.5v3.5M8 10.5h.007" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
      <span class="chart-error-text">{{ error }}</span>
      <button class="chart-retry-btn" @click="loadData('minute')" aria-label="重新加载分时图">重试</button>
    </div>
    <div ref="chartRef" class="chart-container"></div>
  </div>
</template>

<style scoped>
.minute-chart {
  flex: 1;
  min-height: 320px;
  position: relative;
}
.chart-container {
  width: 100%;
  height: 320px;
}
.chart-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  background: var(--color-surface-1);
}
.chart-status-text {
  font-size: 12px;
  color: var(--color-text-tertiary);
}
.chart-error-overlay {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  background: var(--color-surface-1);
}
.chart-error-icon {
  color: var(--color-warning);
}
.chart-error-text {
  font-size: var(--text-xs);
  color: var(--color-warning);
  text-align: center;
  max-width: 240px;
  line-height: 1.4;
}
.chart-retry-btn {
  display: inline-flex;
  align-items: center;
  padding: 3px 12px;
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-sm);
  background: var(--color-warning-bg);
  color: var(--color-warning);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.chart-retry-btn:hover {
  filter: brightness(1.2);
}
</style>
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/detail/MinuteChart.vue
git commit -m "refactor: rewrite MinuteChart to use useChart composable"
```

---

### Task 12: Frontend — Create KLineChart Component

**Files:**
- Create: `src/components/detail/KLineChart.vue`

- [ ] **Step 1: Create KLineChart component**

Create `src/components/detail/KLineChart.vue`:

```vue
<script setup lang="ts">
import { ref, onMounted, watch, nextTick } from 'vue';
import { useChart } from '@/composables/useChart';
import type { PeriodType } from '@/types';

const props = defineProps<{
  code: string;
  market: string;
  name?: string;
  period: PeriodType;
}>();

const chartRef = ref<HTMLElement | null>(null);

const { loading, error, initChart, loadData } = useChart({
  chartRef,
  code: () => props.code,
  market: () => props.market,
  name: () => props.name,
});

onMounted(async () => {
  await nextTick();
  await initChart(props.period);
  await loadData(props.period);
});

// Reload when code, market, or period changes
watch(() => [props.code, props.market, props.period], async () => {
  await nextTick();
  await initChart(props.period);
  await loadData(props.period);
});
</script>

<template>
  <div class="kline-chart">
    <div v-if="loading" class="chart-overlay">
      <span class="chart-status-text">加载K线数据...</span>
    </div>
    <div v-else-if="error" class="chart-overlay chart-error-overlay" role="alert">
      <svg class="chart-error-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
        <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
        <path d="M8 4.5v3.5M8 10.5h.007" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
      <span class="chart-error-text">{{ error }}</span>
      <button class="chart-retry-btn" @click="loadData(period)" aria-label="重新加载K线数据">重试</button>
    </div>
    <div ref="chartRef" class="chart-container"></div>
  </div>
</template>

<style scoped>
.kline-chart {
  flex: 1;
  min-height: 320px;
  position: relative;
}
.chart-container {
  width: 100%;
  height: 320px;
}
.chart-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  background: var(--color-surface-1);
}
.chart-status-text {
  font-size: 12px;
  color: var(--color-text-tertiary);
}
.chart-error-overlay {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  background: var(--color-surface-1);
}
.chart-error-icon {
  color: var(--color-warning);
}
.chart-error-text {
  font-size: var(--text-xs);
  color: var(--color-warning);
  text-align: center;
  max-width: 240px;
  line-height: 1.4;
}
.chart-retry-btn {
  display: inline-flex;
  align-items: center;
  padding: 3px 12px;
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-sm);
  background: var(--color-warning-bg);
  color: var(--color-warning);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.chart-retry-btn:hover {
  filter: brightness(1.2);
}
</style>
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/detail/KLineChart.vue
git commit -m "feat: create KLineChart component for daily/weekly/monthly candlestick charts"
```

---

### Task 13: Frontend — Update StockDetail with Chart Switching

**Files:**
- Modify: `src/components/detail/StockDetail.vue`

- [ ] **Step 1: Add ChartSwitcher and conditional chart rendering**

Rewrite `src/components/detail/StockDetail.vue`:

```vue
<script setup lang="ts">
import { ref, computed } from 'vue';
import type { WatchItem, PeriodType } from '@/types';
import { useQuoteStore } from '@/stores/quote';
import StockSummary from './StockSummary.vue';
import DepthPanel from './DepthPanel.vue';
import MinuteChart from './MinuteChart.vue';
import KLineChart from './KLineChart.vue';
import ChartSwitcher from './ChartSwitcher.vue';

const props = defineProps<{
  item: WatchItem;
}>();

const emit = defineEmits<{
  close: [];
}>();

const quoteStore = useQuoteStore();
const quote = computed(() => quoteStore.getQuote(props.item.code, props.item.market));

const activePeriod = ref<PeriodType>('minute');
</script>

<template>
  <div class="stock-detail">
    <div class="detail-header">
      <div class="detail-title">
        <span class="detail-name">{{ item.name }}</span>
        <span class="detail-code">{{ item.code }}</span>
      </div>
      <button class="detail-close" @click="emit('close')" aria-label="关闭详情">&times;</button>
    </div>

    <div class="detail-content">
      <div class="detail-left">
        <StockSummary v-if="quote" :quote="quote" />
        <DepthPanel :code="item.code" :market="item.market" />
      </div>
      <div class="detail-right">
        <ChartSwitcher v-model="activePeriod" />
        <MinuteChart
          v-if="activePeriod === 'minute'"
          :code="item.code"
          :market="item.market"
          :name="item.name"
        />
        <KLineChart
          v-else
          :code="item.code"
          :market="item.market"
          :name="item.name"
          :period="activePeriod"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.stock-detail {
  border-top: 1px solid var(--color-border, rgba(255,255,255,0.08));
  background: var(--color-surface-1);
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
  color: var(--color-text-primary);
}
.detail-code {
  font-size: 12px;
  color: var(--color-text-tertiary);
}
.detail-close {
  background: none;
  border: none;
  color: var(--color-text-tertiary);
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}
.detail-close:hover { color: var(--color-text-primary); }
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
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/detail/StockDetail.vue
git commit -m "feat: add chart period switching to StockDetail"
```

---

### Task 14: Frontend — Update IndexDetail with Chart Switching

**Files:**
- Modify: `src/components/detail/IndexDetail.vue`

- [ ] **Step 1: Add ChartSwitcher and conditional chart rendering to IndexDetail**

Rewrite `src/components/detail/IndexDetail.vue` (the chart section only changes, add imports and state):

Add to script imports:
```typescript
import { ref } from 'vue';
import type { PeriodType } from '@/types';
import KLineChart from './KLineChart.vue';
import ChartSwitcher from './ChartSwitcher.vue';
```

Add state:
```typescript
const activePeriod = ref<PeriodType>('minute');
```

Replace the chart section in template (around line 83):
```vue
      <!-- 全宽图表 -->
      <div class="chart-section">
        <ChartSwitcher v-model="activePeriod" />
        <MinuteChart
          v-if="activePeriod === 'minute'"
          :code="index.code"
          market="CN"
          :name="index.name"
        />
        <KLineChart
          v-else
          :code="index.code"
          market="CN"
          :name="index.name"
          :period="activePeriod"
        />
      </div>
```

- [ ] **Step 2: Verify type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/detail/IndexDetail.vue
git commit -m "feat: add chart period switching to IndexDetail"
```

---

### Task 15: Integration Verification

**Files:** None (verification only)

- [ ] **Step 1: Full frontend type-check**

Run: `npx vue-tsc --noEmit`
Expected: No errors in any files.

- [ ] **Step 2: Full Rust build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully with no errors.

- [ ] **Step 3: Visual inspection**

Run the app with `npm run tauri dev` and verify:
- TopBar shows no brand icon/program name, only data source tag and theme toggle
- StatusBar appears at bottom of main window with version, copyright, contact, QR button
- Click "点击入群" → popover shows QR placeholder
- Click a stock row → detail panel shows ChartSwitcher tabs
- Switch to 日K → candlestick chart loads (may show empty if market closed)
- Switch back to 分时 → area chart loads

- [ ] **Step 4: Commit any fixes if needed**

```bash
git add -A
git commit -m "chore: final integration fixes for UI enhancement"
```
