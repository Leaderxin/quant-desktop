<script setup lang="ts">
import { ref, h, inject, onMounted } from 'vue';
import { NButton, NDataTable, NDropdown } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import type { WatchItem } from '@/types';
import { formatPrice } from '@/utils/format';
import AddStockDialog from './AddStockDialog.vue';
import StockDetail from '@/components/detail/StockDetail.vue';
import { CLEAR_INDEX_DETAIL_KEY } from '@/components/layout/AppLayout.vue';

const watchlist = useWatchlistStore();
const quoteStore = useQuoteStore();
const showAddDialog = ref(false);

const indexDetailCoord = inject<{
  clearIndexDetail: () => void;
  registerClearStockFn?: (fn: () => void) => void;
} | undefined>(CLEAR_INDEX_DETAIL_KEY);

onMounted(() => {
  indexDetailCoord?.registerClearStockFn?.(() => {
    selectedRow.value = null;
  });
});

// Context menu state
const ctxMenuX = ref(0);
const ctxMenuY = ref(0);
const ctxMenuItem = ref<WatchItem | null>(null);
const showCtxMenu = ref(false);

// Detail panel state
const selectedRow = ref<WatchItem | null>(null);

function handleContextMenu(e: MouseEvent, row: WatchItem) {
  e.preventDefault();
  ctxMenuX.value = e.clientX;
  ctxMenuY.value = e.clientY;
  ctxMenuItem.value = row;
  showCtxMenu.value = true;
}

async function handleDelete() {
  if (!ctxMenuItem.value) return;
  try {
    await watchlist.removeStock(ctxMenuItem.value.code, ctxMenuItem.value.market);
  } catch (e) {
    console.error('removeStock failed:', e);
  }
  showCtxMenu.value = false;
}

async function handleMoveTop() {
  if (!ctxMenuItem.value) return;
  try {
    await invoke('move_watch_top', { id: ctxMenuItem.value.id });
    await watchlist.fetchWatchlist();
  } catch (e) {
    console.error('move_watch_top failed:', e);
  }
  showCtxMenu.value = false;
}

async function handleMoveUp() {
  if (!ctxMenuItem.value) return;
  try {
    await invoke('move_watch_up', { id: ctxMenuItem.value.id });
    await watchlist.fetchWatchlist();
  } catch (e) {
    console.error('move_watch_up failed:', e);
  }
  showCtxMenu.value = false;
}

async function handleMoveDown() {
  if (!ctxMenuItem.value) return;
  try {
    await invoke('move_watch_down', { id: ctxMenuItem.value.id });
    await watchlist.fetchWatchlist();
  } catch (e) {
    console.error('move_watch_down failed:', e);
  }
  showCtxMenu.value = false;
}

const iconTop = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: 'currentColor', strokeWidth: 2, style: 'vertical-align:middle;margin-right:6px' }, [
  h('path', { d: 'M8 2V14' }),
  h('polyline', { points: '4 6 8 2 12 6' }),
  h('line', { x1: 2, y1: 14, x2: 14, y2: 14 }),
]);
const iconUp = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: 'currentColor', strokeWidth: 2, style: 'vertical-align:middle;margin-right:6px' }, [
  h('polyline', { points: '4 9 8 5 12 9' }),
]);
const iconDown = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: 'currentColor', strokeWidth: 2, style: 'vertical-align:middle;margin-right:6px' }, [
  h('polyline', { points: '4 5 8 9 12 5' }),
]);
const iconDelete = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: '#f85149', strokeWidth: 1.5, style: 'vertical-align:middle;margin-right:6px' }, [
  h('path', { d: 'M3 4h10' }),
  h('path', { d: 'M5 4V3a1 1 0 011-1h4a1 1 0 011 1v1' }),
  h('path', { d: 'M6 7v4' }),
  h('path', { d: 'M10 7v4' }),
  h('path', { d: 'M4 4l1 9h6l1-9' }),
]);

