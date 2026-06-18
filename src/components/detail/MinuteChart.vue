<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { init, dispose } from 'klinecharts';
import type { Chart, KLineData, DataLoader } from 'klinecharts';
import type { MinuteData } from '@/types';
import { useSettingsStore } from '@/stores/settings';

const settings = useSettingsStore();

const props = defineProps<{
  code: string;
  market: string;
  name?: string;
}>();

const chartRef = ref<HTMLElement | null>(null);
let chart: Chart | null = null;
const loading = ref(false);
const error = ref('');
let abortController: AbortController | null = null;

function applyChartStyles() {
  if (!chart) return;
  const isDark = settings.theme === 'dark';
  const lineColor = isDark ? 'rgba(255,255,255,0.25)' : 'rgba(0,0,0,0.2)';
  const gridHColor = isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)';
  const gridVColor = isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.04)';
  const axisColor = isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)';
  const tickColor = isDark ? '#8b949e' : '#656d76';
  const tooltipBg = isDark ? 'rgba(22,27,34,0.95)' : 'rgba(255,255,255,0.95)';
  const tooltipText = isDark ? '#c9d1d9' : '#24292f';
  const separatorColor = isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)';
  const crosshairBg = isDark ? 'rgba(22,27,34,0.9)' : 'rgba(255,255,255,0.9)';

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
      horizontal: { show: true, line: { show: true, color: lineColor, size: 1 }, text: { show: true, size: 10, color: tooltipText, family: 'var(--font-mono)', backgroundColor: crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any } as any,
      vertical: { show: true, line: { show: true, color: lineColor, size: 1 }, text: { show: true, size: 10, color: tooltipText, family: 'var(--font-mono)', backgroundColor: crosshairBg, paddingLeft: 4, paddingTop: 2, paddingRight: 4, paddingBottom: 2 } as any } as any,
    },
  });
}

// Holds the transformed kline data for the current stock
const klineData = ref<KLineData[]>([]);

// DataLoader that feeds klineData to the chart
const dataLoader: DataLoader = {
  getBars: (params) => {
    if (params.type === 'init') {
      params.callback(klineData.value, false);
    } else {
      params.callback([], false);
    }
  },
};

async function loadData() {
  // Abort any in-flight request from a previous code change
  if (abortController) {
    abortController.abort();
  }
  abortController = new AbortController();
  const { signal } = abortController;

  loading.value = true;
  error.value = '';
  try {
    const data = await invoke<MinuteData[]>('get_intraday', { code: props.code, market: props.market });

    // Discard stale response if aborted or code changed while fetching
    if (signal.aborted) return;

    if (!data.length) {
      // No data yet (e.g., market not open) — not an error, chart stays empty
      return;
    }

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

    if (signal.aborted) return;

    if (chart) {
      chart.setDataLoader(dataLoader);
    }
  } catch (e) {
    if (signal.aborted) return;
    error.value = `加载分时数据失败: ${String(e).slice(0, 80)}`;
    console.error('[MinuteChart] failed:', e);
  } finally {
    if (!signal.aborted) {
      loading.value = false;
    }
  }
}

onMounted(async () => {
  await nextTick();
  if (chartRef.value) {
    chart = init(chartRef.value, {
      locale: 'zh-CN',
      layout: { basicParams: { yAxisInside: true } },
    });
    if (chart) {
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
      chart.setSymbol({ ticker: props.code, name: props.name || props.code });
      chart.setPeriod({ type: 'minute', span: 5 });
      applyChartStyles();
    }
  }
  await loadData();
});

onUnmounted(() => {
  if (chart) {
    dispose(chart);
    chart = null;
  }
});

watch(() => props.code, () => { loadData(); });
watch(() => settings.theme, () => { applyChartStyles(); });
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
      <button class="chart-retry-btn" @click="loadData()" aria-label="重新加载分时图">重试</button>
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
  background: var(--color-surface-0);
}
.chart-status-text {
  font-size: 12px;
  color: var(--color-text-tertiary);
}
.chart-error-overlay {
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: rgba(22, 27, 34, 0.92);
}
.chart-error-icon {
  color: #ffa657;
}
.chart-error-text {
  font-size: 12px;
  color: #d29922;
  text-align: center;
  max-width: 240px;
  line-height: 1.4;
}
.chart-retry-btn {
  display: inline-flex;
  align-items: center;
  padding: 3px 12px;
  border: 1px solid rgba(255, 166, 87, 0.25);
  border-radius: var(--radius-sm);
  background: rgba(255, 166, 87, 0.06);
  color: #ffa657;
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.chart-retry-btn:hover {
  background: rgba(255, 166, 87, 0.14);
}
</style>
