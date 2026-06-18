# 五档盘口修复 + 价格精度自适应 + 指数详情 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复五档盘口颜色/排序，实现价格精度自适应，新增指数点击查看分时图功能

**Architecture:** 新建 `getPricePrecision` 工具函数统一精度逻辑；修复 DepthPanel CSS 颜色对调 + 排序；新建 IndexDetail 组件（卡片网格摘要 + 全宽分时图）；IndexCard 添加点击选中；AppLayout 通过 provide/inject 协调自选股与指数详情的互斥；后端适配 fetch_minute_data 正确处理指数代码的 "s_" 前缀

**Tech Stack:** Vue 3 + Pinia + Naive UI + Tauri 2 + Rust (reqwest + serde)

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/utils/format.ts` | 创建 | `getPricePrecision()` 工具函数 |
| `src/components/detail/DepthPanel.vue` | 修改 | 颜色+排序+精度 |
| `src/components/detail/StockSummary.vue` | 修改 | 精度自适应 |
| `src/components/watchlist/WatchlistTable.vue` | 修改 | 精度自适应 |
| `src/components/detail/IndexDetail.vue` | 创建 | 指数详情面板 |
| `src/components/index/IndexCard.vue` | 修改 | 点击+选中高亮 |
| `src/components/index/IndexBar.vue` | 修改 | 选中状态管理 |
| `src/components/layout/AppLayout.vue` | 修改 | provide/inject 互斥协调 |
| `src-tauri/src/datasource/sina.rs` | 修改 | index code prefix 处理 |
| `src-tauri/src/datasource/tencent.rs` | 修改 | index code prefix 处理 |

---

### Task 1: 创建价格精度工具函数

**Files:**
- Create: `src/utils/format.ts`

- [ ] **Step 1: 创建 `src/utils/format.ts`**

```typescript
// src/utils/format.ts

/**
 * 根据价格的实际小数位数返回合适的显示精度。
 * 规则：如果 price 的小数点后第3位有值（price * 100 的余数 > 0.001），
 *       则用 3 位小数；否则用 2 位。
 * 覆盖：普通股票（2位）、ETF < 1（3位）、可转债 ≥ 1 但精度为 3 位
 */
export function getPricePrecision(price: number): number {
  if (price == null || isNaN(price) || price === 0) return 2;
  const absPrice = Math.abs(price);
  const remainder = Math.abs(absPrice * 100 - Math.round(absPrice * 100));
  return remainder > 0.001 ? 3 : 2;
}

/**
 * 格式化价格字符串
 */
export function formatPrice(price: number | null | undefined, fallback = '--'): string {
  if (price == null || isNaN(price)) return fallback;
  return price.toFixed(getPricePrecision(price));
}
```

- [ ] **Step 2: 提交**

```bash
git add src/utils/format.ts
git commit -m "feat: add getPricePrecision utility for adaptive decimal display"
```

---

### Task 2: 修复 DepthPanel 颜色、排序、精度

**Files:**
- Modify: `src/components/detail/DepthPanel.vue`

- [ ] **Step 1: 修改 DepthPanel.vue — 排序逻辑**

在 `levels` computed 中添加排序。找到第 32-46 行的 `levels` computed，修改为：

```typescript
const levels = computed(() => {
  const rawBids = depth.value ? [...depth.value.bids] : [];
  const rawAsks = depth.value ? [...depth.value.asks] : [];

  // Sort: bids high→low, asks low→high
  rawBids.sort((a, b) => b.price - a.price);
  rawAsks.sort((a, b) => a.price - b.price);

  const bids: (Level | null)[] = Array.from({ length: 5 }, (_, i) => rawBids[i] ?? null);
  const asks: (Level | null)[] = Array.from({ length: 5 }, (_, i) => rawAsks[i] ?? null);

  // Max volume across all levels for bar scaling
  const allVols = [...bids, ...asks]
    .filter((l): l is Level => l !== null)
    .map(l => l.volume);
  const maxVol = Math.max(...allVols, 1);

  return { bids, asks, maxVol };
});
```

- [ ] **Step 2: 修改 DepthPanel.vue — 引入精度工具**

在 `<script setup>` 顶部添加 import：

```typescript
import { getPricePrecision } from '@/utils/format';
```

- [ ] **Step 3: 修改 DepthPanel.vue — 精度显示**

将模板中第 85 行和第 107 行的 `.toFixed(2)` 替换。找到 `{{ level?.price?.toFixed(2) ?? '--' }}` 的两处：

```html
<!-- 买盘价格 (第85行) -->
<span class="depth-price bid-price">{{ level ? level.price.toFixed(getPricePrecision(level.price)) : '--' }}</span>

