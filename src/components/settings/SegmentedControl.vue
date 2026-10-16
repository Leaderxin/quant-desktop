<script setup lang="ts" generic="T extends string | number">
// 分段控件。样式与股票详情里的周期切换（ChartSwitcher）、市场概览的方向切换
// 保持同一套 —— 全应用只有一种「多选一」的视觉语言。
import type { Component } from 'vue';

const props = defineProps<{
  modelValue: T;
  /** `icon` 可选：给图标就渲染成「图标 + 文字」（如主题三态），不给就只渲染文字。 */
  options: { value: T; label: string; icon?: Component }[];
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
      <!-- 图标是装饰：文案就在旁边，读屏念一遍文字即可 -->
      <component
        :is="o.icon"
        v-if="o.icon"
        class="seg-icon"
        :size="14"
        aria-hidden="true"
      />
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
  /* inline-flex 而不是默认的 block：带图标的项要图标与文字一行居中。
     不带图标的项看不出差别（父级是 flex，这一层照样被 blockify 成 flex）。 */
  display: inline-flex;
  align-items: center;
  gap: 5px;
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
/* 图标走 currentColor，因而跟着选中态/悬停一起变色，不需要单独的配色规则。
   14px 是按 11px 的字号配的：lucide 的图形在 24 视框里留了边距，14px 画出来的
   实际笔画约 11px，与旁边的字等高。 */
.seg-icon {
  flex-shrink: 0;
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
