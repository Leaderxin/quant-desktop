<script setup lang="ts">
import { computed } from 'vue';
import type { Quote } from '@/types';
import { formatPrice, formatVolume } from '@/utils/format';

const props = defineProps<{
  quote: Quote;
}>();

const items = computed(() => [
  { label: '开盘', value: formatPrice(props.quote.open) },
  { label: '最高', value: formatPrice(props.quote.high) },
  { label: '最低', value: formatPrice(props.quote.low) },
  { label: '成交量', value: formatVolume(props.quote.volume) },
  { label: '成交额', value: (props.quote.turnover / 100000000).toFixed(2) + '亿' },
  {
    label: '换手率',
    value: props.quote.turnover_rate != null ? props.quote.turnover_rate.toFixed(2) + '%' : '--'
  },
]);
</script>

<template>
  <div class="stock-summary">
    <div class="summary-row" v-for="item in items" :key="item.label">
      <span class="summary-label">{{ item.label }}</span>
      <span class="summary-value">{{ item.value }}</span>
    </div>
  </div>
</template>

<style scoped>
.stock-summary {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  padding: 12px;
  background: var(--color-surface-2);
  border-radius: 6px;
}
.summary-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.summary-label {
  font-size: 11px;
  color: var(--color-text-tertiary, #888);
}
.summary-value {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-primary, #e0e0e0);
}
</style>
