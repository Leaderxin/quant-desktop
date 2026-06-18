<script setup lang="ts">
import type { IndexQuote } from '@/types';
import { computed } from 'vue';
import { formatPrice } from '@/utils/format';

const props = defineProps<{
  index: IndexQuote;
  selected?: boolean;
}>();

const emit = defineEmits<{
  select: [index: IndexQuote];
}>();

const isUp = computed(() => props.index.change_pct >= 0);
</script>

<template>
  <div
    class="index-card"
    :class="{
      'card-up': isUp,
      'card-down': !isUp,
      'card-selected': selected
    }"
    role="button"
    tabindex="0"
    :aria-label="`查看 ${index.name} 详情`"
    @click="emit('select', index)"
    @keydown.enter="emit('select', index)"
    @keydown.space.prevent="emit('select', index)"
  >
    <span class="index-name">{{ index.name }}</span>
    <span class="index-price tabular-nums" :class="isUp ? 'up' : 'down'">
      {{ formatPrice(index.price) }}
    </span>
    <div class="index-change-row tabular-nums">
      <span :class="isUp ? 'up' : 'down'">{{ isUp ? '+' : '' }}{{ formatPrice(index.change) }}</span>
      <span :class="isUp ? 'up' : 'down'">{{ isUp ? '+' : '' }}{{ index.change_pct.toFixed(2) }}%</span>
    </div>
  </div>
</template>

<style scoped>
.index-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  background: var(--color-surface-1);
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
  padding: var(--space-2) var(--space-3);
  width: 140px;
  height: 60px;
  flex-shrink: 0;
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.index-card:hover {
  background: var(--color-bg-elevated, rgba(255,255,255,0.04));
  border-color: var(--color-border, rgba(255,255,255,0.12));
}
.index-card:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.card-selected {
  border-color: var(--color-accent) !important;
  box-shadow: 0 0 0 1px var(--color-accent);
}

.card-up {
  background: var(--color-up-bg);
}
.card-down {
  background: var(--color-down-bg);
}

.index-name {
  font-size: var(--text-xs);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
  white-space: nowrap;
  line-height: 1.2;
}

.index-price {
  font-size: var(--text-xl);
  font-weight: var(--font-weight-bold);
  font-family: var(--font-mono);
  line-height: 1.3;
}

.index-change-row {
  display: flex;
  gap: var(--space-3);
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  line-height: 1.2;
}

.up { color: var(--color-up); }
.down { color: var(--color-down); }
</style>
