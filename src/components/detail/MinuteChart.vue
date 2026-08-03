<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { useMinuteChart } from '@/composables/useMinuteChart';

const props = defineProps<{
  code: string;
  market: string;
  name?: string;
}>();

const chartRef = ref<HTMLElement | null>(null);

const { loading, error, initChart, loadData } = useMinuteChart({
  chartRef,
  code: computed(() => props.code),
  market: computed(() => props.market),
  name: computed(() => props.name ?? ''),
});

onMounted(async () => {
  await nextTick();
  initChart();
  await loadData();
});

// Reload when code/market changes
watch(() => [props.code, props.market], async () => {
  await nextTick();
  initChart();
  await loadData();
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
      <button class="chart-retry-btn" @click="loadData()" aria-label="重新加载分时图">重试</button>
    </div>
    <div ref="chartRef" class="chart-container"></div>
  </div>
</template>

<style scoped>
@import '@/assets/chart.css';

.minute-chart {
  flex: 1;
  min-height: 420px;
  position: relative;
}
</style>
