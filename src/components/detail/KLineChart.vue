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
.kline-chart {
  flex: 1;
  min-height: 320px;
  position: relative;
}
.chart-container {
  width: 100%;
  height: 320px;
  cursor: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cg stroke='%2358a6ff' stroke-width='2' stroke-linecap='round'%3E%3Cline x1='12' y1='1' x2='12' y2='9'/%3E%3Cline x1='12' y1='15' x2='12' y2='23'/%3E%3Cline x1='1' y1='12' x2='9' y2='12'/%3E%3Cline x1='15' y1='12' x2='23' y2='12'/%3E%3C/g%3E%3Ccircle cx='12' cy='12' r='2' fill='none' stroke='%23ffffff' stroke-width='1.5' opacity='.9'/%3E%3C/svg%3E") 12 12, crosshair;
}
.chart-container :deep(canvas) {
  cursor: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cg stroke='%2358a6ff' stroke-width='2' stroke-linecap='round'%3E%3Cline x1='12' y1='1' x2='12' y2='9'/%3E%3Cline x1='12' y1='15' x2='12' y2='23'/%3E%3Cline x1='1' y1='12' x2='9' y2='12'/%3E%3Cline x1='15' y1='12' x2='23' y2='12'/%3E%3C/g%3E%3Ccircle cx='12' cy='12' r='2' fill='none' stroke='%23ffffff' stroke-width='1.5' opacity='.9'/%3E%3C/svg%3E") 12 12, crosshair !important;
  will-change: transform;
  transform: translateZ(0);
  image-rendering: optimizeSpeed;
}
.chart-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  background: var(--color-surface-1);
}
.chart-status-text {
  font-size: 12px;
  color: var(--color-text-tertiary);
}
.chart-error-overlay {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  background: var(--color-surface-1);
}
.chart-error-icon {
  color: var(--color-warning);
}
.chart-error-text {
  font-size: var(--text-xs);
  color: var(--color-warning);
  text-align: center;
  max-width: 240px;
  line-height: 1.4;
}
.chart-retry-btn {
  display: inline-flex;
  align-items: center;
  padding: 3px 12px;
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-sm);
  background: var(--color-warning-bg);
  color: var(--color-warning);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.chart-retry-btn:hover {
  filter: brightness(1.2);
}
</style>
