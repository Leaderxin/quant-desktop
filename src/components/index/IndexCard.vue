<script setup lang="ts">
import type { IndexQuote } from '@/types';
import { computed } from 'vue';

const props = defineProps<{ index: IndexQuote }>();
const isUp = computed(() => props.index.change_pct >= 0);
</script>

<template>
  <div class="index-card" :class="isUp ? 'card-up' : 'card-down'">
    <span class="index-name">{{ index.name }}</span>
    <span class="index-price tabular-nums" :class="isUp ? 'up' : 'down'">
      {{ index.price.toFixed(2) }}
    </span>
    <span class="index-change tabular-nums" :class="isUp ? 'up' : 'down'">
      {{ isUp ? '+' : '' }}{{ index.change_pct.toFixed(2) }}%
    </span>
  </div>
</template>

<style scoped>
.index-card {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-left: 2px solid var(--color-border-0);
  border-radius: var(--radius-md);
  padding: var(--space-2) var(--space-3);
  flex-shrink: 0;
  min-width: 155px;
  height: 44px;
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
  font-size: var(--text-md);
  font-weight: var(--font-weight-bold);
  font-family: var(--font-mono);
}
.index-change {
  font-size: var(--text-sm);
  font-weight: var(--font-weight-semibold);
  font-family: var(--font-mono);
  min-width: 58px;
  text-align: right;
}
.up { color: var(--color-up); }
.down { color: var(--color-down); }
</style>
