# 分钟级 K 线 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在个股详情图表新增 1/5/15/30/60 分钟 K 线蜡烛图，腾讯支持全部 5 档，新浪隐藏 1 分钟档。

**Architecture:** 后端在两个适配器的 `fetch_kline` 增加分钟分支（腾讯走 `mkline`，新浪走 `getKLineData` 的 `scale` 参数），复用现有 `KLineData` 类型与前端蜡烛渲染管线。前端扩展 `PeriodType`、在 `useChart` 增加分钟周期映射与刷新节奏，切换器用 `NDropdown`「更多」下拉承载分钟档并按数据源过滤。

**Tech Stack:** Rust（rusqlite/reqwest/serde_json/async-trait）、Vue 3 + Pinia + naive-ui + klinecharts v10、Tauri 2。

## Global Constraints

- 成交量归一化：手 → 股 用 `super::VOLUME_HANDS_TO_SHARES`（×100），沿用现有常量，勿新增。
- A 股配色约定：红涨绿跌（`compareRule: 'previous_close'`），勿改现有蜡烛配色。
- 前端无单元测试框架：前端任务的自动化关卡是 `npx vue-tsc --noEmit`；后端用 `cargo test`。
- period 字符串两端一致：`'1min' | '5min' | '15min' | '30min' | '60min'`（腾讯 `m{N}`，新浪 `scale={N}`；新浪无 `1min`）。
- 分钟 K 一次性加载、不做左滑懒加载（`hasMoreForward=false`）。
- 分时（`'minute'`）折线逻辑保持不变，不在本次改动范围内。
- 所有新增前端样式走现有设计 token（`--color-surface-*` / `--color-border-*` / `--color-accent*` / `--radius-*`）。

---

### Task 1: 后端 period→span 映射 + 腾讯分钟 K 线

**Files:**
- Modify: `src-tauri/src/datasource/mod.rs`（新增 `minute_span` 帮助函数 + 测试）
- Modify: `src-tauri/src/datasource/tencent.rs`（新增 `parse_minute_klines` 帮助函数 + `fetch_kline` 分钟分支 + 测试）

**Interfaces:**
- Produces: `datasource::minute_span(period: &str) -> Option<u32>`（`"1min"→1`…`"60min"→60`，其余 `None`）。
- Produces: `datasource::tencent::parse_minute_klines(lines: &[serde_json::Value]) -> Vec<crate::domain::KLineData>`（模块内私有，供测试与分钟分支复用）。
- Consumes: `super::VOLUME_HANDS_TO_SHARES`。

- [ ] **Step 1: 写失败测试（minute_span）**

在 `src-tauri/src/datasource/mod.rs` 末尾（`pub mod ...;` 声明之上）追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_span_maps_known_periods() {
        assert_eq!(minute_span("1min"), Some(1));
        assert_eq!(minute_span("5min"), Some(5));
        assert_eq!(minute_span("15min"), Some(15));
        assert_eq!(minute_span("30min"), Some(30));
        assert_eq!(minute_span("60min"), Some(60));
        assert_eq!(minute_span("daily"), None);
        assert_eq!(minute_span("weekly"), None);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml minute_span_maps_known_periods`
Expected: 编译失败 `cannot find function minute_span`

- [ ] **Step 3: 实现 minute_span**

在 `src-tauri/src/datasource/mod.rs` 的 `normalize_turnover` 函数之后（约 line 62 后）插入：

```rust
/// 将图表周期字符串映射为分钟跨度（仅分钟周期返回 Some）。
/// 用于区分分钟 K 线与日/周/月 K 线。
pub fn minute_span(period: &str) -> Option<u32> {
    match period {
        "1min" => Some(1),
        "5min" => Some(5),
        "15min" => Some(15),
        "30min" => Some(30),
        "60min" => Some(60),
        _ => None,
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml minute_span_maps_known_periods`
Expected: PASS

- [ ] **Step 5: 写失败测试（parse_minute_klines）**

在 `src-tauri/src/datasource/tencent.rs` 末尾追加（若文件已有 `#[cfg(test)] mod tests`，则把该 `#[test]` 并入其中）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tencent_minute_klines() {
        let lines: Vec<serde_json::Value> = serde_json::from_str(
            r#"[["202606180935","10.00","10.20","10.30","9.90","1500",{},"1530000"]]"#,
        )
        .unwrap();
        let out = parse_minute_klines(&lines);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].date, "2026-06-18 09:35");
        assert_eq!(out[0].open, 10.00);
        assert_eq!(out[0].close, 10.20);
        assert_eq!(out[0].high, 10.30);
        assert_eq!(out[0].low, 9.90);
        assert_eq!(out[0].volume, 150000); // 1500 手 ×100
        assert_eq!(out[0].turnover, 1530000.0);
    }
}
```

- [ ] **Step 6: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_tencent_minute_klines`
Expected: 编译失败 `cannot find function parse_minute_klines`

