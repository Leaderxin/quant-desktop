<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { useChart } from '@/composables/useChart';
import type { PeriodType, SubIndicatorType, MainOverlayType } from '@/types';
import { CircleAlert } from '@lucide/vue';

const props = defineProps<{
  code: string;
  market: string;
  name?: string;
  period: PeriodType;
  subIndicator: SubIndicatorType;
  mainOverlay: MainOverlayType;
}>();

const chartRef = ref<HTMLElement | null>(null);

const { loading, error, initChart, loadData } = useChart({
  chartRef,
  code: computed(() => props.code),
  market: computed(() => props.market),
  name: computed(() => props.name ?? ''),
  subIndicator: computed(() => props.subIndicator),
  mainOverlay: computed(() => props.mainOverlay),
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
      <CircleAlert class="chart-error-icon" :size="14" aria-hidden="true" />
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
  min-height: 420px;
  position: relative;
}
</style>
