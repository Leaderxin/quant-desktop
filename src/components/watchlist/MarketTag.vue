<script setup lang="ts">
import { computed } from 'vue';
import { marketTag } from '@/utils/format';

const props = defineProps<{ code: string; category: string }>();

const label = computed(() => marketTag(props.code, props.category));

const cls = computed(() => {
  if (props.category === 'ZS') return 'tag-index';
  if (props.category === 'ETF' || props.category === 'LOF') return 'tag-fund';
  return 'tag-stock';
});
</script>

<template>
  <span v-if="label" class="market-tag" :class="cls">{{ label }}</span>
</template>

<style scoped>
.market-tag {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* 固定最小宽度，让「沪指/深A/ETF」等不同字数标签等宽，代码列对齐 */
  min-width: 36px;
  height: 18px;
  padding: 0 6px;
  font-size: var(--text-xs);
  line-height: 1;
  border-radius: var(--radius-sm);
  white-space: nowrap;
  font-family: var(--font-sans);
  font-weight: var(--font-weight-medium);
}
.tag-stock {
  color: var(--color-accent);
  background: var(--color-accent-dim);
}
.tag-index {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}
.tag-fund {
  color: var(--color-text-secondary);
  background: var(--color-bg-elevated);
}
</style>
