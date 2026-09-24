<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { useMinuteChart } from '@/composables/useMinuteChart';
import { CircleAlert } from 'lucide-vue-next';

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
      <CircleAlert class="chart-error-icon" :size="14" aria-hidden="true" />
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
