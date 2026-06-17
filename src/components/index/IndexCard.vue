<script setup lang="ts">
import type { IndexQuote } from '@/types';
import { computed } from 'vue';

const props = defineProps<{ index: IndexQuote }>();

const isUp = computed(() => props.index.change_pct >= 0);
</script>

<template>
  <div class="index-card">
    <span class="index-name">{{ index.name }}</span>
    <span class="index-price" :class="isUp ? 'up' : 'down'">
      {{ index.price.toFixed(2) }}
    </span>
    <span class="index-change" :class="isUp ? 'up' : 'down'">
      {{ isUp ? '+' : '' }}{{ index.change_pct.toFixed(2) }}%
    </span>
  </div>
</template>

<style scoped>
.index-card {
  display: flex;
  gap: 8px;
  align-items: center;
  background-color: var(--color-card-bg);
  border-radius: 6px;
  padding: 6px 12px;
  flex-shrink: 0;
  font-size: var(--font-size-sm);
}
.index-name {
  color: var(--color-text-secondary);
  white-space: nowrap;
}
.index-price {
  font-weight: 700;
}
.up { color: var(--color-up); }
.down { color: var(--color-down); }
.index-change {
  font-weight: 500;
}
</style>