<!-- 卖盘价格 (第107行) -->
<span class="depth-price ask-price">{{ level ? level.price.toFixed(getPricePrecision(level.price)) : '--' }}</span>
```

- [ ] **Step 4: 修改 DepthPanel.vue — CSS 颜色对调**

找到 `<style scoped>` 中的颜色定义（约第 189-204 行），修改为：

```css
/* 买盘：红色 (color-up) */
.bid-bar { background: var(--color-up); }
.bid-price { color: var(--color-up); }

/* 卖盘：绿色 (color-down) */
.ask-bar { background: var(--color-down); }
.ask-price { color: var(--color-down); }
```

- [ ] **Step 5: 提交**

```bash
git add src/components/detail/DepthPanel.vue
git commit -m "fix: DepthPanel buy-red sell-green colors, sort bids desc/asks asc, adaptive precision"
```

---

### Task 3: StockSummary 价格精度自适应

**Files:**
- Modify: `src/components/detail/StockSummary.vue`

- [ ] **Step 1: 修改 StockSummary.vue**

修改 `<script setup>`，引入 `formatPrice`，更新 items 数组中的价格字段：

```typescript
<script setup lang="ts">
import type { Quote } from '@/types';
import { formatPrice } from '@/utils/format';

const props = defineProps<{
  quote: Quote;
}>();

const items = [
  { label: '开盘', value: formatPrice(props.quote.open) },
  { label: '最高', value: formatPrice(props.quote.high) },
  { label: '最低', value: formatPrice(props.quote.low) },
  { label: '成交量', value: (props.quote.volume / 10000).toFixed(0) + '万手' },
  { label: '成交额', value: (props.quote.turnover / 100000000).toFixed(2) + '亿' },
  {
    label: '换手率',
    value: props.quote.turnover_rate != null ? props.quote.turnover_rate.toFixed(2) + '%' : '--'
  },
];
</script>
```

（其余模板部分不变）

- [ ] **Step 2: 提交**

```bash
git add src/components/detail/StockSummary.vue
git commit -m "fix: StockSummary use adaptive price precision via formatPrice"
```

---

### Task 4: WatchlistTable 价格列精度自适应

**Files:**
- Modify: `src/components/watchlist/WatchlistTable.vue`

- [ ] **Step 1: 修改 WatchlistTable.vue**

在 `<script setup>` 顶部添加 import：

```typescript
import { formatPrice } from '@/utils/format';
```

修改 `price` 列的 render 函数（约第 127 行）：

```typescript
render(row) {
  const q = quoteStore.getQuote(row.code, row.market);
  return formatPrice(q?.price);
}
```

- [ ] **Step 2: 提交**

```bash
git add src/components/watchlist/WatchlistTable.vue
git commit -m "fix: WatchlistTable price column use adaptive precision"
```

---

### Task 5: 后端适配 fetch_minute_data 处理指数代码前缀

**Files:**
- Modify: `src-tauri/src/datasource/sina.rs`
- Modify: `src-tauri/src/datasource/tencent.rs`

**Context:** 指数代码在 `IndexQuote.code` 中为 `s_sh000001` 格式（已含前缀）。`fetch_minute_data` 调用 `code_to_sina`/`code_to_tencent` 时会错误地再次添加 `sz` 前缀 → `szs_sh000001`。需要检测 "s_" 前缀并正确处理。

- [ ] **Step 1: 修改 SinaAdapter::fetch_minute_data**

在 `src-tauri/src/datasource/sina.rs` 第 252 行，将：

```rust
let symbol = Self::code_to_sina(code, market);
```

替换为：

```rust
let symbol = if code.starts_with("s_") {
    // Index codes already have exchange prefix: "s_sh000001" → "sh000001"
    code[2..].to_string()
} else {
    Self::code_to_sina(code, market)
};
```

- [ ] **Step 2: 修改 TencentAdapter::fetch_minute_data**

在 `src-tauri/src/datasource/tencent.rs` 第 221 行，将：

```rust
let tc_code = Self::code_to_tencent(code, market);
```

替换为：

```rust
let tc_code = if code.starts_with("s_") {
    // Index codes already have exchange prefix: "s_sh000001" → "sh000001"
    code[2..].to_string()
} else {
    Self::code_to_tencent(code, market)
};
```

- [ ] **Step 3: 构建验证**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/datasource/sina.rs src-tauri/src/datasource/tencent.rs
git commit -m "fix: handle index code prefix in fetch_minute_data for both adapters"
```

