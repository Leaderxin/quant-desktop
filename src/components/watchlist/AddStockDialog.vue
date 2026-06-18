<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue';
import { NModal, NCard, NInput, NButton, NSpace, useMessage } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import type { StockBrief } from '@/types';
import { useWatchlistStore } from '@/stores/watchlist';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: 'update:show', val: boolean): void }>();
const message = useMessage();
const watchlist = useWatchlistStore();

const keyword = ref('');
const results = ref<StockBrief[]>([]);
const searching = ref(false);
const searchError = ref('');
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
});

watch(() => keyword.value, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!val || val.trim().length === 0) {
    results.value = [];
    searchError.value = '';
    return;
  }
  debounceTimer = setTimeout(async () => {
    searching.value = true;
    searchError.value = '';
    try {
      results.value = await invoke<StockBrief[]>('search_stocks', { keyword: val.trim() });
    } catch (e) {
      results.value = [];
      searchError.value = `搜索失败: ${String(e).slice(0, 60)}`;
    } finally {
      searching.value = false;
    }
  }, 300);
});

async function handleAdd(stock: StockBrief) {
  try {
    await watchlist.addStock(stock.code, stock.market, stock.name);
    message.success(`已添加 ${stock.name}`);
    keyword.value = '';
    results.value = [];
  } catch (e) {
    message.error(`添加失败: ${e}`);
  }
}
</script>

<template>
  <NModal :show="props.show" @update:show="emit('update:show', $event)">
    <NCard
      title="添加自选"
      style="width: 400px;"
      closable
      @close="emit('update:show', false)"
      :bordered="false"
    >
      <NSpace vertical :size="12">
        <NInput
          v-model:value="keyword"
          placeholder="输入代码或名称..."
          :loading="searching"
          clearable
          size="medium"
        >
          <template #prefix>
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true" style="color:var(--color-text-tertiary)">
              <circle cx="7" cy="7" r="5"/>
              <path d="M11 11l2.5 2.5"/>
            </svg>
          </template>
        </NInput>

        <div class="results-wrapper">
          <div v-if="results.length > 0" class="results-list">
            <div
              v-for="s in results"
              :key="s.code"
              class="result-item"
              @click="handleAdd(s)"
            >
              <div class="result-info">
                <span class="result-name">{{ s.name }}</span>
                <span class="result-code tabular-nums">{{ s.code }}</span>
              </div>
              <NButton size="tiny" type="primary" ghost @click.stop="handleAdd(s)">
                添加
              </NButton>
            </div>
          </div>

          <div v-else-if="searchError" class="search-error" role="alert">
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" aria-hidden="true" class="search-error-icon">
              <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
              <path d="M8 4.5v3.5M8 10.5h.007" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
            <span>{{ searchError }}</span>
          </div>
          <div v-else-if="keyword && !searching" class="no-results">
            未找到匹配标的
          </div>
        </div>
      </NSpace>
    </NCard>
  </NModal>
</template>

<style scoped>
.results-wrapper {
  min-height: 260px;
}
.results-list {
  max-height: 260px;
  overflow-y: auto;
  border: 1px solid var(--color-border-0);
  border-radius: var(--radius-md);
}
.result-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  transition: background var(--transition-fast);
  border-bottom: 1px solid var(--color-border-0);
}
.result-item:last-child { border-bottom: none; }
.result-item:hover { background: var(--color-surface-2); }

.result-info {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
.result-name {
  font-size: var(--text-md);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-primary);
}
.result-code {
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  color: var(--color-text-tertiary);
}
.no-results {
  text-align: center;
  padding: var(--space-6);
  color: var(--color-text-tertiary);
  font-size: var(--text-sm);
}
.search-error {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: var(--space-4) var(--space-3);
  color: var(--color-warning);
  font-size: var(--text-xs);
  text-align: center;
}
.search-error-icon {
  flex-shrink: 0;
  color: var(--color-warning);
}
</style>
