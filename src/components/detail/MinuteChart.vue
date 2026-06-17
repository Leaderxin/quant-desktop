<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { init, dispose } from 'klinecharts';
import type { Chart, KLineData, DataLoader } from 'klinecharts';
import type { MinuteData } from '@/types';

const props = defineProps<{
  code: string;
  market: string;
}>();

const chartRef = ref<HTMLElement | null>(null);
let chart: Chart | null = null;
const loading = ref(false);

// Holds the transformed kline data for the current stock
const klineData = ref<KLineData[]>([]);

// DataLoader that reads from klineData ref
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
  loading.value = true;
  try {
    const data = await invoke<MinuteData[]>('get_intraday', { code: props.code, market: props.market });
    if (!chart || !data.length) return;

    klineData.value = data.map((d, i) => ({
      timestamp: Date.now() + i * 60000,
      open: d.price,
      high: d.price,
      low: d.price,
      close: d.price,
      volume: d.volume,
    }));

    chart.resetData();
  } catch (e) {
    console.error('Failed to load intraday data:', e);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  await nextTick();
  if (chartRef.value) {
    chart = init(chartRef.value);
    if (chart) {
      chart.setStyles({
        grid: {
          show: true,
          horizontal: { show: true, color: 'rgba(255,255,255,0.05)' },
          vertical: { show: true, color: 'rgba(255,255,255,0.05)' },
        },
        candle: {
          bar: { upColor: '#ef5350', downColor: '#66bb6a' },
        },
      });
      chart.setDataLoader(dataLoader);
      loadData();
    }
  }
});

onUnmounted(() => {
  if (chart) {
    dispose(chart);
    chart = null;
  }
});

watch(() => props.code, () => { loadData(); });
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