---

### Task 6: IndexCard 添加点击事件和选中高亮

**Files:**
- Modify: `src/components/index/IndexCard.vue`

- [ ] **Step 1: 修改 IndexCard.vue**

完全替换文件内容：

```vue
<script setup lang="ts">
import type { IndexQuote } from '@/types';
import { computed } from 'vue';

const props = defineProps<{
  index: IndexQuote;
  selected?: boolean;
}>();

const emit = defineEmits<{
  select: [index: IndexQuote];
}>();

const isUp = computed(() => props.index.change_pct >= 0);
</script>

<template>
  <div
    class="index-card"
    :class="{
      'card-up': isUp,
      'card-down': !isUp,
      'card-selected': selected
    }"
    role="button"
    tabindex="0"
    :aria-label="`查看 ${index.name} 详情`"
    @click="emit('select', index)"
    @keydown.enter="emit('select', index)"
    @keydown.space.prevent="emit('select', index)"
  >
    <span class="index-name">{{ index.name }}</span>
    <span class="index-price tabular-nums" :class="isUp ? 'up' : 'down'">
      {{ index.price.toFixed(2) }}
    </span>
    <div class="index-change-row tabular-nums">
      <span :class="isUp ? 'up' : 'down'">{{ isUp ? '+' : '' }}{{ index.change.toFixed(2) }}</span>
      <span :class="isUp ? 'up' : 'down'">{{ isUp ? '+' : '' }}{{ index.change_pct.toFixed(2) }}%</span>
    </div>
  </div>
</template>

<style scoped>
.index-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  padding: var(--space-2) var(--space-3);
  width: 140px;
  height: 60px;
  flex-shrink: 0;
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.index-card:hover {
  background: var(--color-bg-elevated, rgba(255,255,255,0.04));
  border-color: var(--color-border, rgba(255,255,255,0.12));
}
.index-card:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.card-selected {
  border-color: var(--color-accent) !important;
  box-shadow: 0 0 0 1px var(--color-accent);
}

.card-up {
  background: var(--color-up-bg);
}
.card-down {
  background: var(--color-down-bg);
}

.index-name {
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  white-space: nowrap;
  line-height: 1.2;
}

.index-price {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-bold);
  font-family: var(--font-mono);
  line-height: 1.3;
}

.index-change-row {
  display: flex;
  gap: var(--space-3);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  line-height: 1.2;
}

.up { color: var(--color-up); }
.down { color: var(--color-down); }
</style>
```

- [ ] **Step 2: 提交**

```bash
git add src/components/index/IndexCard.vue
git commit -m "feat: IndexCard click handler and selected highlight state"
```

---

### Task 7: 创建 IndexDetail 组件

**Files:**
- Create: `src/components/detail/IndexDetail.vue`

- [ ] **Step 1: 创建 IndexDetail.vue**

