<script setup lang="ts">
import { useQuoteStore } from '@/stores/quote';
import IndexCard from './IndexCard.vue';

const quote = useQuoteStore();
</script>

<template>
  <div class="index-bar" v-if="quote.indices.length > 0">
    <IndexCard v-for="idx in quote.indices" :key="idx.code" :index="idx" />
  </div>
  <div v-else class="index-placeholder">
    <span class="placeholder-dot"></span>
    等待指数数据...
  </div>
</template>

<style scoped>
.index-bar {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  background: var(--color-surface-0);
  border-bottom: 1px solid var(--color-border-0);
  flex-shrink: 0;
}
.index-placeholder {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
  border-bottom: 1px solid var(--color-border-0);
  flex-shrink: 0;
}
.placeholder-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--color-text-tertiary);
  animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}
@media (prefers-reduced-motion: reduce) {
  .placeholder-dot { animation: none; opacity: 0.5; }
}
</style>
