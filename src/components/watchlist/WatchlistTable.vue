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
  { title: '名称', key: 'name', width: 100 },
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
      return h('span', { style: { color: v >= 0 ? 'var(--color-up)' : 'var(--color-down)' } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`);
    }
  },
  {
    title: '涨跌额', key: 'change', width: 100,
    render(row) {
      const q = quoteStore.getQuote(row.code, row.market);
      if (!q) return '--';
      const v = q.change;
      return h('span', { style: { color: v >= 0 ? 'var(--color-up)' : 'var(--color-down)' } },
        `${v >= 0 ? '+' : ''}${v.toFixed(2)}`);
    }
  },
  {
    title: '操作', key: 'action', width: 80, fixed: 'right',
    render(row) {
      return h(NButton, {
        size: 'tiny', text: true, type: 'error',
        onClick: async () => {
          await watchlist.removeStock(row.code, row.market);
          message.success(`已移除 ${row.name}`);
        }
      }, { default: () => '删除' });
    }
  },
];
</script>

<template>
  <div class="watchlist-container">
    <div class="watchlist-header">
      <span class="section-title">自选股</span>
      <NButton size="small" type="primary" @click="showAddDialog = true">
        + 添加
      </NButton>
    </div>
    <NDataTable
      :columns="columns"
      :data="watchlist.items"
      :bordered="false"
      :single-line="false"
      size="small"
      max-height="500"
      virtual-scroll
    />
    <AddStockDialog v-model:show="showAddDialog" />
  </div>
</template>

<style scoped>
.watchlist-container {
  flex: 1;
  padding: 8px 16px;
  overflow: auto;
}
.watchlist-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}
.section-title {
  font-weight: 600;
  font-size: var(--font-size-base);
  color: var(--color-text-primary);
}
</style>
