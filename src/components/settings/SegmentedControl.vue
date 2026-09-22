<script setup lang="ts" generic="T extends string | number">
// 分段控件。样式与股票详情里的周期切换（ChartSwitcher）、市场概览的方向切换
// 保持同一套 —— 全应用只有一种「多选一」的视觉语言。
const props = defineProps<{
  modelValue: T;
  options: { value: T; label: string }[];
  /** 无障碍分组名，如「板块涨跌排名条数」。 */
  label: string;
  small?: boolean;
}>();
const emit = defineEmits<{ (e: 'update:modelValue', v: T): void }>();

function select(v: T) {
  if (v !== props.modelValue) emit('update:modelValue', v);
}
</script>

<template>
  <div class="seg" :class="{ small }" role="radiogroup" :aria-label="label">
    <button
      v-for="o in options"
      :key="String(o.value)"
      type="button"
      role="radio"
      :aria-checked="modelValue === o.value"
      :class="{ active: modelValue === o.value }"
      @click="select(o.value)"
    >
      {{ o.label }}
    </button>
  </div>
</template>

<style scoped>
.seg {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--radius-md);
  background: var(--color-surface-2);
}
.seg button {
  padding: 3px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-tertiary);
  font-family: var(--font-sans);
  font-size: var(--text-xs);
  line-height: 1.4;
  white-space: nowrap;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.seg.small button {
  padding: 2px 8px;
}
.seg button:hover {
  color: var(--color-text-secondary);
}
.seg button.active {
  background: var(--color-accent-dim);
  color: var(--color-accent);
  font-weight: var(--font-weight-medium);
}
.seg button:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}
</style>
