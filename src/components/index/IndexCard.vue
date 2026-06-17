<script setup lang="ts">
import type { IndexQuote } from '@/types';
import { computed } from 'vue';

const props = defineProps<{ index: IndexQuote }>();
const isUp = computed(() => props.index.change_pct >= 0);
</script>

<template>
  <div class="index-card" :class="isUp ? 'card-up' : 'card-down'">
    <span class="index-name">{{ index.name }}</span>
    <span class="index-price tabular-nums">{{ index.price.toFixed(2) }}</span>
    <span class="index-change tabular-nums">
      {{ isUp ? '+' : '' }}{{ index.change_pct.toFixed(2) }}%
    </span>
  </div>
</template>

<style scoped>
.index-card {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  padding: var(--space-2) var(--space-3);
  flex-shrink: 0;
  min-width: 150px;
  transition: border-color var(--transition-fast);
}
.card-up { border-left: 2px solid var(--color-up); }
.card-down { border-left: 2px solid var(--color-down); }

.index-name {
  font-size: var(--text-xs);
  color: var(--color-text-secondary);
  white-space: nowrap;
  margin-right: auto;
}
.index-price {
  font-size: var(--text-base);
  font-weight: var(--font-weight-semibold);
  font-family: var(--font-mono);
  color: var(--color-text-primary);
}
.index-change {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-medium);
  font-family: var(--font-mono);
  min-width: 56px;
  text-align: right;
}
.card-up .index-change { color: var(--color-up); }
.card-down .index-change { color: var(--color-down); }
</style>