- [ ] **Step 7: 实现 parse_minute_klines**

在 `src-tauri/src/datasource/tencent.rs` 顶层（`impl` 块之外，模块作用域内）新增私有帮助函数：

```rust
/// 解析腾讯 mkline 分钟数据行 `["YYYYMMDDHHMM", 开, 收, 高, 低, 量(手), {}, 额]`
/// 为 KLineData。`date` 格式化为 "YYYY-MM-DD HH:MM"。
/// 注意字段顺序：位置 2 是收盘、位置 3 是最高、位置 4 是最低。
fn parse_minute_klines(lines: &[serde_json::Value]) -> Vec<crate::domain::KLineData> {
    lines
        .iter()
        .filter_map(|pt| {
            let arr = pt.as_array()?;
            if arr.len() < 6 {
                return None;
            }
            let t = arr[0].as_str()?;
            let date = if t.len() >= 12 {
                format!("{}-{}-{} {}:{}", &t[0..4], &t[4..6], &t[6..8], &t[8..10], &t[10..12])
            } else {
                t.to_string()
            };
            let open: f64 = arr[1].as_str()?.parse().ok()?;
            let close: f64 = arr[2].as_str()?.parse().ok()?;
            let high: f64 = arr[3].as_str()?.parse().ok()?;
            let low: f64 = arr[4].as_str()?.parse().ok()?;
            let volume_hands: f64 = arr[5].as_str()?.parse().unwrap_or(0.0);
            let volume: u64 = (volume_hands * super::VOLUME_HANDS_TO_SHARES as f64) as u64;
            let turnover: f64 = arr
                .get(7)
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
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
        .collect()
}
```

- [ ] **Step 8: 运行测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_tencent_minute_klines`
Expected: PASS

- [ ] **Step 9: 在腾讯 fetch_kline 增加分钟分支**

在 `src-tauri/src/datasource/tencent.rs` 的 `fetch_kline` 内，紧接 `tc_code` 计算之后（现有 line 306-310 的 `let tc_code = ...;` 之后、`// Map period to Tencent API parameter` 之前）插入：

```rust
        // 分钟 K 线：走 mkline 接口（返回日内 OHLC 蜡烛），一次性取约 320 根。
        if let Some(span) = super::minute_span(period) {
            let cnt = count.unwrap_or(320);
            let url = format!(
                "http://ifzq.gtimg.cn/appstock/app/kline/mkline?param={},m{},,{}",
                tc_code, span, cnt
            );
            let resp = headers::with_browser_headers(self.client.get(&url), "https://gu.qq.com")
                .send()
                .await
                .map_err(|e| AppError::network("tencent", format!("分钟K线请求失败: {:#}", e)))?;
            if !resp.status().is_success() {
                return Err(AppError::network("tencent", format!("分钟K线 HTTP {}", resp.status())));
            }
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| AppError::network("tencent", format!("分钟K线解析失败: {}", e)))?;
            let lines = body
                .pointer("/data")
                .and_then(|d| d.as_object())
                .and_then(|obj| obj.values().next())
                .and_then(|stock| stock.get(format!("m{}", span).as_str()))
                .and_then(|arr| arr.as_array())
                .cloned()
                .unwrap_or_default();
            if lines.is_empty() {
                log::warn!("Tencent minute kline empty for code={} span={}", tc_code, span);
            }
            return Ok(parse_minute_klines(&lines));
        }
```

