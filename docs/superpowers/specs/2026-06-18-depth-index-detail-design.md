# 五档盘口修复 + 价格精度自适应 + 指数详情

**日期**: 2026-06-18  
**状态**: Ready for implementation  
**分支**: master

---

## 概述

三个关联改进：
1. 五档盘口颜色/排序修复（买红卖绿）
2. 价格精度自适应（2位/3位小数自动判断）
3. 指数点击查看详情趋势图

---

## 1. 五档盘口颜色与排序修复

**现状问题**：
- DepthPanel 中买盘用 `var(--color-down)`（绿色），卖盘用 `var(--color-up)`（红色）
- A股惯例：买盘红色、卖盘绿色（红涨绿跌，买盘推高价格，卖盘压低价格）
- 买卖盘未排序，按API返回顺序展示

**修复方案**：

| 修改点 | 当前 | 修复后 |
|--------|------|--------|
| 买盘价格颜色 `.bid-price` | `color: var(--color-down)` (绿) | `color: var(--color-up)` (红) |
| 卖盘价格颜色 `.ask-price` | `color: var(--color-up)` (红) | `color: var(--color-down)` (绿) |
| 买盘量柱 `.bid-bar` | `background: var(--color-down)` (绿) | `background: var(--color-up)` (红) |
| 卖盘量柱 `.ask-bar` | `background: var(--color-up)` (红) | `background: var(--color-down)` (绿) |
| 买盘排序 | API原始顺序 | `bids.sort_by(|a,b| b.price.partial_cmp(&a.price))` (高→低) |
| 卖盘排序 | API原始顺序 | `asks.sort_by(|a,b| a.price.partial_cmp(&b.price))` (低→高) |

**文件**: `src/components/detail/DepthPanel.vue`

---

## 2. 价格精度自适应

**现状问题**：所有价格硬编码 `.toFixed(2)`。ETF（价格<1）和可转债需要3位小数。

**方案**：创建共享工具函数 `getPricePrecision(price: number): number`

```typescript
// 策略：检查价格的小数部分判断合适精度
function getPricePrecision(price: number): number {
  if (price === 0 || price == null) return 2;
  const absPrice = Math.abs(price);
  // 如果 price * 100 有显著小数部分，说明第3位有意义
  const remainder = Math.abs(absPrice * 100 - Math.round(absPrice * 100));
  return remainder > 0.001 ? 3 : 2;
}
```

**涉及位置**（所有展示价格的地方）：
- `DepthPanel.vue` — 盘口买卖价格 `.toFixed(precision)`
- `StockSummary.vue` — 摘要（开/高/低/量/额/换手率中的价格）
- `WatchlistTable.vue` — 列表价格列 `render(row)`

**辅助函数放置**：`src/utils/format.ts`（新建），或在组件内定义 computed。

---

## 3. 指数点击查看详情

### 3.1 数据现状

`IndexQuote` 类型（Rust → TS 串行化）：
- `code`, `name`, `price`, `change`, `change_pct`, `volume`, `turnover`
- 不包含 `open`/`high`/`low`（API 不返回这些字段用于指数）

指数列表来自 `quoteStore.indices`（通过 `indices-updated` 事件推送）。

### 3.2 交互流程

```
IndexCard (click) → emit('select', index)
  → IndexBar 管理 selectedIndex 状态
    → 渲染 IndexDetail 面板（下方展开）
    → 同时通知 WatchlistTable 关闭自选股详情（互斥）

WatchlistTable 行点击 → 打开 StockDetail
  → 通知 IndexBar 取消选中指数（互斥）
```

互斥规则：同一时刻最多展示一个详情面板（自选股 OR 指数）。

Toggle 行为：点击已选中的 → 取消选中，关闭详情。

### 3.3 新建组件：IndexDetail.vue

**Props**:
```typescript
interface Props {
  index: IndexQuote;
}
```

**Emits**: `close`

**布局**（方案 C — 卡片网格 + 全宽分时图）：

```
┌─────────────────────────────────────┐
│ 标题栏：指数名称 + 代码        [×]  │
├─────────────────────────────────────┤
│ ┌──────┐ ┌──────┐ ┌──────┐        │
│ │ 最新价│ │ 涨跌额│ │ 涨跌幅│        │
│ │ 3321.8│ │+23.35│ │+0.71%│        │
│ └──────┘ └──────┘ └──────┘        │
│ ┌──────┐ ┌──────┐                  │
│ │ 成交量│ │ 成交额│                  │
│ │1.8亿手│ │2156亿 │                  │
│ └──────┘ └──────┘                  │
├─────────────────────────────────────┤
│                                     │
│         📈 分时走势图（全宽）         │
│                                     │
└─────────────────────────────────────┘
```

摘要卡片使用涨跌色背景：涨绿背景(`color-down-bg`)，跌红背景(`color-up-bg`)，平盘中性背景。

分时图复用 `MinuteChart` 组件，传入 `code` + `market`。指数的 market 固定为 `"CN"`（通过复用 IndexQuote 的 code 直接传给 `get_intraday` 后端）。

### 3.4 修改 IndexCard.vue

- 添加 `@click` 事件 emit
- 添加 `selected` prop 用于高亮样式
- 添加 `cursor: pointer` 和 hover 效果
- 选中状态：边框高亮色 + 背景微升

### 3.5 修改 IndexBar.vue

- 添加 `selectedIndex: IndexQuote | null` 状态
- 向 IndexCard 传递 `selected` 和 `@select`
- 在 IndexBar 下方条件渲染 `IndexDetail`
- 暴露 `clearSelection()` 方法（供 AppLayout/WatchlistTable 调用）

### 3.6 修改 WatchlistTable.vue

- 点击行时通知 IndexBar 清除选中
- 通过 provide/inject 或事件总线实现互斥

**简化方案**：通过 AppLayout 作为中介，用 provide/inject 共享一个 `clearOtherDetail` 回调。

---

## 实现清单

| 文件 | 操作 | 内容 |
|------|------|------|
| `src/utils/format.ts` | 新建 | `getPricePrecision()` 工具函数 |
| `src/components/detail/DepthPanel.vue` | 修改 | 颜色对调 + 排序 + 精度自适应 |
| `src/components/detail/StockSummary.vue` | 修改 | 精度自适应 |
| `src/components/watchlist/WatchlistTable.vue` | 修改 | 精度自适应 + 互斥联动 |
| `src/components/detail/IndexDetail.vue` | 新建 | 指数详情面板 |
| `src/components/index/IndexCard.vue` | 修改 | 点击事件 + 选中高亮 |
| `src/components/index/IndexBar.vue` | 修改 | 选中状态管理 + 渲染详情 |
| `src/components/layout/AppLayout.vue` | 修改 | provide 互斥协调回调 |

---

## 边界情况

- 指数详情打开时切换到无盘口数据的分时图 → 正常展示
- 指数分时图加载失败 → 显示错误+重试按钮（MinuteChart 已有此能力）
- 网络慢/数据延迟 → 展示 loading 状态
- 窗口窄 → 卡片网格自适应缩为2列/1列
- 排序后盘口为空（无数据） → 显示 `--` 占位符

## 不考虑

- 指数 open/high/low（API 不返回）
- 指数盘口数据（不存在）
- 多指数同时选中
