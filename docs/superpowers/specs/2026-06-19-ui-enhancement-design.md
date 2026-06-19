# UI Enhancement Design: TopBar Simplification + Chart Switching + Status Bar

**Date:** 2026-06-19
**Version:** 0.5.2 → 0.5.3
**Status:** Approved

## Overview

Three UI enhancements to improve the QuantDesktop application:

1. **TopBar simplification** — Remove redundant brand icon and program name (window title bar already shows it)
2. **Chart period switching** — Add tabs above chart to switch between minute chart, daily/weekly/monthly K-line
3. **Global status bar** — Add footer bar with version, copyright, business contact, and WeChat group QR code

---

## 1. TopBar Simplification

### Current State

[TopBar.vue](../../../src/components/layout/TopBar.vue) displays:
- `.brand` section: SVG icon (K-line concept) + "QuantDesktop" text
- `.ds-tag`: data source dropdown (Sina/Tencent)
- Theme toggle button

### Change

Remove the `.brand` div entirely (lines 28-44 in template, lines 112-125 in styles). The data source dropdown becomes the first item on the left. Theme toggle stays on the right.

### Rationale

The window title bar (native decoration) already displays "QuantDesktop". Showing it again in-content is redundant and wastes vertical space.

### Affected Files

| File | Change |
|------|--------|
| `src/components/layout/TopBar.vue` | Remove `.brand` template block and associated CSS |

---

## 2. Chart Period Switching

### Current State

[MinuteChart.vue](../../../src/components/detail/MinuteChart.vue) is a self-contained component that:
- Initializes `klinecharts` chart instance directly
- Calls `invoke('get_intraday')` to fetch minute data
- Handles its own lifecycle (init, dispose, theme change)
- Shows only 5-minute intraday data as an area chart

Both [StockDetail.vue](../../../src/components/detail/StockDetail.vue) and [IndexDetail.vue](../../../src/components/detail/IndexDetail.vue) use MinuteChart.

### Design

#### Component Architecture

```
detail/
├── ChartSwitcher.vue    ← NEW: Tab bar (分时 | 日K | 周K | 月K)
├── MinuteChart.vue       ← REFACTOR: Use useChart composable
├── KLineChart.vue        ← NEW: Candlestick chart for daily/weekly/monthly
├── StockDetail.vue       ← MODIFY: Add ChartSwitcher, conditional chart rendering
├── IndexDetail.vue       ← MODIFY: Same as StockDetail
├── StockSummary.vue      ← UNCHANGED
└── DepthPanel.vue        ← UNCHANGED
```

#### New Composable: `useChart.ts`

Extract shared klinecharts logic into `src/composables/useChart.ts`:

```typescript
export function useChart(options: {
  chartRef: Ref<HTMLElement | null>
  code: MaybeRef<string>
  market: MaybeRef<string>
  name?: MaybeRef<string>
}) {
  // Returns:
  // - chart: Chart | null
  // - loading: Ref<boolean>
  // - error: Ref<string>
  // - initChart(period: PeriodType): Promise<void>
  // - loadData(period: PeriodType): Promise<void>
  // - disposeChart(): void
  // - applyTheme(): void
}
```

Responsibilities:
- klinecharts `init()` / `dispose()` lifecycle
- Data loading via `invoke('get_intraday')` or `invoke('get_kline')`
- Theme-aware style application
- Abort controller management for stale requests

#### ChartSwitcher Component

Tab-style selector with 4 options. Props:

```typescript
defineProps<{
  modelValue: 'minute' | 'daily' | 'weekly' | 'monthly'
}>()
defineEmits<{ 'update:modelValue': [value: string] }>()
```

Renders 4 tab buttons. Active tab uses accent background (`--color-accent-dim`) matching the data source tag style.

#### MinuteChart (Refactored)

Strips out all klinecharts init/dispose logic. Uses `useChart` composable. Accepts `period: 'minute'` and renders area chart.

#### KLineChart (New)

Similar to MinuteChart but for K-line periods. Uses `useChart` composable. Renders candlestick chart with:
- `candle.type: 'candle_solid'`
- OHLC data format
- MA indicators (MA5, MA10, MA20) on by default
- Volume sub-chart

#### Backend: New `KLineData` Type

Add to `src-tauri/src/domain/mod.rs`:

```rust
pub struct KLineData {
    pub date: String,       // "2026-06-19"
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub turnover: f64,
}
```

#### Backend: DataSource Trait Extension

Add to `src-tauri/src/datasource/mod.rs`:

```rust
async fn fetch_kline(
    &self, code: &str, market: &str, period: &str,
) -> Result<Vec<crate::domain::KLineData>, String> {
    // default: not implemented
    Err("not supported".into())
}
```

Where `period` is `"daily"`, `"weekly"`, or `"monthly"`.

#### Backend: Sina Adapter

Sina daily K-line API:
`https://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData?symbol=sh600519&scale=240&ma=no&datalen=200`