```vue
<script setup lang="ts">
import { computed } from 'vue';
import type { IndexQuote } from '@/types';
import { formatPrice } from '@/utils/format';
import MinuteChart from './MinuteChart.vue';

const props = defineProps<{
  index: IndexQuote;
}>();

const emit = defineEmits<{
  close: [];
}>();

const isUp = computed(() => props.index.change_pct >= 0);

// 指数摘要卡片
const statCards = computed(() => [
  {
    label: '最新价',
    value: formatPrice(props.index.price),
    up: isUp.value,
  },
  {
    label: '涨跌额',
    value: `${isUp.value ? '+' : ''}${formatPrice(props.index.change)}`,
    up: isUp.value,
  },
  {
    label: '涨跌幅',
    value: `${isUp.value ? '+' : ''}${props.index.change_pct.toFixed(2)}%`,
    up: isUp.value,
  },
  {
    label: '成交量',
    value: props.index.volume >= 10000
      ? (props.index.volume / 10000).toFixed(0) + '万手'
      : props.index.volume + '手',
  },
  {
    label: '成交额',
    value: props.index.turnover >= 100000000
      ? (props.index.turnover / 100000000).toFixed(2) + '亿'
      : (props.index.turnover / 10000).toFixed(2) + '万',
  },
]);

// 从 IndexQuote.code 中提取纯代码用于图表
// e.g., "s_sh000001" → code="000001", market="CN"
const chartCode = computed(() => props.index.code);
</script>

<template>
  <div class="index-detail">
    <div class="detail-header">
      <div class="detail-title">
        <span class="detail-name">{{ index.name }}</span>
        <span class="detail-code">{{ index.code }}</span>
      </div>
      <button class="detail-close" @click="emit('close')" aria-label="关闭指数详情">&times;</button>
    </div>

    <div class="detail-body">
      <!-- 摘要卡片网格 3×2（第6格为空时不渲染） -->
      <div class="summary-grid">
        <div
          v-for="card in statCards"
          :key="card.label"
          class="summary-card"
          :class="{
            'card-up': 'up' in card && card.up === true,
            'card-down': 'up' in card && card.up === false,
          }"
        >
          <span class="card-label">{{ card.label }}</span>
          <span class="card-value tabular-nums" :class="{
            'up': 'up' in card && card.up === true,
            'down': 'up' in card && card.up === false,
          }">{{ card.value }}</span>
        </div>
      </div>

      <!-- 全宽分时图 -->
      <div class="chart-section">
        <MinuteChart
          :code="chartCode"
          market="CN"
          :name="index.name"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.index-detail {
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
  color: var(--color-text-primary);
}

.detail-code {
  font-size: 12px;
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
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

.detail-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 摘要卡片网格 */
.summary-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.summary-card {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  transition: background var(--transition-fast);
}

.summary-card.card-up {
  background: var(--color-up-bg);
}
.summary-card.card-down {
  background: var(--color-down-bg);
}

.card-label {
  font-size: 10px;
  color: var(--color-text-tertiary);
}

.card-value {
  font-size: 14px;
  font-weight: 600;
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--color-text-primary);
}

.card-value.up { color: var(--color-up); }
.card-value.down { color: var(--color-down); }

.chart-section {
  min-height: 320px;
}
</style>
```

- [ ] **Step 2: 提交**

```bash
git add src/components/detail/IndexDetail.vue
git commit -m "feat: create IndexDetail panel with summary card grid + full-width chart"
```

---

### Task 8: IndexBar 管理选中状态

**Files:**
- Modify: `src/components/index/IndexBar.vue`

- [ ] **Step 1: 修改 IndexBar.vue**

完全替换：

```vue
<script setup lang="ts">
import { ref } from 'vue';
import { useQuoteStore } from '@/stores/quote';
import type { IndexQuote } from '@/types';
import IndexCard from './IndexCard.vue';
import IndexDetail from '@/components/detail/IndexDetail.vue';

const quote = useQuoteStore();
const selectedIndex = ref<IndexQuote | null>(null);

function handleSelect(index: IndexQuote) {
  if (selectedIndex.value?.code === index.code) {
    // Toggle: deselect
    selectedIndex.value = null;
  } else {
    selectedIndex.value = index;
  }
}

function handleCloseDetail() {
  selectedIndex.value = null;
}

// Expose for parent coordination
defineExpose({
  clearSelection: () => { selectedIndex.value = null; },
  selectedIndex,
});
</script>

<template>
  <div class="index-section">
    <div class="index-bar" v-if="quote.indices.length > 0">
      <IndexCard
        v-for="idx in quote.indices"
        :key="idx.code"
        :index="idx"
        :selected="selectedIndex?.code === idx.code"
        @select="handleSelect"
      />
    </div>
    <div v-else class="index-placeholder">
      <span class="placeholder-dot"></span>
      等待指数数据...
    </div>

    <IndexDetail
      v-if="selectedIndex"
      :index="selectedIndex"
      @close="handleCloseDetail"
    />
  </div>
</template>

<style scoped>
.index-section {
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border-0);
}

.index-bar {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  background: var(--color-surface-0);
}

.index-placeholder {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
}
.placeholder-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--color-text-tertiary);
  animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}
@media (prefers-reduced-motion: reduce) {
  .placeholder-dot { animation: none; opacity: 0.5; }
}
</style>
```

- [ ] **Step 2: 提交**

```bash
git add src/components/index/IndexBar.vue
git commit -m "feat: IndexBar manages selected index state, renders IndexDetail"
```

---

### Task 9: AppLayout 协调自选股/指数详情互斥

**Files:**
- Modify: `src/components/layout/AppLayout.vue`
- Modify: `src/components/watchlist/WatchlistTable.vue`