- [ ] **Step 10: 编译确认后端无误**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功（无错误）

- [ ] **Step 11: 提交**

```bash
git add src-tauri/src/datasource/mod.rs src-tauri/src/datasource/tencent.rs
git commit -m "feat(backend): tencent minute K-line via mkline endpoint"
```

---

### Task 2: 后端新浪分钟 K 线 + 1 分钟拒绝

**Files:**
- Modify: `src-tauri/src/datasource/sina.rs`（新增 `sina_scale` 帮助函数替换周期守卫 + 测试）

**Interfaces:**
- Produces: `sina_scale(period: &str) -> Result<u32, AppError>`（`daily→240`、`5min→5`…`60min→60`；`1min`/`weekly`/`monthly` → `Err(AppError::Unsupported(..))`）。
- Consumes: 现有 `AppError::Unsupported`。

- [ ] **Step 1: 写失败测试（sina_scale）**

在 `src-tauri/src/datasource/sina.rs` 末尾追加（若已有 tests 模块则并入）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sina_scale_maps_supported_periods() {
        assert_eq!(sina_scale("daily").unwrap(), 240);
        assert_eq!(sina_scale("5min").unwrap(), 5);
        assert_eq!(sina_scale("15min").unwrap(), 15);
        assert_eq!(sina_scale("30min").unwrap(), 30);
        assert_eq!(sina_scale("60min").unwrap(), 60);
    }

    #[test]
    fn sina_scale_rejects_unsupported_periods() {
        assert!(sina_scale("1min").is_err());
        assert!(sina_scale("weekly").is_err());
        assert!(sina_scale("monthly").is_err());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sina_scale`
Expected: 编译失败 `cannot find function sina_scale`

- [ ] **Step 3: 实现 sina_scale**

在 `src-tauri/src/datasource/sina.rs` 顶层（`impl` 块之外）新增：

```rust
/// 将图表周期映射为新浪 getKLineData 的 `scale` 参数（分钟数；日 K = 240）。
/// 新浪不支持 1 分钟与周/月 K，返回 Unsupported 错误。
fn sina_scale(period: &str) -> Result<u32, AppError> {
    match period {
        "daily" => Ok(240),
        "5min" => Ok(5),
        "15min" => Ok(15),
        "30min" => Ok(30),
        "60min" => Ok(60),
        "1min" => Err(AppError::Unsupported(
            "新浪数据源不支持1分钟K线，请切换到腾讯数据源查看".into(),
        )),
        _ => Err(AppError::Unsupported(
            "新浪数据源不支持周K/月K，请切换到腾讯数据源查看".into(),
        )),
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sina_scale`
Expected: PASS（两个测试均通过）

- [ ] **Step 5: 在 fetch_kline 中改用 sina_scale**

在 `src-tauri/src/datasource/sina.rs` 的 `fetch_kline` 内，把现有的周期守卫与硬编码 scale（现有 line 392-402）：

```rust
        // Sina only supports daily K-line; reject minute/weekly/monthly.
        // Minute data should use fetch_minute_data instead.
        if period != "daily" {
            return Err(AppError::Unsupported("新浪数据源不支持周K/月K/分钟K线，请切换到腾讯数据源查看".into()));
        }

        if end_date.is_some() || count.is_some() {
            log::debug!("Sina adapter does not support end_date/count pagination; ignoring");
        }

        let scale = "240";
```

替换为：

```rust
        // 根据周期计算 scale（分钟数；日 K = 240）。1 分钟/周/月由 sina_scale 拒绝。
        let scale = sina_scale(period)?;

        if end_date.is_some() || count.is_some() {
            log::debug!("Sina adapter does not support end_date/count pagination; ignoring");
        }
```

（`scale` 由 `&str` 变为 `u32`；下方 URL 的 `format!("...&scale={}...", symbol, scale)` 与 warn 日志的 `scale` 均兼容 `u32`，无需改动。）

- [ ] **Step 6: 编译确认后端无误**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功

- [ ] **Step 7: 提交**

```bash
git add src-tauri/src/datasource/sina.rs
git commit -m "feat(backend): sina minute K-line via scale param, reject 1min"
```

---

### Task 3: 前端 PeriodType 扩展 + useChart 分钟支持

**Files:**
- Modify: `src/types/index.ts`（扩展 `PeriodType`）
- Modify: `src/composables/useChart.ts`（`periodToKlinecharts` / `getRefreshInterval` / `loadData` 的 `hasMoreForward` / 蜡烛 tooltip 标签）

**Interfaces:**
- Produces: `PeriodType` 扩展类型，供 Task 4/5 使用。
- Consumes: Task 1/2 的后端 `get_kline` 分钟周期能力。

- [ ] **Step 1: 扩展 PeriodType**

在 `src/types/index.ts` 把（现有 line 74）：

```ts
export type PeriodType = 'minute' | 'daily' | 'weekly' | 'monthly';
```

替换为：

```ts
export type PeriodType =
  | 'minute'
  | '1min'
  | '5min'
  | '15min'
  | '30min'
  | '60min'
  | 'daily'
  | 'weekly'
  | 'monthly';
```

- [ ] **Step 2: 更新 periodToKlinecharts**

在 `src/composables/useChart.ts` 把现有 `periodToKlinecharts`（现有 line 242-249）替换为：

```ts
  function periodToKlinecharts(period: PeriodType): { type: string; span: number } {
    switch (period) {
      case 'minute': return { type: 'minute', span: 5 };
      case '1min': return { type: 'minute', span: 1 };
      case '5min': return { type: 'minute', span: 5 };
      case '15min': return { type: 'minute', span: 15 };
      case '30min': return { type: 'minute', span: 30 };
      case '60min': return { type: 'minute', span: 60 };
      case 'weekly': return { type: 'week', span: 1 };
      case 'monthly': return { type: 'month', span: 1 };
      default: return { type: 'day', span: 1 };
    }
  }
```

- [ ] **Step 3: 更新 getRefreshInterval**

在 `src/composables/useChart.ts` 把现有 `getRefreshInterval`（现有 line 332-340）替换为：

```ts
  function getRefreshInterval(period: PeriodType): number {
    switch (period) {
      case 'minute': return 5000;    // 分时图：5 秒
      case '1min':
      case '5min':   return 8000;    // 短分钟 K：8 秒
      case '15min':
      case '30min':
      case '60min':  return 20000;   // 长分钟 K：20 秒
      case 'daily':  return 30000;   // 日K：30 秒
      case 'weekly': return 60000;   // 周K：60 秒
      case 'monthly':return 60000;   // 月K：60 秒
      default:       return 30000;
    }
  }
```

- [ ] **Step 4: 分钟 K 关闭懒加载**

在 `src/composables/useChart.ts` 的 `loadData` 的 `else` 分支里，把现有（现有 line 466-467）：

```ts
          // 新浪日K 600 条一次性加载到底，腾讯支持分页懒加载
          hasMoreForward.value = settings.activeDatasource !== 'sina';
```

替换为：

```ts
          // 分钟 K 一次性加载，不做左滑懒加载；新浪日K 600 条到底；腾讯日/周/月支持懒加载
          const isMinuteK = ['1min', '5min', '15min', '30min', '60min'].includes(period);
          hasMoreForward.value = !isMinuteK && settings.activeDatasource !== 'sina';
```

- [ ] **Step 5: 分钟 K 蜡烛 tooltip 首列标签用「时间」**

在 `src/composables/useChart.ts` 的 `applyCandlestickStyles` 内，函数开头 `const c = themeColors();` 之后加一行：

```ts
    const dateLabel = ['1min', '5min', '15min', '30min', '60min'].includes(currentPeriod.value)
      ? '时间'
      : '日期';
```

并把该函数内蜡烛 tooltip 的 `labels: ['日期', '开', '高', '低', '收', '量', '额'],`（现有 line 221）改为：

```ts
          labels: [dateLabel, '开', '高', '低', '收', '量', '额'],
```

- [ ] **Step 6: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 7: 提交**

```bash
git add src/types/index.ts src/composables/useChart.ts
git commit -m "feat(frontend): extend PeriodType and useChart for minute K-line"
```

---

### Task 4: 前端切源兜底（新浪回落 1 分钟）

**Files:**
- Modify: `src/components/detail/StockDetail.vue`（监听 datasource 变化，1min→5min 回落）

**Interfaces:**
- Consumes: Task 3 的 `PeriodType`；`useSettingsStore().activeDatasource`。
- 说明：路由无需改动——模板现有 `MinuteChart v-if="activePeriod === 'minute'"` / `KLineChart v-else`，分钟 K 周期已由 `v-else` 命中 KLineChart。

- [ ] **Step 1: 引入 settings store 与 watch**

在 `src/components/detail/StockDetail.vue` 的 `<script setup>` 顶部，把现有（现有 line 2）：

```ts
import { ref, computed } from 'vue';
```

改为：

```ts
import { ref, computed, watch } from 'vue';
```

并在现有 `import { useQuoteStore } from '@/stores/quote';` 下一行新增：

```ts
import { useSettingsStore } from '@/stores/settings';
```

- [ ] **Step 2: 新增回落逻辑**

在 `src/components/detail/StockDetail.vue` 的 `const activePeriod = ref<PeriodType>('minute');`（现有 line 22）之后新增：

```ts
const settings = useSettingsStore();

// 新浪数据源不支持 1 分钟：若正在查看 1 分钟时切到新浪，自动回落到 5 分钟。
watch(
  () => settings.activeDatasource,
  (ds) => {
    if (ds === 'sina' && activePeriod.value === '1min') {
      activePeriod.value = '5min';
    }
  }
);
```

- [ ] **Step 3: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: 提交**

```bash
git add src/components/detail/StockDetail.vue
git commit -m "feat(frontend): fall back 1min to 5min when switching to sina"
```

---

### Task 5: 切换器「更多」下拉（UI，用 ui-ux-pro-max）

**Files:**
- Modify: `src/components/detail/ChartSwitcher.vue`（主行 4 tab + 「更多」`NDropdown` 分钟档，按数据源过滤 1 分钟，激活态高亮）

**Interfaces:**
- Consumes: Task 3 的 `PeriodType`；`useSettingsStore().activeDatasource`；`NDropdown`（已在 [TopBar.vue](../../../src/components/layout/TopBar.vue) 使用的 `options`+`@select`+触发插槽模式）。
- Emits: `update:modelValue` 携带分钟周期（`'1min'`…`'60min'`）或主 tab 周期。

- [ ] **Step 1: 用 ui-ux-pro-max 打磨视觉**

实现本任务前，先调用 `ui-ux-pro-max` skill，以本任务下方基线代码为功能起点，产出下拉菜单项样式、选中打勾、「更多」激活态高亮、深浅色适配的最终视觉，全部使用现有设计 token。

- [ ] **Step 2: 替换 ChartSwitcher 实现（功能基线）**

把 `src/components/detail/ChartSwitcher.vue` 全文替换为：

```vue
<script setup lang="ts">
import { computed } from 'vue';
import type { PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { NDropdown } from 'naive-ui';

const props = defineProps<{
  modelValue: PeriodType;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: PeriodType];
}>();

const settings = useSettingsStore();

const tabs: { key: PeriodType; label: string }[] = [
  { key: 'minute', label: '分时' },
  { key: 'daily', label: '日K' },
  { key: 'weekly', label: '周K' },
  { key: 'monthly', label: '月K' },
];

const minuteOptions: { key: PeriodType; label: string }[] = [
  { key: '1min', label: '1分' },
  { key: '5min', label: '5分' },
  { key: '15min', label: '15分' },
  { key: '30min', label: '30分' },
  { key: '60min', label: '60分' },
];

// 新浪无 1 分钟：过滤掉 1min 档
const availableMinutes = computed(() =>
  settings.activeDatasource === 'sina'
    ? minuteOptions.filter((o) => o.key !== '1min')
    : minuteOptions
);

const dropdownOptions = computed(() =>
  availableMinutes.value.map((o) => ({
    label: props.modelValue === o.key ? `✓ ${o.label}` : o.label,
    key: o.key,
  }))
);

const isMinuteActive = computed(() =>
  minuteOptions.some((o) => o.key === props.modelValue)
);

// 选中某分钟周期时「更多」按钮显示该周期，否则显示「更多」
const moreLabel = computed(() => {
  const found = minuteOptions.find((o) => o.key === props.modelValue);
  return found ? found.label : '更多';
});

function handleMinuteSelect(key: string) {
  emit('update:modelValue', key as PeriodType);
}
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

    <n-dropdown
      trigger="click"
      :options="dropdownOptions"
      @select="handleMinuteSelect"
    >
      <button
        class="switcher-tab switcher-more"
        :class="{ active: isMinuteActive }"
        :aria-selected="isMinuteActive"
        aria-haspopup="menu"
      >
        {{ moreLabel }}<span class="more-caret" aria-hidden="true">▾</span>
      </button>
    </n-dropdown>
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
.switcher-more {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
.more-caret {
  font-size: 9px;
  line-height: 1;
}
</style>
```

- [ ] **Step 3: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 4: 提交**

```bash
git add src/components/detail/ChartSwitcher.vue
git commit -m "feat(frontend): minute K-line dropdown in chart switcher"
```

---

### Task 6: 端到端手动验证

**Files:** 无（运行验证）

- [ ] **Step 1: 启动应用**

Run: `npm run tauri dev`

- [ ] **Step 2: 腾讯数据源验证**

在顶栏确认数据源为「腾讯」。展开任一自选股详情，点击「更多」下拉，依次选择 1/5/15/30/60 分：
- 每档均渲染蜡烛图 + MA + VOL；
- 「更多」按钮文字变为当前档（如「5分」）并高亮；
- tooltip 首列显示「时间」且为 `MM-DD HH:MM` 之类的分钟时间。

- [ ] **Step 3: 新浪数据源验证**

顶栏切换到「新浪」：
- 「更多」下拉**不出现 1 分**，仅 5/15/30/60；
- 选 5/15/30/60 均正常渲染分钟蜡烛。

- [ ] **Step 4: 切源兜底验证**

腾讯下选中「1分」→ 顶栏切到「新浪」→ 图表自动回落到「5分」（「更多」显示 5分，不停留在 1 分）。

- [ ] **Step 5: 自动刷新验证**

停在某分钟 K（如 5 分），观察最后一根蜡烛随行情更新且不整屏抖动（交易时段内）。

- [ ] **Step 6: 回归确认**

分时 / 日K / 周K / 月K 四档仍正常；ETF 等 3 位小数品种分钟 K 价格轴精度正确。

---

## Self-Review

**Spec coverage：**
- 分钟 K 5 档 → Task 1（腾讯）+ Task 2（新浪）+ Task 3（前端周期映射）+ Task 5（选择入口）。✅
- 新浪隐藏 1 分钟 → Task 5（菜单过滤）+ Task 2（后端拒绝）+ Task 4（切源兜底）。✅
- 蜡烛渲染 + MA + VOL → 复用现有 `KLineChart`/`useChart`（Task 3 周期映射命中蜡烛分支）。✅
- 一次性加载不懒加载 → Task 3 Step 4（`hasMoreForward=false`）。✅
- 刷新节奏 → Task 3 Step 3。✅
- 换手率缺失置 0 → Task 1（`arr.get(7)` 兜底）/ Task 2（新浪 turnover=0 沿用）。✅
- 分时逻辑不动 → 无任务触碰 `fetch_minute_data` / `MinuteChart`。✅

**Placeholder scan：** 无 TBD/TODO；每个代码步骤均含完整代码。✅

**Type consistency：** period 字符串 `'1min'..'60min'` 在 `minute_span`（Rust）、`sina_scale`（Rust）、`periodToKlinecharts`/`getRefreshInterval`/`isMinuteK`（TS）、`minuteOptions`（TS）中一致；`parse_minute_klines` 返回 `Vec<KLineData>` 与 `fetch_kline` 返回类型一致。✅
