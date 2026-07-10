# 分钟级 K 线设计文档

日期：2026-07-10
状态：已批准，待实现

## 目标

在个股详情图表中新增**分钟级 K 线**（1/5/15/30/60 分钟蜡烛图），并按数据源区分能力：**新浪数据源下不显示 1 分钟档**。腾讯支持全部 5 档，新浪支持 5/15/30/60（无 1 分钟）。

## 背景与现状

- `PeriodType = 'minute' | 'daily' | 'weekly' | 'monthly'`。
- 切换器 [ChartSwitcher.vue](../../../src/components/detail/ChartSwitcher.vue) 是 4 个平铺 tab：分时 / 日K / 周K / 月K。
- 分时（`'minute'`）走 `get_intraday` → `fetch_minute_data`（硬编码 m5 / scale=5），渲染为**折线/面积图**。
- 日/周/月走 `get_kline` → `fetch_kline`，渲染为**蜡烛图**（含 MA + VOL）。
- 数据源能力：
  - 腾讯 `ifzq.gtimg.cn/appstock/app/kline/mkline?param={code},m{N},,{count}`，`N ∈ {1,5,15,30,60}`。
  - 新浪 `money.finance.sina.com.cn/.../getKLineData?symbol={code}&scale={N}`，`N ∈ {5,15,30,60}`（无 1 分钟；日 K 用 scale=240）。

分钟接口返回的都是每根 bar 的 OHLC + volume，是真正的 K 线蜡烛数据。

## 设计

### 1. 切换器 UX（用 `/ui-ux-pro-max` 设计实现）

主行保持 5 个位置，末位为「更多」下拉：

```
│ 分时 │ 日K │ 周K │ 月K │ 更多 ▾ │      未选分钟时
│ 分时 │ 日K │ 周K │ 月K │ 5分 ▾  │      选中某分钟周期后（高亮）
```

- 用 naive-ui `NDropdown` 承载分钟菜单：`1分 / 5分 / 15分 / 30分 / 60分`。
- **激活态**：选中某分钟周期后，「更多」按钮文字变为该周期（如「5分」）并高亮（复用现有 `.active` 的 accent 样式）；切回日/周/月后复位为「更多」，去高亮。
- 下拉当前选中项打勾。
- 样式全部走现有设计 token（surface / border / accent / radius），深浅色自适应。
- 无障碍：主行 tab 保持现有 `role=tab` / `aria-selected`；「更多」按钮标注 `aria-haspopup` / `aria-expanded`。

### 2. 数据源区分（核心）

三层保证「新浪下不显示 1 分钟」且无死角：

1. **前端菜单过滤**：`settings.activeDatasource === 'sina'` 时，下拉隐藏 `1分`，仅显示 `5/15/30/60分`。
2. **切源兜底**：用户正查看 `1min` 时把数据源切到新浪，自动回落到 `5min`（在详情组件监听 `settings.activeDatasource` 变化；仅当当前 period 为 `1min` 且新源为 sina 时触发）。
3. **后端防御**：新浪 `fetch_kline` 收到 `1min` 返回现有 `AppError::Unsupported`（「请切换到腾讯数据源查看」），前端错误浮层已可兜底。

### 3. 前端类型与路由

- `PeriodType` 扩展为：
  ```ts
  'minute' | '1min' | '5min' | '15min' | '30min' | '60min' | 'daily' | 'weekly' | 'monthly'
  ```
  - `'minute'`：分时折线图（`fetch_minute_data`，**保持原样不动**）。
  - 新增 5 个：分钟 K 线蜡烛图，走 `get_kline`。
