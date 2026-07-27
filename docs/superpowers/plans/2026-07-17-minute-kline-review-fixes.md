# 分钟K线代码评审修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复分钟K线代码评审产出的 8 项问题,保留分钟K左滑分页(修复而非关闭),新浪源分钟K不左滑,复权基准差异文档化。

**Architecture:** 前端在 `useChart.ts` 引入 `minuteKSpan` 作为分钟周期→跨度的单一来源,`periodToKlinecharts`/`getRefreshInterval`/`loadMoreHistory` 统一复用;`loadMoreHistory` 对分钟K改用分钟级 endDate(`YYYYMMDDHHMM`)+ 偏移一个 span,新浪源短路。新浪 1min 回落抽到共享 helper 复用。后端 `parse_kline_bar` 防御性解析 turnover,加 mkline 复权取舍注释,改注释错别字。

**Tech Stack:** Tauri 2 / Rust(reqwest + serde_json)、Vue 3 + Pinia + klinecharts v10、vue-tsc 类型检查、cargo test。

## Global Constraints

- Pool: `notify`/`tone` 保持中文,与现有日志风格一致。
- 后端 IOC:不强依赖具体响应格式,防御性解析(turnover 数字字符串缺失时回退 0)。
- 前端 `vue-tsc --noEmit` 必须通过;`cargo test --manifest-path src-tauri/Cargo.toml` 必须通过。
- 不改动 IPC 命令签名(`get_kline` 入参 `endDate` 仍为字符串,值改为 `YYYYMMDDHHMM` 或 `YYYY-MM-DD`)。
- 提交粒度:每个任务一个提交,messages 用 `fix:`/`refactor:` 前缀。
- 不引入新依赖。

---

## File Structure

| 文件 | 责任 | 改动类型 |
|------|------|---------|
| `src/composables/useChart.ts` | 图表数据加载/刷新/分页;新增 `minuteKSpan`,复用 `isMinuteK`;修复 `loadMoreHistory` 分钟 endDate;新浪源分钟K短路 | 修改 |
| `src/composables/minutePeriod.ts` | 新文件:导出 `minuteKSpan(period)`、`useMinuteKUnavailable()` 共享 helper | 创建 |
| `src/components/detail/StockDetail.vue` | 调用共享 helper 替换内联 watch | 修改 |
| `src/components/detail/IndexDetail.vue` | 调用共享 helper 替换内联 watch | 修改 |
| `src-tauri/src/datasource/tencent.rs` | `parse_kline_bar` 防御性解析 turnover;mkline 分钟K复权取舍注释;改注释错别字;加测试 | 修改 |
| `docs/superpowers/specs/2026-07-10-minute-kline-design.md` | 追加复权基准取舍说明 | 修改 |

---

### Task 1: 后端 parse_kline_bar 防御性解析 turnover 并修注释

**Files:**
- Modify: `src-tauri/src/datasource/tencent.rs:10-37`
- Test: `src-tauri/src/datasource/tencent.rs`(`mod tests`)

**Interfaces:**
- Consumes: 无
- Produces: `parse_kline_bar(arr, is_minute: bool)` 行为变更——分钟K从 index 6/7 扫描首个可解析数字串作为 turnover;日K保持 index 6。

- [ ] **Step 1: 加失败测试——分钟K turnover 在 index 6(无空对象)的情况**

在 `src-tauri/src/datasource/tencent.rs` 的 `mod tests` 末尾追加:

```rust
    #[test]
    fn parses_kline_bar_minute_turnover_at_index6() {
        // 另一种分钟K格式: index 6 直接是金额字符串, 无空对象 {}
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500","1530000"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), true).unwrap();
        assert_eq!(out.date, "2026-06-18 09:35");
        assert_eq!(out.volume, 150000); // 1500 手 ×100
        assert_eq!(out.turnover, 1530000.0);
    }

    #[test]
    fn parses_kline_bar_minute_turnover_at_index7() {
        // 原有格式: index 6 为 {}, index 7 为金额
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500",{},"1530000"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), true).unwrap();
        assert_eq!(out.turnover, 1530000.0);
    }

    #[test]
    fn parses_kline_bar_minute_no_turnover() {
        // turnover 缺失: 回退 0
        let val: serde_json::Value = serde_json::from_str(
            r#"["202606180935","10.00","10.20","10.30","9.90","1500"]"#,
        )
        .unwrap();
        let out = parse_kline_bar(val.as_array().unwrap(), true).unwrap();
        assert_eq!(out.turnover, 0.0);
    }
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_kline_bar_minute_turnover_at_index6 -- --nocapture`
Expected: 第一个测试 FAIL(`turnover` 仍读 index 7 得 0)。