Parameters:
- `scale`: `240` (daily), `120` (weekly — approximate via daily aggregation), `60` (monthly)
- `datalen`: number of bars to fetch (default 200)

#### Backend: Tencent Adapter

Tencent daily K-line API:
`http://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sh600519,day,,,200,qfq`

Parameters:
- Period type: `day`, `week`, `month`
- Last param: `qfq` (前复权)

#### Backend: New Tauri Command

Add to `src-tauri/src/commands/quote.rs`:

```rust
#[tauri::command]
pub async fn get_kline(
    code: String, market: String, period: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<KLineData>, String> {
    let source = state.datasource_manager.active();
    source.fetch_kline(&code, &market, &period).await
}
```

Register in `lib.rs`.

### Affected Files (Chart Switching)

| File | Change |
|------|--------|
| `src/composables/useChart.ts` | **NEW** — shared chart composable |
| `src/components/detail/ChartSwitcher.vue` | **NEW** — tab selector |
| `src/components/detail/KLineChart.vue` | **NEW** — K-line candlestick chart |
| `src/components/detail/MinuteChart.vue` | **REFACTOR** — use composable |
| `src/components/detail/StockDetail.vue` | **MODIFY** — add ChartSwitcher + conditional chart |
| `src/components/detail/IndexDetail.vue` | **MODIFY** — same as StockDetail |
| `src/types/index.ts` | **MODIFY** — add KLineData, PeriodType |
| `src-tauri/src/domain/mod.rs` | **MODIFY** — add KLineData struct |
| `src-tauri/src/datasource/mod.rs` | **MODIFY** — add fetch_kline to trait |
| `src-tauri/src/datasource/sina.rs` | **MODIFY** — implement fetch_kline |
| `src-tauri/src/datasource/tencent.rs` | **MODIFY** — implement fetch_kline |
| `src-tauri/src/commands/quote.rs` | **MODIFY** — add get_kline command |
| `src-tauri/src/lib.rs` | **MODIFY** — register get_kline command |

---

## 3. Global Status Bar

### Design

New `StatusBar.vue` component placed at the bottom of `AppLayout.vue`, above any error/warning banners.

### Layout

```
+------------------------------------------------------------------+
|  v0.5.3  |  © 2026 QuantDesktop     📧 biz@example.com  💬 点击入群 |
+------------------------------------------------------------------+
   左区域                          右区域
   (版本 + 版权)                   (商务合作 + 入群按钮)
```

### StatusBar Component

Props (all optional with defaults for placeholders):

```typescript
defineProps<{
  version?: string      // default: "0.5.3"
  copyright?: string    // default: "© 2026 QuantDesktop"
  contactEmail?: string // default: "biz@example.com"
  qrcodeSrc?: string    // default: placeholder image path
}>()
```

### WeChat QR Code

- "点击入群" button: small green button, matches the "添加" button style
- Click triggers Naive UI `NPopover` with QR code image
- QR code image stored in `src/assets/` directory
- Placeholder: a simple generated placeholder until real QR code is provided
- Popover trigger: click, placement: top

### AppLayout Integration

StatusBar renders at the bottom of `.app-layout`, after `main-content` and before any banners. It is `flex-shrink: 0` and does not scroll.

```
AppLayout.vue
├── error-banner (if error)
├── warning-banner (if warning)
├── TopBar
├── IndexBar
├── main-content (WatchlistTable + Detail panels)
└── StatusBar    ← NEW: at the very bottom
```

### Affected Files (Status Bar)

| File | Change |
|------|--------|
| `src/components/layout/StatusBar.vue` | **NEW** — footer bar component |
| `src/components/layout/AppLayout.vue` | **MODIFY** — add StatusBar to layout |
| `src/assets/qrcode-placeholder.png` | **NEW** — placeholder QR code |

---

## Visual Design Tokens

StatusBar uses existing design system tokens:
- Background: `var(--color-surface-1)` or slightly darker
- Border-top: `1px solid var(--color-border-0)`
- Font size: `11px` / `var(--text-xs)`
- Height: approximately 28px (matching the compact ticker bar style)

ChartSwitcher tabs use:
- Active: `background: var(--color-accent-dim)`, `color: var(--color-accent)`
- Inactive: transparent, `color: var(--color-text-tertiary)`
- Hover: `color: var(--color-text-secondary)`

---

## Implementation Order

1. **Phase 1**: TopBar simplification (simplest, immediate win)
2. **Phase 2**: StatusBar + AppLayout integration
3. **Phase 3**: Backend K-line support (domain, trait, adapters, command)
4. **Phase 4**: Frontend chart switching (composable, ChartSwitcher, MinuteChart refactor, KLineChart)
5. **Phase 5**: Integration testing and polish

---

## Out of Scope

- HK/US market K-line data (Phase 5 enhancement)
- Technical indicators beyond default MA (planned Phase 4)
- Status bar auto-hide or customization settings
- QR code image generation (user provides the image)
