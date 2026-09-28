<script setup lang="ts">
import { computed, h, inject, onMounted, ref } from 'vue';
import { NButton, NDataTable, NDropdown, useMessage } from 'naive-ui';
import type { DataTableColumns, DropdownOption } from 'naive-ui';
import { useWatchlistStore } from '@/stores/watchlist';
import { useQuoteStore } from '@/stores/quote';
import { useMarketStore } from '@/stores/market';
import { useSettingsStore } from '@/stores/settings';
import type { WatchItem } from '@/types';
import { Inbox, Plus } from '@lucide/vue';
import { formatPrice, formatVolume, formatCode, cnCategory } from '@/utils/format';
import { columnLabel, type ColumnKey } from '@/utils/prefs';
import AddStockDialog from './AddStockDialog.vue';
import GroupTabs from './GroupTabs.vue';
import MarketTag from './MarketTag.vue';
import StockDetail from '@/components/detail/StockDetail.vue';
import { CLEAR_INDEX_DETAIL_KEY } from '@/utils/keys';

const watchlist = useWatchlistStore();
const quoteStore = useQuoteStore();
const market = useMarketStore();
const settings = useSettingsStore();
const message = useMessage();
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

const selectedRow = ref<WatchItem | null>(null);

// ── 列定义 ──
// 表格列按设置里的 key 顺序动态拼装。宽度写死而不是自适应：金融数据列需要
// 数字成列对齐（tabular-nums），自适应宽度会让每次行情跳动都重排列宽。

const COLUMN_WIDTHS: Record<ColumnKey, number> = {
  code: 76,
  name: 190,
  price: 100,
  change_pct: 100,
  change: 90,
  volume: 90,
  turnover: 100,
  turnover_rate: 80,
};

/** 涨跌色列。统一走 .pct-col 的 up/down 类，配色方案切换时无需改这里。 */
function coloredPct(row: WatchItem, text: string) {
  const q = quoteStore.getQuote(row.code, row.market);
  if (!q) return '--';
  return h('span', { class: `pct-col ${q.change_pct >= 0 ? 'up' : 'down'}` }, text);
}

/** 可排序的数值列。排除 code/name —— 它们是字符串，做减法没有意义。 */
type NumericColumnKey = 'price' | 'change_pct' | 'change' | 'volume' | 'turnover' | 'turnover_rate';

function sorterOf(key: NumericColumnKey) {
  return (a: WatchItem, b: WatchItem) => {
    const qa = quoteStore.getQuote(a.code, a.market);
    const qb = quoteStore.getQuote(b.code, b.market);
    return (qa?.[key] ?? 0) - (qb?.[key] ?? 0);
  };
}

function buildColumn(key: ColumnKey): DataTableColumns<WatchItem>[number] {
  const width = COLUMN_WIDTHS[key];
  const title = columnLabel(key);

  switch (key) {
    case 'code':
      return {
        title, key, width,
        render: (row) => h('span', { class: 'code-text' }, formatCode(row.code)),
      };
    case 'name':
      return {
        title, key, width,
        render: (row) => h('div', { class: 'name-cell' }, [
          h(MarketTag, { code: row.code, category: cnCategory(row.code) }),
          h('span', { class: 'name-text' }, row.name),
        ]),
      };
    case 'price':
      return {
        title, key, width, sorter: sorterOf('price'),
        render: (row) => {
          const q = quoteStore.getQuote(row.code, row.market);
          if (!q) return '--';
          return coloredPct(row, formatPrice(q.price));
        },
      };
    case 'change_pct':
      return {
        title, key, width, sorter: sorterOf('change_pct'),
        render: (row) => {
          const q = quoteStore.getQuote(row.code, row.market);
          if (!q) return '--';
          return coloredPct(row, `${q.change_pct >= 0 ? '+' : ''}${q.change_pct.toFixed(2)}%`);
        },
      };
    case 'change':
      return {
        title, key, width, sorter: sorterOf('change'),
        render: (row) => {
          const q = quoteStore.getQuote(row.code, row.market);
          if (!q) return '--';
          return coloredPct(row, `${q.change >= 0 ? '+' : ''}${formatPrice(q.change)}`);
        },
      };
    case 'volume':
      return {
        title, key, width, sorter: sorterOf('volume'),
        render: (row) => {
          const q = quoteStore.getQuote(row.code, row.market);
          if (!q || q.volume == null) return '--';
          return h('span', formatVolume(q.volume));
        },
      };
    case 'turnover':
      return {
        title, key, width, sorter: sorterOf('turnover'),
        render: (row) => {
          const q = quoteStore.getQuote(row.code, row.market);
          if (!q || q.turnover == null) return '--';
          const wan = q.turnover / 10000;
          if (wan >= 10000) return h('span', `${(wan / 10000).toFixed(2)}亿`);
          if (wan > 0) return h('span', `${wan.toFixed(2)}万`);
          return h('span', '0');
        },
      };
    case 'turnover_rate':
      return {
        title, key, width, sorter: sorterOf('turnover_rate'),
        render: (row) => {
          const q = quoteStore.getQuote(row.code, row.market);
          if (!q || q.turnover_rate == null) return '--';
          return h('span', `${q.turnover_rate.toFixed(2)}%`);
        },
      };
  }
}

