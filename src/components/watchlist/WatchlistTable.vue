<script setup lang="ts">
import { ref } from 'vue';
import { NButton, NDataTable, useMessage } from 'naive-ui';
import type { DataTableColumns } from 'naive-ui';
import { h } from 'vue';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import type { WatchItem } from '@/types';
import AddStockDialog from './AddStockDialog.vue';

const watchlist = useWatchlistStore();
const quoteStore = useQuoteStore();
const message = useMessage();
const showAddDialog = ref(false);

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
      :row-props="() => ({ style: 'height: 36px' })"
      flex-height
      class="watchlist-table"
    />

    <AddStockDialog v-model:show="showAddDialog" />
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
