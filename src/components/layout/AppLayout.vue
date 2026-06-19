<script setup lang="ts">
import TopBar from './TopBar.vue';
import IndexBar from '@/components/index/IndexBar.vue';
import WatchlistTable from '@/components/watchlist/WatchlistTable.vue';
import StatusBar from './StatusBar.vue';
import { provide, ref } from 'vue';
import { CLEAR_INDEX_DETAIL_KEY } from '@/utils/keys';

const clearIndexDetailFn = ref<(() => void) | null>(null);
const clearStockDetailFn = ref<(() => void) | null>(null);

provide(CLEAR_INDEX_DETAIL_KEY, {
  registerClearIndexFn: (fn: () => void) => { clearIndexDetailFn.value = fn; },
  clearIndexDetail: () => { clearIndexDetailFn.value?.(); },
  registerClearStockFn: (fn: () => void) => { clearStockDetailFn.value = fn; },
  clearStockDetail: () => { clearStockDetailFn.value?.(); },
});

defineProps<{
  initError?: string | null;
  initReady?: boolean;
  quoteError?: string | null;
}>();

defineEmits<{
  retry: [];
}>();
</script>

<template>
  <div class="app-layout">
    <!-- Global init error banner -->
    <div v-if="initError" class="error-banner" role="alert">
      <div class="error-banner-content">
        <svg class="error-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true">
          <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
          <path d="M8 4.5v3.5M8 10.5h.007" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        <span class="error-text">{{ initError }}</span>
        <button class="error-retry-btn" @click="$emit('retry')" aria-label="重新加载应用">
          <svg viewBox="0 0 14 14" width="12" height="12" fill="none" aria-hidden="true">
            <path d="M2 7a5 5 0 0 1 8.5-3.5M12 7a5 5 0 0 1-8.5 3.5M10.5 1.5v2h-2M3.5 12.5v-2h2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          重试
        </button>
      </div>
    </div>

    <!-- Quote listener error (non-blocking warning) -->
    <div v-else-if="quoteError && initReady" class="warning-banner" role="alert">
      <div class="warning-banner-content">
        <svg class="warning-icon" viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
          <path d="M8 1.5L15.5 14.5H.5L8 1.5z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
          <path d="M8 6v3M8 11h.007" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
        </svg>
        <span class="warning-text">行情数据连接异常，部分数据可能不是最新</span>
      </div>
    </div>

    <TopBar />
    <IndexBar />
    <main class="main-content">
      <WatchlistTable />
    </main>
    <StatusBar />
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--color-surface-0);
}

.main-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* --- Error banner (blocking) --- */
.error-banner {
  background: var(--color-warning-bg);
  border-bottom: 1px solid var(--color-warning-border);
  flex-shrink: 0;
}
.error-banner-content {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  max-width: 100%;
}
.error-icon {
  color: var(--color-warning);
  flex-shrink: 0;
}
.error-text {
  flex: 1;
  min-width: 0;
  font-size: var(--text-xs);
  color: var(--color-warning);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.error-retry-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  padding: 2px 8px;
  border: 1px solid var(--color-warning-border);
  border-radius: var(--radius-sm);
  background: var(--color-warning-bg);
  color: var(--color-warning);
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.error-retry-btn:hover {
  filter: brightness(1.2);
}

/* --- Warning banner (non-blocking, quote error) --- */
.warning-banner {
  background: var(--color-warning-bg);
  border-bottom: 1px solid var(--color-warning-border);
  flex-shrink: 0;
  opacity: 0.7;
}
.warning-banner-content {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
}
.warning-icon {
  color: var(--color-warning);
  flex-shrink: 0;
}
.warning-text {
  font-size: 11px;
  color: var(--color-warning);
}
</style>