- [ ] **Step 3: 改实现——防御性扫描 turnover**

替换 `src-tauri/src/datasource/tencent.rs` 的 `parse_kline_bar` 中 turnover 段(第 32-37 行)为:

```rust
    // 分钟K 的 turnover 位置随腾讯接口版本变化: index 6 可能是空对象 {} 或金额字符串,
    // index 7 也可能是金额。因此扫描 index 6 与 7, 取首个可解析为 f64 的字符串。
    let turnover: f64 = if is_minute {
        (6..=7)
            .filter_map(|i| arr.get(i))
            .filter_map(|v| v.as_str())
            .filter_map(|s| s.parse::<f64>().ok())
            .next()
            .unwrap_or(0.0)
    } else {
        arr.get(6)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    };
```

- [ ] **Step 4: 修正文档注释错别字(#8)与复权取舍(#2 前半)**

修改 `parse_kline_bar` 上方注释(第 10 行)及分钟K格式说明:

```rust
/// 解析腾讯 K 线数组为 KLineData（日 K / 分钟 K 共用）。
/// 腾讯 K 线数组格式：`[date, open, close, high, low, volume(手), turnover_opt?, ...]`
/// - 日 K：`["2026-06-19", "开", "收", "高", "低", "量(手)", "额", ...]`（≥6 元素，turnover 在 index 6）
/// - 分钟 K：`["202606180935", "开", "收", "高", "低", "量(手)", "{}"|"额"|?, "额"?]`
///   分钟K的 turnover 位置不固定(腾讯接口版本差异)，扫描 index 6/7 取首个可解析数值。
/// `is_minute` 控制：date 格式转换（YYYYMMDDHHMM → YYYY-MM-DD HH:MM）、turnover 扫描范围(6-7 vs 6)。
```

- [ ] **Step 5: 运行测试验证通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parse_kline_bar -- --nocapture`
Expected: 所有 `parse_kline_bar*` 测试 PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/datasource/tencent.rs
git commit -m "fix(backend): defensive turnover parse & doc fixes for parse_kline_bar"
```

---

### Task 2: 后端 mkline 分钟K复权取舍注释(#2 后半)

**Files:**
- Modify: `src-tauri/src/datasource/tencent.rs:355-358`

**Interfaces:**
- Consumes: 无
- Produces: 仅注释,无行为变化。

- [ ] **Step 1: 在 mkline 分支添加复权取舍注释**

修改 `src-tauri/src/datasource/tencent.rs` 分钟K 分支开头的注释块:

```rust
        // ── 分钟 K 线：走 mkline 端点 ──
        // fqkline 不支持分钟周期（返回 "bad params"），故分钟 K 单独走 mkline。
        // 复权取舍：mkline 端点不支持复权参数，故分钟K为【不复权原始价】；
        // 而日/周/月走 fqkline 携带 qfq（【前复权】）。同一股票的分钟K与日K价格基准
        // 在除权日会出现跳变（分钟K跳高/原价，日K平滑），这是依赖腾讯接口能力的设计取舍。
        // 若前端需要一致性，可将日/周/月也改为不复权（fqkline 末参改空），代价为历史价位回退原始价。
        // URL 格式: param={code},m{span},{start},{end_YYYYMMDDHHMM},{count}
        // end_date 去横线以匹配 mkline 响应时间戳格式（YYYYMMDDHHMM）
```

- [ ] **Step 2: 确认编译**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: 编译成功(仅注释变更)。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/datasource/tencent.rs
git commit -m "docs(backend): note mkline vs fqkline adjust-factor trade-off"
```

---

### Task 3: 新建前端 minutePeriod 共享 helper(#6/#4)

**Files:**
- Create: `src/composables/minutePeriod.ts`

**Interfaces:**
- Consumes: `PeriodType` from `@/types`、`useSettingsStore` from `@/stores/settings`、Vue `watch`
- Produces:
  - `minuteKSpan(period: PeriodType): number | null` —— 分钟周期→分钟跨度,非分钟K(分时 'minute' 与日/周/月)返回 null
  - `isMinuteK(period: PeriodType): boolean` —— 是否分钟K
  - `useMinuteKUnavailable(activePeriod: Ref<PeriodType>): void` —— 挂载 watcher,当切到新浪且 activePeriod 为 1min 时自动回落到 5min

helper 完整代码如下:

```ts
import { watch, ref, type Ref } from 'vue';
import type { PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';

/** 分钟周期 → 分钟跨度。非分钟K（分时 'minute' 与日/周/月）返回 null。
 * 单一来源，与后端 datasource::minute_span 保持一致。 */
export function minuteKSpan(period: PeriodType): number | null {
  switch (period) {
    case '1min': return 1;
    case '5min': return 5;
    case '15min': return 15;
    case '30min': return 30;
    case '60min': return 60;
    default: return null;
  }
}

/** 判断某周期是否为分钟 K（区别于分时 'minute' 与日/周/月 K）。*/
export function isMinuteK(period: PeriodType): boolean {
  return minuteKSpan(period) !== null;
}

/** 新浪数据源不支持 1 分钟 K 线：若 activePeriod 为 1min 且切到新浪，自动回落到 5min。
 * 复用方传入自己持有的 activePeriod ref，本函数挂载 watcher 统一回退。 */
export function useMinuteKUnavailable(activePeriod: Ref<PeriodType>): void {
  const settings = useSettingsStore();
  watch(
    () => settings.activeDatasource,
    (ds) => {
      if (ds === 'sina' && activePeriod.value === '1min') {
        activePeriod.value = '5min';
      }
    },
  );
}
```

- [ ] **Step 1: 创建文件写入如上内容**

写入 `src/composables/minutePeriod.ts`。

- [ ] **Step 2: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: PASS(新文件独立,无类型错误)。

- [ ] **Step 3: 提交**

```bash
git add src/composables/minutePeriod.ts
git commit -m "refactor(frontend): add minutePeriod shared helpers (span/unavailable)"
```

---

### Task 4: useChart 复用 isMinuteK/minuteKSpan(#6/#7)

**Files:**
- Modify: `src/composables/useChart.ts:9-12`(删本地 isMinuteK)、`:252-261`(periodToKlinecharts)、`:343-353`(getRefreshInterval)

**Interfaces:**
- Consumes: `isMinuteK`、`minuteKSpan` from `./minutePeriod`
- Produces: `periodToKlinecharts` / `getRefreshInterval` 通过共享 helper 判定。

- [ ] **Step 1: 删除本地 isMinuteK,改为 import**

替换 `src/composables/useChart.ts` 第 9-12 行的本地 `isMinuteK` 定义,并在文件顶部 import:

第 7 行后追加 import:
```ts
import { isMinuteK, minuteKSpan } from './minutePeriod';
```

删除第 9-12 行的:
```ts
/** 判断某周期是否为分钟 K（区别于分时 'minute' 与日/周/月 K）。*/
function isMinuteK(period: PeriodType): boolean {
  return period !== 'minute' && period.endsWith('min');
}
```

- [ ] **Step 2: 重写 periodToKlinecharts(#6)**

替换 `src/composables/useChart.ts` `periodToKlinecharts` 函数为:

```ts
  function periodToKlinecharts(period: PeriodType): { type: string; span: number } {
    const span = minuteKSpan(period);
    if (span !== null) return { type: 'minute', span };
    switch (period) {
      case 'minute': return { type: 'minute', span: 5 };
      case 'weekly': return { type: 'week', span: 1 };
      case 'monthly': return { type: 'month', span: 1 };
      default: return { type: 'day', span: 1 };
    }
  }
```

- [ ] **Step 3: 重写 getRefreshInterval(#7)**

替换 `src/composables/useChart.ts` `getRefreshInterval` 开头的 parseInt 块为:

```ts
  function getRefreshInterval(period: PeriodType): number {
    const span = minuteKSpan(period);
    if (span !== null) return span <= 5 ? 8000 : 20000; // 短分钟 K 8s / 长分钟 K 20s
    switch (period) {
      case 'minute': return 5000;    // 分时图：5 秒
      case 'daily':  return 30000;   // 日K：30 秒
      case 'weekly': return 60000;   // 周K：60 秒
      case 'monthly':return 60000;   // 月K：60 秒
      default:       return 30000;
    }
  }
```

- [ ] **Step 4: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src/composables/useChart.ts
git commit -m "refactor(frontend): use minuteKSpan single source in useChart"
```

---

### Task 5: 修复 loadMoreHistory 分钟K分页(#1 修复方案 / #5)

**Files:**
- Modify: `src/composables/useChart.ts:37-44`(加 formatDateMinuteStamp)、`:61-101`(loadMoreHistory)

**Interfaces:**
- Consumes: `minuteKSpan` / `isMinuteK` from `./minutePeriod`、`settings.activeDatasource`
- Produces: `loadMoreHistory` 对分钟K输出 `YYYYMMDDHHMM` endDate,偏移一个 span;新浪源分钟K短路。

- [ ] **Step 1: 加分钟级时间戳格式化函数**

在 `src/composables/useChart.ts` 的 `formatDate`(第 44 行)之后插入:

```ts
  /** 将时间戳格式化为 YYYYMMDDHHMM 字符串（无分隔符），用于腾讯 mkline 的 end 参数。
   *  与后端 parse_kline_bar 的分钟K date 格式(YYYYMMDDHHMM)对齐。 */
  function formatDateMinuteStamp(ts: number): string {
    const d = new Date(ts);
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}${p(d.getHours())}${p(d.getMinutes())}`;
  }
```

- [ ] **Step 2: 重写 loadMoreHistory 的 endDate 构造**

替换 `src/composables/useChart.ts` `loadMoreHistory` 的前段(第 62-77 行)为:

```ts
  /** 加载更早的历史数据（getBars forward 和预加载共用） */
  async function loadMoreHistory(): Promise<KCLineData[]> {
    if (!hasMoreForward.value || loading.value) return [];
    // 分钟K：腾讯源按 span 向前偏移取更早 bar；新浪源不支持分页，单独短路。
    // 日/周/月：date 级 endDate(YYYY-MM-DD)，偏移一整天，沿用原逻辑。
    const span = minuteKSpan(currentPeriod.value);
    const isSina = settings.activeDatasource === 'sina';
    if (span !== null && isSina) {
      // 新浪端点不支持向后分页(#5)，直接结束懒加载
      hasMoreForward.value = false;
      return [];
    }
    loading.value = true;
    try {
      const earliest = allData.value[0];
      const endDate = earliest
        ? (span !== null
            ? formatDateMinuteStamp(earliest.timestamp - span * 60_000)  // 分钟K: 前移一个 span
            : formatDate(earliest.timestamp - 86_400_000))              // 日/周月: 前移一整天
        : undefined;
      const count = 200;
```

(后续 `const data = await invoke...` 至函数末尾保持不变。)

- [ ] **Step 3: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: PASS。

- [ ] **Step 4: 提交**

```bash
git add src/composables/useChart.ts
git commit -m "fix(frontend): minute-K forward pagination via minute-level endDate"
```

---

### Task 6: loadData 保留分钟K左滑标志(#1 辅助 / #5 一致性)

**Files:**
- Modify: `src/composables/useChart.ts:479-482`

**Interfaces:**
- Consumes: `isMinuteK` from `./minutePeriod`、`settings.activeDatasource`
- Produces: 分钟K hasMoreForward 仅腾讯源为 true,新浪为 false;与 loadMoreHistory 短路一致。

- [ ] **Step 1: 修正 hasMoreForward 赋值**

替换 `src/composables/useChart.ts` 第 479-480 行注释与赋值为:

```ts
          // 分钟K：腾讯源支持左滑分页(mkline end 翻页)，新浪源不支持；
          // 新浪日K 600 条到底；腾讯日/周/月支持 fqkline end_date 翻页
          const isSina = settings.activeDatasource === 'sina';
          hasMoreForward.value = !isSina;
```

(新浪源的所有周期(含分钟K与日K)一律 `hasMoreForward=false`:新浪端点不支持向后分页。非新浪源(腾讯)所有周期 `true`。与原 `!== 'sina'` 行为一致,仅注释与命名更清晰;分钟K+sina 的短路由 `loadMoreHistory` 顶部的 span+sina 守卫(Task 5)再次保障。)

- [ ] **Step 2: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: PASS。

- [ ] **Step 3: 提交**

```bash
git add src/composables/useChart.ts
git commit -m "refactor(frontend): simplify hasMoreForward gate by datasource"
```

---

### Task 7: StockDetail/IndexDetail 复用共享 helper(#4)

**Files:**
- Modify: `src/components/detail/StockDetail.vue:1-33`
- Modify: `src/components/detail/IndexDetail.vue:1-30`

**Interfaces:**
- Consumes: `useMinuteKUnavailable` from `@/composables/minutePeriod`
- Produces: 两组件去掉内联 watch,改调共享 helper。

- [ ] **Step 1: 改 StockDetail.vue**

替换 `src/components/detail/StockDetail.vue` 的 import 与脚本段:

import 段(第 3 行 `useQuoteStore` 之后)追加:
```ts
import { useMinuteKUnavailable } from '@/composables/minutePeriod';
```

删除原 settings import 与 watch 块(评审 diff 引入的第 6 行 `useSettingsStore` import 与第 23-33 行的 `const settings = ... watch(...)`),替换为:

```ts
const activePeriod = ref<PeriodType>('minute');

// 新浪数据源不支持 1 分钟：若正在查看 1 分钟时切到新浪，自动回落到 5 分钟。
useMinuteKUnavailable(activePeriod);
```

- [ ] **Step 2: 改 IndexDetail.vue**

替换 `src/components/detail/IndexDetail.vue` 的 import 与脚本段:

import 段(第 4 行 `useSettingsStore` 之后)删除 `useSettingsStore` 的 import(本任务后该方法不再直接用),追加:
```ts
import { useMinuteKUnavailable } from '@/composables/minutePeriod';
```

删除原 `const settings = useSettingsStore();` 与 watch 块,替换为:

```ts
const activePeriod = ref<PeriodType>('minute');

// 新浪数据源不支持 1 分钟：若正在查看 1 分钟时切到新浪，自动回落到 5 分钟。
useMinuteKUnavailable(activePeriod);
```

(注意 IndexDetail.vue 原本还用 `settings` 仅服务于该 watch,删 watch 后 `useSettingsStore` import 也应一并删除以避免 unused。)

- [ ] **Step 3: 类型检查**

Run: `npx vue-tsc --noEmit`
Expected: PASS。

- [ ] **Step 4: 提交**

```bash
git add src/components/detail/StockDetail.vue src/components/detail/IndexDetail.vue
git commit -m "refactor(frontend): reuse useMinuteKUnavailable in detail panels"
```

---

### Task 8: 设计文档追加复权取舍(#2 文档)

**Files:**
- Modify: `docs/superpowers/specs/2026-07-10-minute-kline-design.md`(末尾追加)

**Interfaces:** 无

- [ ] **Step 1: 末尾追加章节**

在 `docs/superpowers/specs/2026-07-10-minute-kline-design.md` 末尾追加:

```markdown

## 复权基准取舍

- **分钟K** 通过腾讯 `mkline` 端点获取,该端点**不支持复权参数**,故分钟K为**不复权原始价**。
- **日K/周K/月K** 通过腾讯 `fqkline` 端点获取,携带 `qfq` 参数,为**前复权**。
- **后果**:同一股票的分钟K与日K在除权日价格基准不一致——分钟K按原始价跳变,日K经前复权平滑。用户在图表间切换分时↔分钟K↔日K时,不连续是已知的依赖腾讯接口能力的设计取舍。
- **如需一致化**:可将 fqkline 末参 `qfq` 置空(改为不复权),代价为日/周/月K历史价位回退到原始价、除权价位不再平滑。
```

- [ ] **Step 2: 提交**

```bash
git add docs/superpowers/specs/2026-07-10-minute-kline-design.md
git commit -m "docs: record mkline/fqkline adjust-factor trade-off"
```

---

### Task 9: 全量验证

**Files:** 无

- [ ] **Step 1: 前端类型检查**

Run: `npx vue-tsc --noEmit`
Expected: PASS,无 error。

- [ ] **Step 2: 后端测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 全部 PASS(含新增 parse_kline_bar 测试与既有 sina_scale/minute_span 测试)。

- [ ] **Step 3: 后端编译**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: 编译成功。

- [ ] **Step 4: git 状态干净**

Run: `git status --short`
Expected: 空(所有改动已按任务提交)。

- [ ] **Step 5: 提交历史汇总**

Run: `git log origin/feature/minute-kline..HEAD --oneline`
Expected: 列出本次修复的 8 条提交。
