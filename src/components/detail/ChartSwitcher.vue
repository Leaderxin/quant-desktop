<script setup lang="ts">
import { computed, ref } from 'vue';
import type { PeriodType } from '@/types';
import { useSettingsStore } from '@/stores/settings';
import { NDropdown } from 'naive-ui';

const props = defineProps<{
  modelValue: PeriodType;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: PeriodType];
}>();

const settings = useSettingsStore();
const dropdownOpen = ref(false);

const tabs: { key: PeriodType; label: string }[] = [
  { key: 'minute', label: '分时' },
  { key: 'daily', label: '日K' },
  { key: 'weekly', label: '周K' },
  { key: 'monthly', label: '月K' },
];

const minuteOptions: { key: PeriodType; label: string }[] = [
  { key: '1min', label: '1分' },
  { key: '5min', label: '5分' },
  { key: '15min', label: '15分' },
  { key: '30min', label: '30分' },
  { key: '60min', label: '60分' },
];

// 新浪无 1 分钟：过滤掉 1min 档
const availableMinutes = computed(() =>
  settings.activeDatasource === 'sina'
    ? minuteOptions.filter((o) => o.key !== '1min')
    : minuteOptions
);

const dropdownOptions = computed(() =>
  availableMinutes.value.map((o) => ({
    label: props.modelValue === o.key ? `✓ ${o.label}` : o.label,
    key: o.key,
  }))
);

const isMinuteActive = computed(() =>
  availableMinutes.value.some((o) => o.key === props.modelValue)
);

// 选中某分钟周期时「更多」按钮显示该周期，否则显示「更多」
const moreLabel = computed(() => {
  const found = availableMinutes.value.find((o) => o.key === props.modelValue);
  return found ? found.label : '更多';
});

function handleMinuteSelect(key: string) {
  emit('update:modelValue', key as PeriodType);
}
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

    <n-dropdown
      trigger="click"
      :show="dropdownOpen"
      @update:show="(v: boolean) => dropdownOpen = v"
      :options="dropdownOptions"
      @select="handleMinuteSelect"
    >
      <button
        class="switcher-tab switcher-more"
        :class="{ active: isMinuteActive }"
        :aria-selected="isMinuteActive"
        aria-haspopup="menu"
        :aria-expanded="dropdownOpen"
      >
        {{ moreLabel }}<span class="more-caret" aria-hidden="true">▾</span>
      </button>
    </n-dropdown>
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
.switcher-more {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
.more-caret {
  font-size: 9px;
  line-height: 1;
}
</style>
