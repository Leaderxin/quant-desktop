<script setup lang="ts">
import { computed, ref, inject, onMounted } from 'vue';
import { useQuoteStore } from '@/stores/quote';
import { useSettingsStore } from '@/stores/settings';
import type { IndexQuote } from '@/types';
import IndexCard from './IndexCard.vue';
import IndexDetail from '@/components/detail/IndexDetail.vue';
import { CLEAR_INDEX_DETAIL_KEY } from '@/utils/keys';

const quote = useQuoteStore();
const settings = useSettingsStore();

const indexDetailCoord = inject<{
  registerClearIndexFn: (fn: () => void) => void;
  clearStockDetail: () => void;
} | undefined>(CLEAR_INDEX_DETAIL_KEY);

onMounted(() => {
  indexDetailCoord?.registerClearIndexFn(() => {
    selectedIndex.value = null;
  });
});

/**
 * 顶栏实际展示的指数：按设置里的顺序与勾选过滤。
 *
 * 后端一次性拉取整个候选池（14 个），前端在这里筛选排序 —— 这样用户在设置页
 * 勾一个指数**当场**就能看到它出现，不必等下一轮轮询（盘中 2 秒、休市可达 30 秒）。
 * 代价是每次多几行 HTTP 响应文本，可忽略。
 *
 * 用 map 而不是对 `quote.indices` 排序：配置顺序才是唯一权威，行情返回顺序
 * 不该影响顶栏排列。
 */
const displayIndices = computed<IndexQuote[]>(() => {
  const byCode = new Map(quote.indices.map((i) => [i.code, i]));
  return settings.indexCodes
    .map((code) => byCode.get(code))
    .filter((i): i is IndexQuote => i !== undefined);
});

const selectedIndex = ref<IndexQuote | null>(null);

function handleSelect(index: IndexQuote) {
  if (selectedIndex.value?.code === index.code) {
    // Toggle: deselect
    selectedIndex.value = null;
  } else {
    indexDetailCoord?.clearStockDetail();
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
    <div class="index-bar" v-if="displayIndices.length > 0">
      <IndexCard
        v-for="idx in displayIndices"
        :key="idx.code"
        :index="idx"
        :selected="selectedIndex?.code === idx.code"
        @select="handleSelect"
      />
    </div>
    <div v-else class="index-placeholder">
      <span class="placeholder-dot"></span>
      <!-- 区分「还没拉到数据」与「用户把指数全取消了」：后者提示去设置页 -->
      {{ settings.indexCodes.length === 0 ? '未选择指数，请在设置中勾选' : '等待指数数据...' }}
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
  position: relative;
  border-bottom: 1px solid var(--color-border-0);
}

.index-bar {
  display: flex;
  flex-wrap: nowrap;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  background: var(--color-surface-0);
  overflow-x: auto;
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