const ctxOptions = [
  { label: '置顶', key: 'top', icon: iconTop },
  { label: '上移', key: 'up', icon: iconUp },
  { label: '下移', key: 'down', icon: iconDown },
  { type: 'divider' as const, key: 'd1' },
  { label: '删除', key: 'delete', icon: iconDelete },
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
  { title: '代码', key: 'code', width: 68 },
  {
    title: '名称', key: 'name', width: 120, ellipsis: true,
    sorter: (a: WatchItem, b: WatchItem) => a.name.localeCompare(b.name),
  },
  {
    title: '最新价', key: 'price', width: 100,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.price ?? 0) - (qb?.price ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      return formatPrice(q?.price);
    }
  },
  {
    title: '涨跌幅', key: 'change_pct', width: 100,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.change_pct ?? 0) - (qb?.change_pct ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change_pct;
      return h('span', { class: `pct-col ${v >= 0 ? 'up' : 'down'}` },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`);
    }
  },
  {
    title: '涨跌额', key: 'change', width: 90,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.change ?? 0) - (qb?.change ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change;
      return h('span', { class: `pct-col ${v >= 0 ? 'up' : 'down'}` },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}`);
    }
  },
  {
    title: '成交量', key: 'volume', width: 90,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.volume ?? 0) - (qb?.volume ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q || q.volume == null) return '--';
      // volume is in shares; 1 手 = 100 shares
      const shou = q.volume / 100;
      if (shou >= 10000) return h('span', `${(shou / 10000).toFixed(2)}万手`);
      if (shou > 0) return h('span', `${shou.toFixed(0)}手`);
      return h('span', '0手');
    }
  },
  {
    title: '成交额', key: 'turnover', width: 90,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.turnover ?? 0) - (qb?.turnover ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q || q.turnover == null) return '--';
      // turnover is in 元; display in 万元 or 亿元
      const wan = q.turnover / 10000;
      if (wan >= 10000) return h('span', `${(wan / 10000).toFixed(2)}亿`);
      if (wan > 0) return h('span', `${wan.toFixed(2)}万`);
      return h('span', '0');
    }
  },
  {
    title: '换手率', key: 'turnover_rate', width: 80,
    sorter: (a: WatchItem, b: WatchItem) => {
      const qa = quoteStore.getQuote(a.code, a.market);
      const qb = quoteStore.getQuote(b.code, b.market);
      return (qa?.turnover_rate ?? 0) - (qb?.turnover_rate ?? 0);
    },
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q || q.turnover_rate == null) return '--';
      return h('span', `${q.turnover_rate.toFixed(2)}%`);
    }
  },
];

defineExpose({ clearSelection: () => { selectedRow.value = null; } });
</script>

<template>
  <div class="watchlist-container">
    <div class="watchlist-header">
      <h2 class="section-title">自选股</h2>
      <NButton size="small" type="primary" @click="showAddDialog = true" class="add-btn" aria-label="添加自选股票">
        + 添加
      </NButton>
    </div>

    <div v-if="watchlist.error" class="error-state" role="alert">
      <p class="error-text">{{ watchlist.error }}</p>
      <NButton size="tiny" @click="watchlist.fetchWatchlist()">重试</NButton>
    </div>
    <div v-else-if="watchlist.items.length === 0" class="empty-state">
      <svg class="empty-icon" viewBox="0 0 32 32" width="32" height="32" fill="none" aria-hidden="true">
        <rect x="4" y="6" width="24" height="20" rx="2" stroke="currentColor" stroke-width="1.5"/>
        <line x1="4" y1="12" x2="28" y2="12" stroke="currentColor" stroke-width="1.5"/>
        <line x1="10" y1="16" x2="14" y2="16" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        <line x1="10" y1="20" x2="18" y2="20" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
      </svg>
      <p class="empty-text">暂无自选股票</p>
      <p class="empty-hint">点击「+ 添加」搜索并添加股票</p>
    </div>

    <NDataTable
      v-else
      :columns="columns"
      :data="watchlist.items"
      :bordered="false"
      :single-line="true"
      size="small"
      :row-props="(row: WatchItem) => ({
        style: `height: 36px; cursor: pointer; ${selectedRow?.id === row.id ? 'background: var(--color-bg-elevated, rgba(255,255,255,0.04))' : ''}`,
        onContextmenu: (e: MouseEvent) => handleContextMenu(e, row),
        onClick: () => {
          if (selectedRow?.id === row.id) {
            selectedRow = null;
          } else {
            indexDetailCoord?.clearIndexDetail();
            selectedRow = row;
          }
        }
      })"
      flex-height
      class="watchlist-table"
    />

    <StockDetail
      v-if="selectedRow"
      :item="selectedRow"
      @close="selectedRow = null"
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
  padding: 0 var(--space-4);
}
.watchlist-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) 0;
  flex-shrink: 0;
}
.section-title {
  font-size: var(--text-md);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  letter-spacing: -0.01em;
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
.empty-icon { color: var(--color-text-tertiary); opacity: 0.4; }
.empty-text { font-size: var(--text-md); font-weight: var(--font-weight-medium); color: var(--color-text-secondary); }
.empty-hint { font-size: var(--text-xs); }
.error-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
}
.error-text {
  font-size: var(--text-sm);
  color: var(--color-warning);
  text-align: center;
  max-width: 300px;
}

:deep(.watchlist-table) {
  flex: 1;
}
/* P&L color classes (used via render functions) */
:deep(.pct-col) { font-weight: 500; }
:deep(.pct-col.up) { color: var(--color-up); }
:deep(.pct-col.down) { color: var(--color-down); }
</style>