/**
 * 列配置 + 默认排序一起决定表格形态。
 *
 * `defaultSortOrder` 是 naive-ui 的**非受控**初值：只在表格挂载时生效一次。
 * 这正好符合「默认排序」的语义（点列头临时改排，不该被设置覆盖），但代价是
 * 改完设置必须重新挂载表格才生效 —— AppLayout 用 v-if 切换设置页与看盘界面，
 * 返回时表格本来就会重建，所以这点自动成立。
 */
const columns = computed<DataTableColumns<WatchItem>>(() => {
  const def = settings.watchlistDefaultSort;
  return settings.watchlistColumns.map((key) => {
    const col = buildColumn(key);
    if (def && def.key === key) {
      return { ...col, defaultSortOrder: def.order };
    }
    return col;
  });
});

// ── 右键菜单 ──

const ctxX = ref(0);
const ctxY = ref(0);
const ctxItem = ref<WatchItem | null>(null);
const showCtxMenu = ref(false);

function handleContextMenu(e: MouseEvent, row: WatchItem) {
  e.preventDefault();
  // 菜单大致尺寸，用来把位置夹进视口
  const menuW = 176;
  const menuH = 230;
  ctxX.value = Math.min(e.clientX, window.innerWidth - menuW);
  ctxY.value = Math.min(e.clientY, window.innerHeight - menuH);
  ctxItem.value = row;
  showCtxMenu.value = true;
}

/** 当前分组里的位置，用于禁用首/末行的上移/下移。 */
const ctxIndex = computed(() =>
  ctxItem.value ? watchlist.visibleItems.findIndex((i) => i.id === ctxItem.value!.id) : -1,
);

/**
 * 「从本组移除」在只剩这一个分组时，后端会连股票一起删掉。菜单文案随之切换成
 * 「删除自选」—— 不让用户在毫无提示的情况下丢数据。
 */
const isOnlyGroup = computed(() => {
  if (!ctxItem.value) return false;
  return watchlist.groupsOf(ctxItem.value.id).length <= 1;
});

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
const iconFolder = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: 'currentColor', strokeWidth: 1.6, style: 'vertical-align:middle;margin-right:6px' }, [
  h('rect', { x: 2, y: 3.5, width: 9, height: 9, rx: 1.5 }),
  h('path', { d: 'M13.5 5.5v6a2 2 0 0 1-2 2h-6' }),
]);
const iconRemove = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: 'currentColor', strokeWidth: 1.6, style: 'vertical-align:middle;margin-right:6px' }, [
  h('path', { d: 'M3 8h10' }),
  h('path', { d: 'M8 3l5 5-5 5' }),
]);
const iconDelete = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: '#f85149', strokeWidth: 1.5, style: 'vertical-align:middle;margin-right:6px' }, [
  h('path', { d: 'M3 4h10' }),
  h('path', { d: 'M5 4V3a1 1 0 011-1h4a1 1 0 011 1v1' }),
  h('path', { d: 'M6 7v4' }),
  h('path', { d: 'M10 7v4' }),
  h('path', { d: 'M4 4l1 9h6l1-9' }),
]);

const checkMark = () => h('svg', { viewBox: '0 0 16 16', width: 14, height: 14, fill: 'none', stroke: 'currentColor', strokeWidth: 2, style: 'vertical-align:middle;margin-right:6px' }, [
  h('polyline', { points: '3 8.5 6.5 12 13 4' }),
]);

/**
 * 「添加到分组」子菜单：多归属，所以是**勾选**而不是单选移动。
 * 勾选状态来自每只股票当前的分组集合。
 */
const groupOptions = computed<DropdownOption[]>(() => {
  const item = ctxItem.value;
  if (!item) return [];
  const memberOf = new Set(watchlist.groupsOf(item.id).map((g) => g.id));
  return watchlist.groups.map((g) => ({
    key: `grp:${g.id}`,
    label: g.name,
    // naive-ui 的 dropdown 没有 checkbox 类型，用 icon 位置画勾
    icon: memberOf.has(g.id) ? checkMark : undefined,
  }));
});

const ctxOptions = computed<DropdownOption[]>(() => [
  { label: '置顶', key: 'top', icon: iconTop, disabled: ctxIndex.value <= 0 },
  { label: '上移', key: 'up', icon: iconUp, disabled: ctxIndex.value <= 0 },
  { label: '下移', key: 'down', icon: iconDown, disabled: ctxIndex.value >= watchlist.visibleItems.length - 1 },
  { type: 'divider', key: 'd1' },
  { label: '添加到分组', key: 'groups', icon: iconFolder, children: groupOptions.value },
  { type: 'divider', key: 'd2' },
  {
    label: isOnlyGroup.value ? '删除自选' : '从本组移除',
    key: 'remove',
    icon: iconRemove,
  },
  { label: '删除自选', key: 'delete', icon: iconDelete },
]);

