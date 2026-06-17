<script setup lang="ts">
import { ref, h } from 'vue';
import { NButton, NDataTable, NDropdown, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import type { WatchItem } from '@/types';
import AddStockDialog from './AddStockDialog.vue';

const watchlist = useWatchlistStore();
const quoteStore = useQuoteStore();
const message = useMessage();
const showAddDialog = ref(false);

// Context menu state
const ctxMenuX = ref(0);
const ctxMenuY = ref(0);
const ctxMenuItem = ref<WatchItem | null>(null);
const showCtxMenu = ref(false);

function handleContextMenu(e: MouseEvent, row: WatchItem) {
  e.preventDefault();
  ctxMenuX.value = e.clientX;
  ctxMenuY.value = e.clientY;
  ctxMenuItem.value = row;
  showCtxMenu.value = true;
}

async function handleDelete() {
  if (!ctxMenuItem.value) return;
  await watchlist.removeStock(ctxMenuItem.value.code, ctxMenuItem.value.market);
  showCtxMenu.value = false;
}

async function handleMoveTop() {
  if (!ctxMenuItem.value) return;
  await invoke('move_watch_top', { id: ctxMenuItem.value.id });
  await watchlist.fetchWatchlist();
  showCtxMenu.value = false;
}

async function handleMoveUp() {
  if (!ctxMenuItem.value) return;
  await invoke('move_watch_up', { id: ctxMenuItem.value.id });
  await watchlist.fetchWatchlist();
  showCtxMenu.value = false;
}

async function handleMoveDown() {
  if (!ctxMenuItem.value) return;
  await invoke('move_watch_down', { id: ctxMenuItem.value.id });
  await watchlist.fetchWatchlist();
  showCtxMenu.value = false;
}

const ctxOptions = [
  { label: '置顶', key: 'top' },
  { label: '上移', key: 'up' },
  { label: '下移', key: 'down' },
  { type: 'divider' as const, key: 'd1' },
  { label: '删除', key: 'delete' },
];

function handleCtxSelect(key: string) {
  switch (key) {
    case 'top': handleMoveTop(); break;
    case 'up': handleMoveUp(); break;
    case 'down': handleMoveDown(); break;
    case 'delete': handleDelete(); break;
  }
}

const columns: DataTableColumns<WatchItem> = [
  { title: '代码', key: 'code', width: 80 },
  { title: '名称', key: 'name', width: 100, ellipsis: true },
  {
    title: '最新价', key: 'price', width: 100,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      return q?.price?.toFixed(2) ?? '--';
    }
  },
  {
    title: '涨跌幅', key: 'change_pct', width: 100,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change_pct;
      const color = v >= 0 ? '#f85149' : '#3fb950';
      return h('span', { style: { color, fontWeight: 500 } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`);
    }
  },
  {
    title: '涨跌额', key: 'change', width: 90,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change;
      const color = v >= 0 ? '#f85149' : '#3fb950';
      return h('span', { style: { color } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}`);
    }
  },
  {
    title: '', key: 'action', width: 50, fixed: 'right',
    render(row) {
      return h(NButton, {
        size: 'tiny', text: true, type: 'error',
        onClick: async () => {
          await watchlist.removeStock(row.code, row.market);
          message.success(`已移除 ${row.name}`);
        }
      }, { default: () => '−' });
    }
  },
];
</script>

<template>
  <div class="watchlist-container">
    <div class="watchlist-header">
      <h2 class="section-title">自选股</h2>
      <NButton size="small" type="primary" @click="showAddDialog = true" class="add-btn">
        + 添加
      </NButton>
    </div>

    <div v-if="watchlist.items.length === 0" class="empty-state">
      <span class="empty-icon">📋</span>
      <p class="empty-text">暂无自选标的</p>
      <p class="empty-hint">点击「+ 添加」搜索并添加股票</p>
    </div>

    <NDataTable
      v-else
      :columns="columns"
      :data="watchlist.items"
      :bordered="false"
      :single-line="true"
      size="small"
      :row-props="(row: WatchItem) => ({ style: 'height: 36px; cursor: context-menu', onContextmenu: (e: MouseEvent) => handleContextMenu(e, row) })"
      flex-height
      class="watchlist-table"
    />

    <AddStockDialog v-model:show="showAddDialog" />

    <NDropdown
      :show="showCtxMenu"
      :x="ctxMenuX"
      :y="ctxMenuY"
      :options="ctxOptions"
      placement="bottom-start"
      trigger="manual"
      @select="handleCtxSelect"
      @clickoutside="showCtxMenu = false"
    />
  </div>
</template>

<style scoped>
.watchlist-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.watchlist-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) var(--space-4);
  flex-shrink: 0;
}
.section-title {
  font-size: var(--text-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  letter-spacing: var(--tracking-tight);
}
.add-btn {
  font-size: var(--text-xs);
  height: 28px;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  color: var(--color-text-tertiary);
}
.empty-icon { font-size: 2rem; opacity: 0.4; }
.empty-text { font-size: var(--text-md); font-weight: var(--font-weight-medium); color: var(--color-text-secondary); }
.empty-hint { font-size: var(--text-xs); }

:deep(.watchlist-table) {
  flex: 1;
}
</style>
