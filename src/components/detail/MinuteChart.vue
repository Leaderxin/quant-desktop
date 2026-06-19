<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { useChart } from '@/composables/useChart';

const props = defineProps<{
  code: string;
  market: string;
  name?: string;
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
  await initChart('minute');
  await loadData('minute');
});

// Reload when code/market changes
watch(() => [props.code, props.market], async () => {
  await nextTick();
  await initChart('minute');
  await loadData('minute');
});
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
      <button class="chart-retry-btn" @click="loadData('minute')" aria-label="重新加载分时图">重试</button>
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
