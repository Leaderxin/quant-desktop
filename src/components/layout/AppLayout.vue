<script setup lang="ts">
import TopBar from './TopBar.vue';
import IndexBar from '@/components/index/IndexBar.vue';
import MarketOverviewPanel from '@/components/market/MarketOverviewPanel.vue';
import WatchlistTable from '@/components/watchlist/WatchlistTable.vue';
import StatusBar from './StatusBar.vue';
import SettingsPage from '@/components/settings/SettingsPage.vue';
import { provide, ref } from 'vue';
import { useSettingsStore } from '@/stores/settings';
import { CircleAlert, RefreshCw, TriangleAlert } from 'lucide-vue-next';
import { CLEAR_INDEX_DETAIL_KEY } from '@/utils/keys';

const settings = useSettingsStore();

/**
 * 设置页用 v-if 而不是 v-show 覆盖看盘界面。
 *
 * 两个后果都是想要的：
 * - 设置期间看盘界面的轮询全部停掉（MarketOverviewPanel 卸载即 stopRefresh），
 *   改配置时不会有行情刷新在背后抢 IPC；
 * - 返回看盘时自选表重建，`defaultSortOrder` 这类只在挂载时生效的初值会按新
 *   设置重新应用 —— 否则改完默认排序得重启应用才看得到效果。
 */
const showSettings = ref(false);

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
  appError?: string | null;
}>();

defineEmits<{
  retry: [];
  dismissAppError: [];
}>();
</script>

<template>
  <div class="app-layout">
    <!-- Global init error banner -->
    <div v-if="initError" class="error-banner" role="alert">
      <div class="error-banner-content">
        <CircleAlert class="error-icon" :size="14" aria-hidden="true" />
        <span class="error-text">{{ initError }}</span>
        <button class="error-retry-btn" @click="$emit('retry')" aria-label="重新加载应用">
          <RefreshCw :size="12" aria-hidden="true" />
          重试
        </button>
      </div>
    </div>

    <!-- Global child component error boundary (non-blocking) -->
    <div v-else-if="appError && initReady" class="warning-banner" role="alert">
      <div class="warning-banner-content">
        <TriangleAlert class="warning-icon" :size="13" aria-hidden="true" />
        <span class="warning-text">{{ appError }}</span>
        <button class="error-dismiss-btn" @click="$emit('dismissAppError')" aria-label="关闭">✕</button>
      </div>
    </div>

    <!-- Quote listener error (non-blocking warning) -->
    <div v-else-if="quoteError && initReady" class="warning-banner" role="alert">
      <div class="warning-banner-content">
        <TriangleAlert class="warning-icon" :size="13" aria-hidden="true" />
        <span class="warning-text">行情数据连接异常，部分数据可能不是最新</span>
      </div>
    </div>

    <SettingsPage v-if="showSettings" @close="showSettings = false" />

    <template v-else>
      <TopBar />
      <IndexBar />
      <MarketOverviewPanel v-if="settings.marketOverviewVisible" />
      <main class="main-content">
        <WatchlistTable />
      </main>
    </template>
    <StatusBar :settings-open="showSettings" @open-settings="showSettings = true" />
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
.error-dismiss-btn {
  flex-shrink: 0;
  background: none;
  border: none;
  color: var(--color-warning);
  font-size: 12px;
  cursor: pointer;
  padding: 2px 4px;
  line-height: 1;
  opacity: 0.6;
  transition: opacity var(--transition-fast);
}
.error-dismiss-btn:hover {
  opacity: 1;
}
</style>
