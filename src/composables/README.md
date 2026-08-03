## 图表数据流架构

### 整体分层

```
数据源 API (Rust Tencent/Sina)
  → Scheduler (tokio 自适应轮询)
    → Tauri events → Pinia Stores (主窗口/Ticker)
    → Tauri invoke() → Vue Composables → Canvas 图表
```

### 三层 Composables 职责拆分

```
useChartCore.ts  ←─ 公共基础
  ├─ 图表创建/销毁 (klinecharts init/dispose)
  ├─ themeColors() — 深/浅主题色板
  ├─ applyChartStyles / applyCandlestickStyles — 样式设置
  ├─ syncPrecision() — 价格精度自适应
  └─ initChartCore(period) — 初始化 + 返回 isNew

useMinuteChart.ts  ←─ 分时图 (MinuteChart.vue)
  ├─ invoke get_intraday → 240 分钟点
  ├─ barSubscriber 5s 增量刷新 (无闪烁)

useChart.ts  ←─ K线图 (KLineChart.vue)
  ├─ invoke get_kline → K线数据 + 懒加载历史
  ├─ syncSubIndicator(VOL|MACD) → 副图切换
  └─ MA 叠加 + VOL 自定义外观
```

### 关键数据流

1. **挂载**: initChart → initChartCore → applyStyles → overrideIndicator(VOL) → syncSubIndicator → createIndicator(MA)
2. **加载**: loadData → invoke → map + setDataLoader → getBars('init') 触发渲染 → syncPrecision → startAutoRefresh
3. **周期切换**: watch([code,market,period]) → initChart + loadData → allData 重置
4. **副图切换**: SubIndicatorSwitcher emit → watch(subIndicator) → syncSubIndicator → createIndicator({name}, {pane: SUB_PANE_ID}) → isStack=false 自动替换
5. **主题切换**: watch(settings.theme) → reapplyStyles → themeColors() 重新计算全色板
