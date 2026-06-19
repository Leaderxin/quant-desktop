<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
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
  code: computed(() => props.code),
  market: computed(() => props.market),
  name: computed(() => props.name ?? ''),
});

onMounted(async () => {
  await nextTick();
  await initChart(props.period);
  await loadData(props.period);
});

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
@import '@/assets/chart.css';

.kline-chart {
  flex: 1;
  min-height: 320px;
  position: relative;
}
</style>