- [StockDetail.vue](../../../src/components/detail/StockDetail.vue) 路由：仅 `'minute'` → `MinuteChart`（折线）；其余全部（含分钟 K）→ `KLineChart`（蜡烛 + MA + VOL）。
- [useChart.ts](../../../src/composables/useChart.ts)：
  - `periodToKlinecharts`：`1min→{type:'minute',span:1}`、`5min→{minute,5}`、`15min→{minute,15}`、`30min→{minute,30}`、`60min→{minute,60}`。
  - `getRefreshInterval`：分钟 K 自动刷新间隔——`1min`/`5min` → 8000ms，`15min`/`30min`/`60min` → 20000ms。
  - `loadData`：分钟 K 走已有的 `else`（`get_kline`）分支；对分钟周期设 `hasMoreForward = false`（不懒加载）。
  - `startAutoRefresh`：分钟 K 复用已有的「腾讯增量刷新 / 新浪全量刷新」分支，无需新增逻辑。
  - 蜡烛 tooltip 对分钟周期首列标签用「时间」（现为「日期」）——小幅优化，可选。

### 4. 后端适配器

`fetch_kline(code, market, period, end_date, count)` 增加分钟分支，返回 `KLineData`：

- **腾讯** [tencent.rs](../../../src-tauri/src/datasource/tencent.rs)：分钟周期走 `mkline?param={code},m{N},,{count}`。
  - 解析数组 `[时间, 开, 收, 高, 低, 量, {}, 额]`（**注意第 2 位是收盘、第 3 位是最高、第 4 位是最低**）。
  - `date` = `YYYY-MM-DD HH:MM`（由 `"202606180935"` 格式化）。
  - 量：手 ×100 转股（`VOLUME_HANDS_TO_SHARES`）。
  - 换手率/额：`arr[7]` 若存在则解析，否则置 0。
  - 抽出私有 helper（如 `fetch_minute_kline(tc_code, span, count) -> Vec<KLineData>`），`fetch_kline` 的分钟分支调用它。
- **新浪** [sina.rs](../../../src-tauri/src/datasource/sina.rs)：分钟周期走 `getKLineData?symbol={code}&scale={N}`。
  - `N ∈ {5,15,30,60}`；`1min` 返回 `Unsupported`。
  - 对象 `{day, open, high, low, close, volume}` 直接映射；`date` = `day`（含时分）；`turnover` 缺失置 0。
- period 字符串到 span 的映射（两端一致）：`"1min"→1`、`"5min"→5`、`"15min"→15`、`"30min"→30`、`"60min"→60`。

### 5. 刷新与历史深度

- 分钟 K **一次性加载约 320 根**（两家分钟历史都有限，约几天到十几天），不做左滑懒加载（`hasMoreForward=false`）——规避腾讯 `mkline` 分页不稳定问题。日/周/月懒加载不受影响。
- 增量刷新：腾讯分钟 K 复用现有 `count:10` 增量合并路径；新浪走全量重载路径（现有分支已覆盖）。

## 数据流

```
ChartSwitcher（更多 ▾ → NDropdown 分钟档）
  → StockDetail activePeriod = '5min'
    → KLineChart(period='5min')
      → useChart.loadData → invoke get_kline(period='5min')
        → fetch_kline 分钟分支（腾讯 mkline / 新浪 getKLineData scale）
          → Vec<KLineData> → 蜡烛渲染 + 自动刷新
```

## 错误处理

- 新浪 + `1min`：后端 `Unsupported`，前端错误浮层显示提示。切源兜底优先在前端拦截，后端为兜底防线。
- 分钟接口空数据：沿用现有 `log::warn` + 返回空数组，前端显示空图。
- 网络失败：沿用现有 `AppError::network` + 错误浮层 + 重试按钮。

## 测试要点

- 手动验证：腾讯下 5 档分钟 K 均可渲染蜡烛 + MA + VOL；新浪下下拉无 1 分钟，5/15/30/60 正常。
- 切源兜底：腾讯选 1 分钟 → 切新浪 → 自动回落 5 分钟。
- 自动刷新：分钟 K 最后一根随行情更新，不整屏抖动。
- 精度：3 位小数品种（ETF）分钟 K 价格轴正确（复用 `syncPrecision`）。
- `vue-tsc --noEmit` 与 `cargo build` 通过。

## 不在本次范围

- 分时图（`'minute'` 折线）逻辑改动。
- HK/US 分钟数据。
- 分钟数据本地 SQLite 缓存。
- 分钟 K 导出。
- 分钟 K 左滑历史懒加载。
