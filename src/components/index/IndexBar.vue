<script setup lang="ts">
import { useQuoteStore } from '@/stores/quote';
import IndexCard from './IndexCard.vue';

const quote = useQuoteStore();
</script>

<template>
  <div class="index-bar">
    <IndexCard
      v-for="idx in quote.indices"
      :key="idx.code"
      :index="idx"
    />
    <div v-if="quote.indices.length === 0" class="index-placeholder">
      等待指数数据...
    </div>
  </div>
</template>

<style scoped>
.index-bar {
  display: flex;
  gap: 10px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  overflow-x: auto;
}
.index-placeholder {
  color: var(--color-text-secondary);
  font-size: var(--font-size-sm);
  padding: 8px 0;
}
</style>