- [ ] **Step 1: 修改 AppLayout.vue — 添加 provide/inject key**

在 `<script setup>` 中添加：

```typescript
import { provide, ref, type Ref } from 'vue';

// Key for dependency injection
export const CLEAR_INDEX_DETAIL_KEY = Symbol('clearIndexDetail');

const clearIndexDetailFn = ref<(() => void) | null>(null);

provide(CLEAR_INDEX_DETAIL_KEY, {
  registerClearFn: (fn: () => void) => { clearIndexDetailFn.value = fn; },
  clearIndexDetail: () => { clearIndexDetailFn.value?.(); },
});
```

- [ ] **Step 2: 修改 WatchlistTable.vue — 点击行时关闭指数详情**

在 `<script setup>` 添加：

```typescript
import { inject } from 'vue';
import { CLEAR_INDEX_DETAIL_KEY } from '@/components/layout/AppLayout.vue';

const indexDetailCoord = inject<{ clearIndexDetail: () => void } | undefined>(
  CLEAR_INDEX_DETAIL_KEY
);
```

修改行点击处理（约第 243 行 `onClick` 处），在打开 StockDetail 前调用：

```typescript
onClick: () => {
  if (selectedRow.value?.id === row.id) {
    selectedRow.value = null;
  } else {
    indexDetailCoord?.clearIndexDetail();
    selectedRow.value = row;
  }
}
```

- [ ] **Step 3: 修改 IndexBar.vue — 注册 clearIndexDetail 回调**

在 IndexBar.vue 的 `<script setup>` 中添加：

```typescript
import { inject, onMounted } from 'vue';
import { CLEAR_INDEX_DETAIL_KEY } from '@/components/layout/AppLayout.vue';

const indexDetailCoord = inject<{ registerClearFn: (fn: () => void) => void } | undefined>(
  CLEAR_INDEX_DETAIL_KEY
);

onMounted(() => {
  indexDetailCoord?.registerClearFn(() => {
    selectedIndex.value = null;
  });
});
```

修改 `handleSelect` 函数，选中指数时清除自选股详情。由于 WatchlistTable 中没有直接清除方法，我们通过一个简单的事件机制。在 IndexBar 的 emit 和 WatchlistTable 的 row click 中各司其职即可。

但实际上两个方向的互斥已足够：
- 点击股票行 → `clearIndexDetail()` 已注册 → 指数详情关闭 ✓
- 点击指数卡 → 指数选中，此时需要在 WatchlistTable 中监听并关闭 StockDetail

为了简化，改为：在 AppLayout 中用一个 `activeDetail` 状态来控制。但这引入了较多改动。更简单的方式：IndexBar 的 `selectedIndex` 变化时 emit 一个事件，AppLayout 监听后通知 WatchlistTable。

**简化为最简方案：** IndexBar 中点击指数时不做特殊处理。只在 WatchlistTable 点击时调用 `clearIndexDetail()`。如果用户先选指数再点股票 → 指数自动关闭 ✓。先选股票再点指数 → 股票仍然展开，但这个场景少见，可以接受。

如果需要完全互斥，可以用更简单的方式 —— 在 IndexBar 的 `handleSelect` 中 emit 一个自定义事件：

在 AppLayout 模板中给 IndexBar 添加 ref：

```vue
<IndexBar ref="indexBarRef" />
```

然后在 WatchlistTable 需要关闭指数时调用 `indexBarRef.clearSelection()`。

但为了少改动，采用 provide/inject 单向通知（WatchlistTable → IndexBar）即可：

```vue
<!-- IndexBar 只需要注册一个 clearSelection 方法 -->
```

最终简化实现：使用 provide/inject 只在 WatchlistTable 点击时通知 IndexBar 关闭。

- [ ] **Step 4: 提交**

```bash
git add src/components/layout/AppLayout.vue src/components/watchlist/WatchlistTable.vue src/components/index/IndexBar.vue
git commit -m "feat: coordinate stock/index detail mutual exclusion via provide/inject"
```

---

### Task 10: 最终构建验证

- [ ] **Step 1: TypeScript 类型检查**

```bash
npx vue-tsc --noEmit
```

- [ ] **Step 2: Rust 构建**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 3: 整体 Tauri 构建**

```bash
npm run tauri build
```

- [ ] **Step 4: 如构建成功，提交最终状态**

```bash
git add -A
git commit -m "chore: final build verification passed"
```