function toggleMembership(groupId: number, checked: boolean) {
  const item = ctxItem.value;
  if (!item) return;
  const current = watchlist.groupsOf(item.id).map((g) => g.id);
  const next = checked
    ? [...current, groupId]
    : current.filter((id) => id !== groupId);
  if (next.length === 0) {
    // 后端也会拒绝空集合，但这里先给出可执行的解释，而不是把原始报错甩给用户
    message.warning('至少要属于一个分组。如果确实不需要了，请用「删除自选」。');
    return;
  }
  void watchlist.setWatchGroups(item.id, next);
}

async function handleCtxSelect(key: string) {
  const item = ctxItem.value;
  showCtxMenu.value = false;
  if (!item) return;

  if (key.startsWith('grp:')) {
    const gid = Number(key.slice(4));
    const wasMember = watchlist.groupsOf(item.id).some((g) => g.id === gid);
    toggleMembership(gid, !wasMember);
    return;
  }

  switch (key) {
    case 'top': await watchlist.moveTop(item.id); break;
    case 'up': await watchlist.moveUp(item.id); break;
    case 'down': await watchlist.moveDown(item.id); break;
    case 'remove': {
      const wasOnly = watchlist.groupsOf(item.id).length <= 1;
      await watchlist.removeFromActiveGroup(item.id);
      // 后端在「移出最后一个分组」时会连股票一起删掉，如实告知而不是静默消失
      if (wasOnly) message.info(`已删除 ${item.name}`);
      break;
    }
    case 'delete':
      await watchlist.removeStock(item.code, item.market);
      break;
  }
}

const emptyGroupName = computed(() => watchlist.groupName(watchlist.activeGroupId));

defineExpose({ clearSelection: () => { selectedRow.value = null; } });
</script>

<template>
  <div class="watchlist-container">
    <div class="watchlist-header">
      <GroupTabs />
      <button class="add-btn" @click="showAddDialog = true" aria-label="添加自选股票">
        <Plus :size="14" aria-hidden="true" />
        添加自选
      </button>
    </div>

    <div v-if="watchlist.error" class="error-state" role="alert">
      <p class="error-text">{{ watchlist.error }}</p>
      <NButton size="tiny" @click="watchlist.fetchWatchlist()">重试</NButton>
    </div>
    <div v-else-if="watchlist.visibleItems.length === 0" class="empty-state">
      <Inbox class="empty-icon" :size="32" aria-hidden="true" />
      <!-- 区分「整个自选是空的」与「只是这个分组空」：后者用户会以为数据丢了 -->
      <template v-if="watchlist.items.length === 0">
        <p class="empty-text">暂无自选股票</p>
        <p class="empty-hint">点击「添加自选」搜索并添加股票</p>
      </template>
      <template v-else>
        <p class="empty-text">「{{ emptyGroupName }}」分组下暂无自选</p>
        <p class="empty-hint">切换到其它分组，或点上方「添加自选」加入本组</p>
      </template>
    </div>

    <NDataTable
      v-else
      :columns="columns"
      :data="watchlist.visibleItems"
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
            // 详情面板要占竖向空间,先把同样吃高度的市场概览收起(已折叠时是 no-op)
            market.setExpanded(false);
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
      :x="ctxX"
      :y="ctxY"
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
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--space-3);
  /* 下留 8px：分组标签原本直接压在分隔线上，标签文字与线之间只剩标签自身
     的内边距，视觉上"触底"。8px 也符合 4/8 间距节奏。标签的选中下划线跟着
     标签一起上移 —— 它是标签自身的状态指示，不必贴在分隔线上。 */
  padding: var(--space-2) 0;
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border-0);
}
.add-btn {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 0 12px;
  height: 28px;
  margin-bottom: 2px;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--color-accent);
  color: #fff;
  font-size: var(--text-xs);
  font-family: var(--font-sans);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  flex-shrink: 0;
  transition: filter var(--transition-fast);
}
.add-btn:hover {
  filter: brightness(1.15);
}
.add-btn:active {
  filter: brightness(0.9);
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
  margin-top: var(--space-1);
}
/* P&L color classes (used via render functions) */
:deep(.pct-col) { font-weight: 500; }
:deep(.pct-col.up) { color: var(--color-up); }
:deep(.pct-col.down) { color: var(--color-down); }
/* 列渲染内容由 NDataTable 挂载，scoped 样式需用 :deep() 才能生效 */
:deep(.code-text) {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
:deep(.name-cell) {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
:deep(.name-text) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
