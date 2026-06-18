<script setup lang="ts">
import { ref } from 'vue';
import { useQuoteStore } from '@/stores/quote';
import type { IndexQuote } from '@/types';
import IndexCard from './IndexCard.vue';
import IndexDetail from '@/components/detail/IndexDetail.vue';

const quote = useQuoteStore();
const selectedIndex = ref<IndexQuote | null>(null);

function handleSelect(index: IndexQuote) {
  if (selectedIndex.value?.code === index.code) {
    // Toggle: deselect
    selectedIndex.value = null;
  } else {
    selectedIndex.value = index;
  }
}

function handleCloseDetail() {
  selectedIndex.value = null;
}

// Expose for parent coordination
defineExpose({
  clearSelection: () => { selectedIndex.value = null; },
});
</script>

<template>
  <div class="index-section">
    <div class="index-bar" v-if="quote.indices.length > 0">
      <IndexCard
        v-for="idx in quote.indices"
        :key="idx.code"
        :index="idx"
        :selected="selectedIndex?.code === idx.code"
        @select="handleSelect"
      />
    </div>
    <div v-else class="index-placeholder">
      <span class="placeholder-dot"></span>
      等待指数数据...
    </div>

    <IndexDetail
      v-if="selectedIndex"
      :index="selectedIndex"
      @close="handleCloseDetail"
    />
  </div>
</template>

<style scoped>
.index-section {
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border-0);
}

.index-bar {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  background: var(--color-surface-0);
}

.index-placeholder {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  color: var(--color-text-tertiary);
  font-size: var(--text-xs);
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
