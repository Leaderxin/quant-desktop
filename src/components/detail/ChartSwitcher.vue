<script setup lang="ts">
import type { PeriodType } from '@/types';

defineProps<{
  modelValue: PeriodType;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: PeriodType];
}>();

const tabs: { key: PeriodType; label: string }[] = [
  { key: 'minute', label: '分时' },
  { key: 'daily', label: '日K' },
  { key: 'weekly', label: '周K' },
  { key: 'monthly', label: '月K' },
];
</script>

<template>
  <div class="chart-switcher" role="tablist" aria-label="图表类型切换">
    <button
      v-for="tab in tabs"
      :key="tab.key"
      role="tab"
      :aria-selected="modelValue === tab.key"
      class="switcher-tab"
      :class="{ active: modelValue === tab.key }"
      @click="emit('update:modelValue', tab.key)"
    >
      {{ tab.label }}
    </button>
  </div>
</template>

<style scoped>
.chart-switcher {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
  width: fit-content;
}
.switcher-tab {
  padding: 3px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: all var(--transition-fast);
  line-height: 1.4;
}
.switcher-tab:hover {
  color: var(--color-text-secondary);
}
.switcher-tab.active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
</style>
